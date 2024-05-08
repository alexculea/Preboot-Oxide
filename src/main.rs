mod conf;
mod util;

#[macro_use]
extern crate anyhow;

use anyhow::{Context, Ok};
use async_std::prelude::FutureExt;
use async_std::sync::Mutex;
use async_std::task::sleep;
use async_std::{net::UdpSocket, task};
use dhcproto::v4::Opcode;
use log::{debug, error, info, trace};
use network_interface::NetworkInterface;
use network_interface::NetworkInterfaceConfig;
use polling::AsRawSource;
use polling::AsSource;
use polling::Events;
use polling::{Event, Poller};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::future::Future;
use std::net::UdpSocket as StdUdpSocket;
use std::os::fd::AsFd;
use std::os::fd::AsRawFd;
use std::os::fd::BorrowedFd;
use std::os::fd::FromRawFd;
use std::os::fd::IntoRawFd;
use std::os::fd::OwnedFd;
use std::os::fd::RawFd;
use std::sync::Arc;
use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
};

use dhcproto::v4::{
    Decodable, Decoder, DhcpOption, DhcpOptions, Encodable, Encoder, Flags, Message, MessageType,
    OptionCode,
};

use crate::conf::Conf;

struct Session {
    pub client_ip: Option<Ipv4Addr>,
    pub subnet: DhcpOption,
    pub lease_time: DhcpOption,
}

type SessionMap = HashMap<u32, Session>;

type Result<T> = anyhow::Result<T, anyhow::Error>;

async fn server_loop(server_config: Conf) -> Result<()> {
    let listen_ips = ["0.0.0.0:67", "255.255.255.255:68"];
    let sessions: Arc<Mutex<SessionMap>> = Arc::new(Mutex::new(Default::default()));
    let network_interfaces = NetworkInterface::show().unwrap();
    let sockets2: Vec<Socket> = network_interfaces
        .iter()
        .map(|itf| {
            listen_ips
                .iter()
                .map(|ip| {
                    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
                    socket.set_broadcast(true)?;
                    socket.bind_device(Some(itf.name.as_bytes()))?;
                    socket.bind(&SockAddr::from(ip.parse::<SocketAddrV4>()?))?;

                    info!("Listening on IP {ip} on device {}", itf.name);
                    Ok(socket)
                })
                .collect::<Result<Vec<Socket>>>()
        })
        .collect::<Result<Vec<Vec<Socket>>>>()?
        .into_iter()
        .flatten()
        .collect();

    let sockets: Vec<UdpSocket> = sockets2
        .into_iter()
        .map(|socket| socket2_to_async_std(socket))
        .collect();
    let sockets = Arc::new(sockets);

    let poller = Poller::new().context("Setting up OS polling")?;
    sockets
        .iter()
        .enumerate()
        .map(|(index, socket)| {
            // SAFETY: sources have to be deleted before the poller is dropped
            unsafe { poller.add(socket, polling::Event::readable(index)) }
        })
        .collect::<std::io::Result<()>>()?;

    let mut events = Events::new();
    loop {
        let _ = poller.wait(&mut events, None)?;
        sockets
            .iter()
            .enumerate()
            .map(|(index, socket)| {
                unsafe {
                    // SAFETY: The resource pointed to by fd must remain open for the duration of the returned BorrowedFd, and it must not have the value -1.
                    let fd = BorrowedFd::borrow_raw(socket.as_raw_fd());

                    // SAFETY: sources have to be deleted before the poller is dropped
                    poller.modify(fd, polling::Event::readable(index))
                }
            })
            .collect::<std::io::Result<()>>()?;

        for event in events.iter() {
            let mut buf = vec![0u8; 1024];
            let task_sockets = Arc::clone(&sockets);
            let (bytes_read, peer) = task_sockets[event.key].recv_from(&mut buf).await?;
            if bytes_read == 0 {
                continue;
            }

            let sessions = sessions.clone();
            let server_config = server_config.clone();
            task::spawn(async move {
                let _ = handle_dhcp_message(buf, peer, task_sockets, &server_config, sessions)
                    .await
                    .map_err(|e| error!("{}", e));
            });
        }

        events.clear();
    }
}

