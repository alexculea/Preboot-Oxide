use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddrV4},
    os::fd::{AsRawFd, BorrowedFd},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Ok};
use async_std::sync::RwLock;
use async_std::{net::UdpSocket, task};
use log::{debug, error, info, trace};

use crate::util::bytes_to_mac_address;
use dhcproto::v4::{
    Decodable, Decoder, DhcpOption, DhcpOptions, Encodable, Encoder, Flags, Message, MessageType,
    Opcode, OptionCode,
};
use network_interface::{Addr, NetworkInterface, NetworkInterfaceConfig};
use polling::{Events, Poller as IOPoller};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};

use crate::conf::{Conf, MacAddress};
use crate::Result;

struct Session {
    pub client_ip: Option<Ipv4Addr>,
    pub subnet: DhcpOption,
    pub lease_time: DhcpOption,
    pub start_time: std::time::SystemTime,
}

type SessionMap = HashMap<u32, Session>;
type SocketVec2DRes = Result<Vec<Vec<Socket>>>;

pub async fn server_loop(server_config: Conf) -> Result<()> {
    let listen_ips = ["0.0.0.0:67", "255.255.255.255:68"];
    let sessions = Arc::new(RwLock::new(Default::default()));
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
                    .map(|ip| socket_from_iface_ip(iface, ip))
                    .collect()
            })
            .collect::<SocketVec2DRes>()
            .context("Setting up network sockets on devices.")?
            .into_iter()
            .flatten()
            .map(|socket| socket2_to_async_std(socket))
            .collect(),
    );

    start_session_cleaner(Arc::clone(&sessions));

    let poller = IOPoller::new().context("Setting up OS IO polling.")?;
    enlist_sockets_for_events(&poller, &sockets)?;

    let mut events = Events::new();
    loop {
        let _ = poller.wait(&mut events, None)?; // blocks until we get notified by the OS
        re_enlist_sockets_for_events(&sockets, &poller)?;

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

fn start_session_cleaner(active_sessions: Arc<RwLock<SessionMap>>) {
    task::spawn(async move {
        loop {
            task::sleep(Duration::from_secs(60)).await;
            let now = std::time::SystemTime::now();
            let mut items_to_remove = Vec::with_capacity(50);
            let sessions = active_sessions.read().await;
            for (_, (client_xid, session)) in sessions.iter().enumerate() {
                if let Some(age) = now.duration_since(session.start_time).ok() {
                    if age > Duration::from_secs(120) {
                        items_to_remove.push(client_xid);
                    }
                }
            }

            if items_to_remove.is_empty() {
                continue;
            }

            let mut sessions = active_sessions.write().await;
            sessions.retain(|client_xid, _| !items_to_remove.contains(&client_xid));
            drop(sessions); // unlock the RwLock
                            // would have been dropped anyway at the end of the loop
                            // but best to keep awareness of this happing to avoid deadlocks
        }
    });
}

fn enlist_sockets_for_events(poller: &IOPoller, sockets: &Arc<Vec<UdpSocket>>) -> Result<()> {
    sockets
        .iter()
        .enumerate()
        .map(|(index, socket)| {
            // SAFETY: sources have to be deleted before the poller is dropped
            unsafe { poller.add(socket, polling::Event::readable(index)) }
        })
        .collect::<std::io::Result<()>>()?;
    Ok(())
}

fn re_enlist_sockets_for_events(sockets: &Arc<Vec<UdpSocket>>, poller: &IOPoller) -> Result<()> {
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
    Ok(())
}

fn socket_from_iface_ip(iface: &NetworkInterface, ip: &&str) -> Result<Socket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_broadcast(true)?;
    socket
        .bind_device(Some(iface.name.as_bytes()))
        .context(format!("Binding socket to network device: {}", iface.name))?;
    socket.set_reuse_port(true)?;
    socket.set_reuse_address(true)?;
    socket
        .bind(&SockAddr::from(ip.parse::<SocketAddrV4>()?))
        .context(format!(
            "Binding socket to {ip} on network device: {}",
            iface.name
        ))?;

    info!("Listening on IP {ip} on device {}", iface.name);
    Ok(socket)
}

