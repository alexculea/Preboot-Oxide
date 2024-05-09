mod conf;
mod util;

#[macro_use]
extern crate anyhow;

use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddrV4},
    os::fd::{AsRawFd, BorrowedFd},
    sync::Arc,
};

use async_std::sync::Mutex;
use async_std::{net::UdpSocket, task};

use anyhow::{Context, Ok};
use log::{debug, error, info, trace};

use dhcproto::v4::{
    Decodable, Decoder, DhcpOption, DhcpOptions, Encodable, Encoder, Flags, Message, MessageType,
    Opcode, OptionCode,
};
use network_interface::{Addr, NetworkInterface, NetworkInterfaceConfig};
use polling::{Events, Poller};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};

use async_tftp::server::TftpServerBuilder;

use crate::conf::Conf;

struct Session {
    pub client_ip: Option<Ipv4Addr>,
    pub subnet: DhcpOption,
    pub lease_time: DhcpOption,
}

type SessionMap = HashMap<u32, Session>;

type Result<T> = anyhow::Result<T, anyhow::Error>;

type SocketVec2DRes = Result<Vec<Vec<Socket>>>;

async fn dhcp_server_loop(server_config: Conf) -> Result<()> {
    let listen_ips = ["0.0.0.0:67", "255.255.255.255:68"];
    let sessions = Arc::new(Mutex::new(Default::default()));
    let network_interfaces = NetworkInterface::show().context("Listing network interfaces")?;
    let sockets: Arc<Vec<UdpSocket>> = Arc::new(
        network_interfaces
            .iter()
            .filter(|iface| {
                // only listen on the configured network interfaces
                server_config
                    .get_ifaces()
                    .map(|ifaces| ifaces.contains(&iface.name))
                    .unwrap_or(true) // or on all if no interfaces are configured
            })
            .map(|iface| {
                listen_ips
                    .iter()
                    .map(|ip| {
                        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
                        socket.set_broadcast(true)?;
                        socket
                            .bind_device(Some(iface.name.as_bytes()))
                            .context(format!("Binding socket to network device: {}", iface.name))?;
                        socket
                            .bind(&SockAddr::from(ip.parse::<SocketAddrV4>()?))
                            .context(format!(
                                "Binding socket to {ip} on network device: {}",
                                iface.name
                            ))?;

                        info!("Listening on IP {ip} on device {}", iface.name);
                        Ok(socket)
                    })
                    .collect()
            })
            .collect::<SocketVec2DRes>()
            .context("Setting up network sockets on devices.")?
            .into_iter()
            .flatten()
            .map(|socket| socket2_to_async_std(socket))
            .collect(),
    );

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
            let network_interfaces = network_interfaces.clone();
            task::spawn(async move {
                let _ = handle_dhcp_message(
                    event.key,
                    task_sockets,
                    network_interfaces,
                    &server_config,
                    sessions,
                )
                .await
                .map_err(|e| error!("{}", e));
            });
        }

        events.clear();
    }
}

async fn handle_dhcp_message(
    receiving_socket_index: usize,
    sockets: Arc<Vec<UdpSocket>>,
    network_interfaces: Vec<NetworkInterface>,
    server_config: &Conf,
    sessions: Arc<Mutex<SessionMap>>,
) -> Result<()> {
    let receiving_socket = &sockets[receiving_socket_index];
    let mut rcv_data = [0u8; 576]; // https://www.rfc-editor.org/rfc/rfc1122, 3.3.3 Fragmentation
    let (bytes_read, peer) = receiving_socket.recv_from(&mut rcv_data).await?;
    if bytes_read == 0 {
        return Ok(());
    }

    let receiving_interface_index = (receiving_socket_index / 2) as usize; // 2 sockets per interface
    let receiving_interface = &network_interfaces[receiving_interface_index];
    let self_ipv4: Ipv4Addr = receiving_interface
        .addr
        .iter()
        .filter(|addr| addr.ip().is_ipv4())
        .take(1)
        .map(|addr| match addr {
            Addr::V4(ipv4) => ipv4.ip,
            _ => unreachable!(),
        })
        .collect::<Vec<Ipv4Addr>>()
        .first()
        .context(format!(
            "No IPv4 address found on interface {}",
            receiving_interface.name
        ))?
        .clone();

    let client_msg = Message::decode(&mut Decoder::new(&rcv_data))?;
    let client_xid = client_msg.xid();
    let opts = client_msg.opts();
    let msg_type = opts.msg_type().context("No message type found")?;

    debug!(
        "Received from IP: {}, port: {}, DHCP Msg: {:?}",
        peer.ip(),
        peer.port(),
        msg_type
    );
    trace!("{}", client_msg.to_string());

    if !matches_filter(&client_msg) {
        return Ok(());
    }

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

            let msg = apply_self_to_message(&client_msg, &self_ipv4);
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

            ack = apply_self_to_message(&ack, &self_ipv4);
            ack = add_boot_info_to_message(&ack, server_config, Some(&self_ipv4));

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

    for socket in sockets.iter() {
        socket.send_to(&buf, to_addr).await?;
        debug!(
            "DHCP reply ({:?}) sent to: {}",
            response.opts().get(OptionCode::MessageType).unwrap(),
            to_addr
        );
    }

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

fn add_boot_info_to_message(msg: &Message, config: &Conf, my_ipv4: Option<&Ipv4Addr>) -> Message {
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

fn apply_self_to_message(msg: &Message, my_ipv4: &Ipv4Addr) -> Message {
    let mut res = msg.clone();
    let mut opts = msg.opts().clone();
    opts.insert(DhcpOption::ServerIdentifier(my_ipv4.clone()));
    res.set_siaddr(my_ipv4.clone()).set_opts(opts);

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

    if let Some(tftp_path) = server_config.get_tftp_path() {
        let tftp_dir = tftp_path.clone();
        task::spawn(async {
            let tftpd = TftpServerBuilder::with_dir_ro(tftp_path)
                .unwrap()
                .build()
                .await
                .unwrap();
            tftpd.serve().await.unwrap();
        });
        info!("TFTP server started on path: {}", tftp_dir);
    } else {
        info!("TFTP server not started, no path configured.");
    }

    let result = task::block_on(dhcp_server_loop(server_config))
        .context(format!("Setting up network sockets"));

    debug!("Exiting");
    result
}