async fn handle_dhcp_message(
    rcv_data: Vec<u8>,
    peer: SocketAddr,
    server_sockets: Arc<Vec<UdpSocket>>,
    server_config: &Conf,
    sessions: Arc<Mutex<SessionMap>>,
) -> Result<()> {
    let client_msg = Message::decode(&mut Decoder::new(&rcv_data))?;
    let opts = client_msg.opts();
    let msg_type = match opts.msg_type() {
        Some(inner) => inner,
        None => bail!("Can't get DHCP message type"),
    };

    debug!(
        "Received from IP: {}, port: {}, DHCP Msg: {:?}",
        peer.ip(),
        peer.port(),
        msg_type
    );
    trace!("{}", client_msg.to_string());

    if !matches_filter(&client_msg) {
        debug!("Ignoring DHCP request.");
        return Ok(());
    }

    let client_xid = client_msg.xid();

    let response = match msg_type {
        MessageType::Offer => {
            sessions.lock().await.insert(
                client_msg.xid(),
                Session {
                    client_ip: Some(client_msg.yiaddr()),
                    subnet: client_msg
                        .opts()
                        .get(OptionCode::SubnetMask)
                        .unwrap()
                        .clone(),
                    lease_time: client_msg
                        .opts()
                        .get(OptionCode::AddressLeaseTime)
                        .unwrap()
                        .clone(),
                },
            );

            let msg = apply_self_to_message(&client_msg, server_config, None);
            add_boot_info_to_message(&msg, server_config, None)
        }
        MessageType::Request => {
            let sessions = sessions.lock().await;
            let session = sessions.get(&client_xid).context(format!(
                "No session state found for transaction {}. Ignoring.",
                client_xid
            ))?;

            let mut ack = Message::default();
            let mut opts = DhcpOptions::default();
            opts.insert(DhcpOption::MessageType(MessageType::Ack));
            opts.insert(session.subnet.clone());
            opts.insert(session.lease_time.clone());

            ack.set_flags(Flags::new(0).set_broadcast())
                .set_yiaddr(session.client_ip.unwrap())
                .set_opcode(Opcode::BootReply)
                .set_opts(opts)
                .set_xid(client_msg.xid());
            drop(sessions);

            ack = apply_self_to_message(&ack, server_config, None);
            ack = add_boot_info_to_message(&ack, server_config, None);

            ack
        }
        MessageType::Decline => {
            bail!(
                "Client {} declined the offer.",
                client_msg
                    .chaddr()
                    .iter()
                    .map(|x| format!("{:02X}", x))
                    .collect::<String>()
            );
        }
        _ => return Ok(()),
    };

    let to_addr = "255.255.255.255:68";
    let mut buf = Vec::new();
    let mut e = Encoder::new(&mut buf);
    response.encode(&mut e)?;

    trace!("Responding with message: {}", response.to_string());

    for socket in server_sockets.iter() {
        socket.send_to(&buf, to_addr).await?;
        debug!(
            "DHCP reply ({:?}) sent to: {}",
            response.opts().get(OptionCode::MessageType).unwrap(),
            to_addr
        );
    }

    // let client_socket = UdpSocket::bind(to_addr).await?;
    // let client_socket = UdpSocket::bind(to_addr).await?;
    // client_socket.set_broadcast(true)?;
    // client_socket.connect(to_addr).await?;
    // client_socket.send(&buf).await?;

    trace!("Worker, about to exit.");
    Ok(())
}

fn matches_filter(msg: &Message) -> bool {
    let msg_opts = msg.opts();
    let has_boot_file_name = msg_opts.get(OptionCode::BootfileName).is_some();
    let is_request = msg_opts.has_msg_type(MessageType::Request);
    let is_offer = msg_opts.has_msg_type(MessageType::Offer);
    let is_ack = msg_opts.has_msg_type(MessageType::Ack);

    let matches = (!has_boot_file_name && is_offer) | is_request | is_ack;
    if !matches {
        debug!(
            "DHCP message ignored due to not matching filter. \
            Required: has_boot_file_name: {has_boot_file_name}, is_request: {is_request}\
            is_offer: {is_offer}, is_ack: {is_ack}"
        );
    }

    trace!("Elligible DHCP message found, intercepting.");

    matches
}

fn socket2_to_async_std(socket: Socket) -> UdpSocket {
    let std_socket = Into::<std::net::UdpSocket>::into(socket);
    UdpSocket::from(std_socket)
}

fn async_socket_to_socket2(socket: UdpSocket) -> Result<Socket> {
    let std_socket = std::net::UdpSocket::try_from(socket)?;
    Ok(Socket::from(std_socket))
}

// async fn relay_message_to_main_dhcp(from: impl ToSocketAddrs, msg: &Message) -> Result<Message> {
//     let mut encoder_buff = vec![0u8; 256];
//     let mut decoder_buff = vec![0u8; 256];

//     let mut encoder = Encoder::new(&mut encoder_buff);

//     let mut discover_msg = msg.clone();
//     discover_msg.set_xid(rand::thread_rng().gen());