async fn handle_dhcp_message(
    receiving_socket_index: usize,
    sockets: Arc<Vec<UdpSocket>>,
    network_interfaces: Vec<NetworkInterface>,
    server_config: &Conf,
    sessions: Arc<RwLock<SessionMap>>,
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
    trace!("{:#?}", client_msg);

    if !matches_filter(&client_msg) {
        return Ok(());
    }

    let client_mac_address = client_msg.chaddr().first_chunk().ok_or(anyhow!(
        "The client MAC address does not fit the size requirements of exactly 6 bytes."
    ))?;

    let response = match msg_type {
        MessageType::Offer => {
            let mut sessions = sessions.write().await;
            let max_sessions = server_config.get_max_sessions();
            if u64::try_from(sessions.len())? > max_sessions {
                bail!("Max sessions of {max_sessions} reached. Ignoring.")
            }

            sessions.insert(
                client_msg.xid(),
                Session {
                    start_time: std::time::SystemTime::now(),
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
            add_boot_info_to_message(&msg, server_config, client_mac_address, Some(&self_ipv4))?
        }
        MessageType::Request => {
            let sessions = sessions.read().await;
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
                .set_chaddr(client_mac_address)
                .set_xid(client_msg.xid());
            drop(sessions);

            ack = apply_self_to_message(&ack, &self_ipv4);
            ack = add_boot_info_to_message(
                &ack,
                server_config,
                client_mac_address,
                Some(&self_ipv4),
            )?;

            ack
        }
        MessageType::Decline | MessageType::Ack => {
            let mut sessions = sessions.write().await;
            sessions.remove(&client_xid);
            drop(sessions);

            return if msg_type == MessageType::Decline {
                bail!(
                    "Client {} declined REQUEST.",
                    bytes_to_mac_address(client_msg.chaddr())
                )
            } else {
                Ok(())
            };
        }
        _ => return Ok(()),
    };

    let to_addr = "255.255.255.255:68";
    let mut buf = Vec::new();
    let mut e = Encoder::new(&mut buf);
    response.encode(&mut e)?;

    trace!("Responding with message: {:#?}", response);

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

    trace!("Eligible DHCP message found, intercepting.");

    matches
}

fn socket2_to_async_std(socket: Socket) -> UdpSocket {
    let std_socket = Into::<std::net::UdpSocket>::into(socket);
    UdpSocket::from(std_socket)
}

fn add_boot_info_to_message(
    msg: &Message,
    config: &Conf,
    client: &MacAddress,
    my_ipv4: Option<&Ipv4Addr>,
) -> Result<Message> {
    let mut res = msg.clone();
    let mut opts = msg.opts().clone();

    let boot_filename = config.get_boot_file(client).ok_or(anyhow!(
        "Cannot determine boot file path for client having MAC address: {}.",
        bytes_to_mac_address(client)
    ))?;
    let tfpt_srv_addr = config.get_boot_server_ipv4(my_ipv4, client).ok_or(anyhow!(
        "Cannot determine TFTP server IPv4 address for client having MAC address: {}",
        bytes_to_mac_address(client)
    ))?;

    opts.insert(DhcpOption::BootfileName(boot_filename.as_bytes().to_vec()));
    opts.insert(DhcpOption::TFTPServerAddress(tfpt_srv_addr));
    opts.insert(DhcpOption::ServerIdentifier(tfpt_srv_addr));

    res.set_siaddr(tfpt_srv_addr)
        .set_fname_str(boot_filename)
        .set_opts(opts);

    return Ok(res);
}

fn apply_self_to_message(msg: &Message, my_ipv4: &Ipv4Addr) -> Message {
    let mut res = msg.clone();
    let mut opts = msg.opts().clone();
    opts.insert(DhcpOption::ServerIdentifier(my_ipv4.clone()));
    res.set_siaddr(my_ipv4.clone()).set_opts(opts);

    res
}
