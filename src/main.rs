mod conf;
mod util;

#[macro_use]
extern crate anyhow;

use anyhow::{Context, Ok};
use async_std::sync::Mutex;
use async_std::{net::UdpSocket, task};
use dhcproto::v4::Opcode;
use log::{debug, error, info, trace};
use network_interface::NetworkInterface;
use network_interface::NetworkInterfaceConfig;
use polling::Events;
use polling::Poller;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::os::fd::AsRawFd;
use std::os::fd::BorrowedFd;
use std::sync::Arc;
use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddrV4},
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
        .filter(|iface| {
            server_config
                .get_ifaces()
                .map(|ifaces| ifaces.contains(&iface.name))
                .unwrap_or(true)
        })
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
            let task_sockets = Arc::clone(&sockets);
            let sessions = sessions.clone();
            let server_config = server_config.clone();
            task::spawn(async move {
                let _ = handle_dhcp_message(event.key, task_sockets, &server_config, sessions)
                    .await
                    .map_err(|e| error!("{}", e));
            });
        }

        events.clear();
    }
}

async fn handle_dhcp_message(
    receiving_socket_index: usize,
    server_sockets: Arc<Vec<UdpSocket>>,
    server_config: &Conf,
    sessions: Arc<Mutex<SessionMap>>,
) -> Result<()> {
    let receiving_socket = &server_sockets[receiving_socket_index];
    let mut rcv_data = vec![0u8; 256];
    let (bytes_read, peer) = receiving_socket.recv_from(&mut rcv_data).await?;
    if bytes_read == 0 {
        return Ok(());
    }

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
    let env_prefix = crate::conf::ENV_VAR_PREFIX;
    let log_level = std::env::var(format!("{env_prefix}LOG_LEVEL")).unwrap_or("error".into());

    pretty_env_logger::formatted_timed_builder()
        .parse_filters(&log_level)
        .init();

    let server_config = Conf::from_proccess_env()?;
    server_config.validate()?;

    let result =
        task::block_on(server_loop(server_config)).context(format!("Setting up network sockets"));

    debug!("Exiting");
    result
}