//     let mut addr = from.to_socket_addrs().await?.next().unwrap();
//     addr.set_port(0);

//     let socket = UdpSocket::bind(addr).await?;
//     socket.set_broadcast(true)?;
//     socket.connect("255.255.255:68").await?;

//     discover_msg.encode(&mut encoder)?;

//     trace!("Relaying message to the main DHCP server.");
//     socket.send(&encoder_buff).await?;
//     trace!("Message relayed to the main DHCP server.");

//     socket.recv(&mut decoder_buff).await?;
//     let mut decoder = Decoder::new(&mut decoder_buff);
//     let response = Message::decode(&mut decoder)?;

//     trace!("Message received from main DHCP server.");
//     Ok(response)
// }

fn add_boot_info_to_message(msg: &Message, config: &Conf, my_ipv4: Option<Ipv4Addr>) -> Message {
    let mut res = msg.clone();
    let mut opts = msg.opts().clone();

    let boot_filename = config.get_boot_file().unwrap();
    let tfpt_srv_addr = config.get_boot_server_ipv4(my_ipv4).unwrap();

    opts.insert(DhcpOption::BootfileName(boot_filename.as_bytes().to_vec()));
    opts.insert(DhcpOption::TFTPServerAddress(tfpt_srv_addr));
    opts.insert(DhcpOption::ServerIdentifier(tfpt_srv_addr));

    res.set_siaddr(tfpt_srv_addr)
        .set_fname_str(boot_filename)
        .set_opts(opts);

    return res;
}

fn make_offer_message(
    xid: u32,
    chaddr: &[u8],
    yip: Ipv4Addr,
    boot_filename: &str,
    tfpt_srv_addr: Ipv4Addr,
) -> Message {
    let mut res = Message::default();
    let mut opts = DhcpOptions::default();
    let flags = Flags::new(0).set_broadcast();

    opts.insert(DhcpOption::BootfileName(boot_filename.as_bytes().to_vec()));
    opts.insert(DhcpOption::TFTPServerAddress(tfpt_srv_addr));
    opts.insert(DhcpOption::MessageType(MessageType::Offer));
    opts.insert(DhcpOption::ServerIdentifier(tfpt_srv_addr));
    opts.insert(DhcpOption::SubnetMask("255.255.255.0".parse().unwrap()));
    opts.insert(DhcpOption::AddressLeaseTime(30 * 60));

    res.set_xid(xid)
        .set_yiaddr(yip)
        .set_fname_str(boot_filename)
        .set_siaddr(tfpt_srv_addr)
        .set_flags(flags)
        .set_chaddr(chaddr)
        .set_opcode(Opcode::BootReply)
        .set_opts(opts);

    res
}

fn apply_self_to_message(msg: &Message, conf: &Conf, my_ipv4: Option<Ipv4Addr>) -> Message {
    let mut res = msg.clone();
    let mut opts = msg.opts().clone();

    let tfpt_srv_addr = conf.get_boot_server_ipv4(my_ipv4).unwrap();
    opts.insert(DhcpOption::ServerIdentifier(tfpt_srv_addr));

    res.set_siaddr(tfpt_srv_addr).set_opts(opts);

    res
}

fn main() -> Result<()> {
    let mut dot_env_path = std::env::current_exe().unwrap_or_default();
    dot_env_path.set_file_name(".env");

    let _ = dotenv::from_path(dot_env_path);
    let log_level = std::env::var("PXE_DHCP_LOG_LEVEL").unwrap_or("error".into());

    pretty_env_logger::formatted_timed_builder()
        .parse_filters(&log_level)
        .init();

    let server_config = Conf::from_proccess_env()?;
    server_config.validate()?;

    let result =
        task::block_on(server_loop(server_config)).context(format!("Setting up network socket"));

    debug!("Exiting");
    result
}

// use socket2::{Domain, Type, Protocol, Socket};
// use std::net::SocketAddr;
// use async_std::net::UdpSocket;

// // Step 1: Create a socket2::Socket and set IP_DONTFRAG
// let domain = Domain::ipv4();
// let type_ = Type::dgram();
// let protocol = Protocol::udp();
// let socket = Socket::new(domain, type_, Some(protocol)).unwrap();
// socket.set_ip_dontfrag(true).unwrap();

// // Step 2: Convert into a std::net::UdpSocket
// let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
// socket.bind(&addr.into()).unwrap();
// let std_socket = socket.into_udp_socket();

// // Step 3: Convert into an async_std::net::UdpSocket
// let async_socket = UdpSocket::from(std_socket);

// // Now you can use async_socket with the DF flag set
