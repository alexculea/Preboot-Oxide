mod conf;
mod util;

#[macro_use]
extern crate anyhow;

use anyhow::{Context, Ok};
use dhcproto::v4::Opcode;
use log::{debug, error, info, trace};
use network_interface::NetworkInterface;
use network_interface::NetworkInterfaceConfig;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
};

use async_std::{
    net::UdpSocket,
    task,
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
    let relay_mode = true;
    let addr = if relay_mode {
        "255.255.255.255:68"
    } else {
        "0.0.0.0:67"
    };

    let mut buf = vec![0u8; 1024];
    // let socket2 = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).unwrap();
    // socket2.set_broadcast(true)?;
    // socket2.bind(&addr.parse::<SocketAddr>()?.try_into()?)?;
    // let mut socket = UdpSocket::from(Into::<std::net::UdpSocket>::into(socket2));

    let mut socket = UdpSocket::bind(addr).await?;
    let mut sessions: SessionMap = Default::default();

    info!("Listening on {}.", addr.to_string());

    let network_interfaces = NetworkInterface::show().unwrap();
    for itf in network_interfaces.iter() {
        debug!(
            "Interface: {}:{} - broadcast: {}",
            itf.name,
            itf.addr[0].ip().to_string(),
            itf.addr[0].broadcast().unwrap().to_string()
        );
    }

    loop {
        let (message_size, peer) = socket.recv_from(&mut buf).await?;
        let _ = handle_dhcp_message(
            message_size,
            peer,
            &network_interfaces,
            &mut socket,
            &server_config,
            &mut sessions,
            relay_mode,
        )
        .await
        .map_err(|e| error!("{}", e));
    }
}

async fn handle_dhcp_message(
    size: usize,
    peer: SocketAddr,
    network_interfaces: &Vec<NetworkInterface>,
    socket: &mut UdpSocket,
    server_config: &Conf,
    sessions: &mut SessionMap,
    relay_mode: bool,
) -> Result<()> {    
    let client_msg = Message::decode(&mut Decoder::new(&buf))?;
    let opts = client_msg.opts();
    let msg_type = match opts.msg_type() {
        Some(inner) => inner,
        None => bail!("Can't get DHCP message type"),
    };

    debug!(
        "Received {} from IP: {}, port: {}, DHCP Msg: {:?}",
        size,
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
    let mut my_ipv4 = None;
    if let std::net::IpAddr::V4(ipv4) = socket.local_addr()?.ip() {
        my_ipv4 = Some(ipv4);
    }

    let response = match msg_type {
        MessageType::Offer => {
            sessions.insert(
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

            let msg = apply_self_to_message(&client_msg, server_config, my_ipv4);
            add_boot_info_to_message(&msg, server_config, my_ipv4)
        }
        MessageType::Discover => {
            if relay_mode {
                return Ok(());
            }

            make_offer_message(
                client_xid,
                client_msg.chaddr(),
                "10.5.5.12".parse().unwrap(),
                server_config.get_boot_file().unwrap().as_str(),
                server_config.get_boot_server_ipv4(my_ipv4).unwrap(),
            )
        }
        MessageType::Request => {
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

            ack = apply_self_to_message(&ack, server_config, my_ipv4);
            ack = add_boot_info_to_message(&ack, server_config, my_ipv4);

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

    let mut local_addr = socket.local_addr()?;
    local_addr.set_port(0);

    *socket = UdpSocket::bind(&local_addr).await?;

    for iface in network_interfaces.iter() {
        let from_addr = format!("{}:67", iface.addr[0].ip().to_string());
        let socket2 = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        socket2.set_broadcast(true)?;
        socket2.bind_device(Some(iface.name.as_bytes()))?;
        socket2.bind(&SockAddr::from(from_addr.parse::<SocketAddrV4>()?))?;

        let client_socket = UdpSocket::from(Into::<std::net::UdpSocket>::into(socket2));
        client_socket.send_to(&buf, to_addr).await?;
        debug!(
            "DHCP reply on {} ({:?}) sent to: {}",
            iface.name,
            response.opts().get(OptionCode::MessageType).unwrap(),
            to_addr
        );
    }

    local_addr.set_port(67);
    *socket = UdpSocket::bind(&local_addr).await?;

    // let client_socket = UdpSocket::bind(to_addr).await?;
    // let client_socket = UdpSocket::bind(to_addr).await?;
    // client_socket.set_broadcast(true)?;
    // client_socket.connect(to_addr).await?;
    // client_socket.send(&buf).await?;

    Ok(())
}

fn matches_filter(msg: &Message) -> bool {
    let msg_opts = msg.opts();
    let has_boot_file_name = msg_opts.get(OptionCode::BootfileName).is_some();
    let is_discover = msg_opts.has_msg_type(MessageType::Discover);
    let is_request = msg_opts.has_msg_type(MessageType::Request);
    let is_offer = msg_opts.has_msg_type(MessageType::Offer);
    let is_ack = msg_opts.has_msg_type(MessageType::Ack);

    let matches = (!has_boot_file_name && is_offer) | is_discover | is_request | is_ack;
    if !matches {
        debug!(
            "DHCP message ignored due to not matching filter. \
            Required: has_boot_file_name: {}, is_discover: {}, is_request: {}\
            is_offer: {}, is_ack: {}",
            has_boot_file_name, is_discover, is_request, is_offer, is_ack
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

    env_logger::builder().parse_filters(&log_level).init();

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
