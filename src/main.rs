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
use std::os::fd::FromRawFd;
use std::{
    collections::HashMap,
    convert::identity,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    time::Duration,
};

use async_std::{
    net::{ToSocketAddrs, UdpSocket}, // 3
    task,                            // 2
};
use std::os::fd::AsRawFd;

use dhcproto::v4::{
    Decodable, Decoder, DhcpOption, DhcpOptions, Encodable, Encoder, Flags, Message, MessageType,
    OptionCode,
};
use rand::Rng;

use crate::conf::Conf;

type Result<T> = anyhow::Result<T, anyhow::Error>;

async fn server_loop(server_config: Conf) -> Result<()> {
    let relay_mode = false;
    let addr = if relay_mode {
        "0.0.0.0:68"
    } else {
        "0.0.0.0:67"
    };
    let mut buf = vec![0u8; 1024];

    // Step 1: Create a socket2::Socket and set IP_DONTFRAG
    let domain = Domain::for_address(addr.parse()?);
    let socket_type = Type::DGRAM;
    let protocol = Protocol::UDP;
    let socket2 = Socket::new(domain, socket_type, Some(protocol)).unwrap();
    // socket2.set_tos(10)?;
    socket2.set_broadcast(true)?;
    socket2.bind(&addr.parse::<SocketAddr>()?.try_into()?)?;

    let mut socket = UdpSocket::from(Into::<std::net::UdpSocket>::into(socket2));
    let mut sessions: HashMap<u32, u32> = Default::default();

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
        let msg = Message::decode(&mut Decoder::new(&buf))?;
        let _ = handle_dhcp_message(
            msg,
            message_size,
            peer,
            &network_interfaces,
            &mut socket,
            &server_config,
            &mut sessions,
        )
        .await
        .map_err(|e| error!("{}", e));
    }
}

// Respond to DHCP Discover with a DHCP offer
// Handle DHCP Request of that offer by responding with a DHCP Acknowledge
async fn handle_dhcp_message(
    client_msg: Message,
    size: usize,
    peer: SocketAddr,
    network_interfaces: &Vec<NetworkInterface>,
    socket: &mut UdpSocket,
    server_config: &Conf,
    sessions: &mut HashMap<u32, u32>,
) -> Result<()> {
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
        MessageType::Offer | MessageType::Ack => {
            // let main_dhcp_server_discover = relay_message_to_main_dhcp(socket.local_addr()?, &client_msg).await?;
            // sessions.insert(main_dhcp_server_discover.xid(), client_xid);
            async_std::task::sleep(Duration::from_millis(500)).await;

            add_boot_info_to_message(
                client_msg,
                client_xid,
                server_config.get_boot_file().unwrap().as_str(),
                server_config.get_boot_server_ipv4(my_ipv4).unwrap(),
            )
        }
        MessageType::Discover => {
            async_std::task::sleep(Duration::from_millis(500)).await;
            make_offer_message(
                client_xid,
                client_msg.chaddr(),
                "10.5.5.12".parse().unwrap(),
                server_config.get_boot_file().unwrap().as_str(),
                server_config.get_boot_server_ipv4(my_ipv4).unwrap(),
            )
        }
        MessageType::Request => {
            async_std::task::sleep(Duration::from_millis(500)).await;
            let mut offer = make_offer_message(
                client_xid,
                client_msg.chaddr(),
                "10.5.5.12".parse().unwrap(),
                server_config.get_boot_file().unwrap().as_str(),
                server_config.get_boot_server_ipv4(my_ipv4).unwrap(),
            );
            offer = add_boot_info_to_message(
                offer,
                client_xid,
                server_config.get_boot_file().unwrap().as_str(),
                server_config.get_boot_server_ipv4(my_ipv4).unwrap(),
            );
            let mut opts = offer.opts().clone();
            opts.remove(OptionCode::MessageType);
            opts.insert(DhcpOption::MessageType(MessageType::Ack));
            offer.set_opts(opts);
            offer
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
        trace!(
            "DHCP message ignored due to not matching filter. \
            Required: has_boot_file_name, is_discover, is_request.\
            State: {}, {}, {}",
            has_boot_file_name,
            is_discover,
            is_request
        );
    }

    trace!("Elligible DHCP message found, intercepting.");

    matches
}

async fn relay_message_to_main_dhcp(from: impl ToSocketAddrs, msg: &Message) -> Result<Message> {
    let mut encoder_buff = vec![0u8; 256];
    let mut decoder_buff = vec![0u8; 256];

    let mut encoder = Encoder::new(&mut encoder_buff);

    let mut discover_msg = msg.clone();
    discover_msg.set_xid(rand::thread_rng().gen());

    let mut addr = from.to_socket_addrs().await?.next().unwrap();
    addr.set_port(0);

    let socket = UdpSocket::bind(addr).await?;
    socket.set_broadcast(true)?;
    socket.connect("255.255.255:68").await?;

    discover_msg.encode(&mut encoder)?;

    trace!("Relaying message to the main DHCP server.");
    socket.send(&encoder_buff).await?;
    trace!("Message relayed to the main DHCP server.");

    socket.recv(&mut decoder_buff).await?;
    let mut decoder = Decoder::new(&mut decoder_buff);
    let response = Message::decode(&mut decoder)?;

    trace!("Message received from main DHCP server.");
    Ok(response)
}

fn add_boot_info_to_message(
    msg: Message,
    xid: u32,
    boot_filename: &str,
    tfpt_srv_addr: Ipv4Addr,
) -> Message {
    let mut res = msg.clone();
    let mut opts = msg.opts().clone();

    opts.insert(DhcpOption::BootfileName(boot_filename.as_bytes().to_vec()));
    opts.insert(DhcpOption::TFTPServerAddress(tfpt_srv_addr));
    opts.insert(DhcpOption::ServerIdentifier(tfpt_srv_addr));

    res.set_xid(xid)
        .set_siaddr(tfpt_srv_addr)
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

    //opts.insert(DhcpOption::BootfileName(boot_filename.as_bytes().to_vec()));
    //opts.insert(DhcpOption::TFTPServerAddress(tfpt_srv_addr));
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

// fn make_ack_message(xid: u32) -> Message {
//     let mut res = Message::default();
//     let mut opts = DhcpOptions::default();
//     opts.insert(DhcpOption::MessageType(MessageType::Offer));

//     res.set_xid(xid);
//     res.set_opts(opts);

//     res
// }

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
