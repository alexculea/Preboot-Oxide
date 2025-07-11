use std::{
    fmt::Debug,
    net::{Ipv4Addr, SocketAddrV4},
    os::fd::{AsRawFd, BorrowedFd},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Ok};
use async_std::{future::timeout, net::UdpSocket, sync::RwLock, task};
use derive_builder::Builder;
use log::{debug, error, info, trace};

use crate::conf::ConfEntryRef;
use dhcproto::v4::{
    Decodable, Decoder, DhcpOption, DhcpOptions, Encodable, Encoder, Flags, Message, MessageType,
    Opcode, OptionCode,
};
use network_interface::{Addr, NetworkInterface, NetworkInterfaceConfig};
use polling::{Event, Events, Poller as IOPoller}; // TODO: Migrate to mio
use socket2::{Domain, Protocol, SockAddr, Socket, Type};

use crate::conf::{Conf, MacAddress};
use crate::util::QuotaMap;
use crate::Result;

/// The contention timeout for acquiring a lock on the session map
const LOCK_TIMEOUT: Duration = Duration::from_millis(500);
type SessionMap = QuotaMap<u32, Session>;

struct Session {
    pub client_ip: Option<Ipv4Addr>,
    pub gateway_ip: Option<Ipv4Addr>,
    pub ciaddr: Option<Ipv4Addr>,
    pub subnet: Option<DhcpOption>,
    pub lease_time: Option<DhcpOption>,
    pub start_time: std::time::SystemTime,
    pub discover_message: Option<Message>,
}

pub struct Interface {
    pub iface: NetworkInterface,
    pub client: UdpSocket,
    pub server: UdpSocket,
}

pub struct Interfaces {
    pub interfaces: Vec<Interface>,
}

impl Interfaces {
    pub fn sockets<'a>(&'a self) -> Vec<&'a UdpSocket> {
        self.interfaces
            .iter()
            .map(|iface| vec![&iface.server, &iface.client])
            .collect::<Vec<Vec<&'a UdpSocket>>>()
            .into_iter()
            .flatten()
            .collect()
    }

    pub fn interface_from_event<'a>(&'a self, ev: &Event) -> Option<&'a Interface> {
        let index = ev.key as usize / 2;
        self.interfaces.get(index)
    }

    pub fn socket_from_event<'a>(&'a self, ev: &Event) -> Option<&'a UdpSocket> {
        let sockets = self.sockets();

        Some(sockets[ev.key as usize])
    }
}

impl From<Vec<Interface>> for Interfaces {
    fn from(interfaces: Vec<Interface>) -> Self {
        Self { interfaces }
    }
}

#[derive(Default, Clone)]
struct DhcpMsgWrapper {
    pub msg: Message,
}

impl DhcpMsgWrapper {
    pub fn from_buffer(buffer: &[u8]) -> Result<Self> {
        let mut decoder = Decoder::new(buffer);
        let msg = Message::decode(&mut decoder)?;
        Ok(Self { msg })
    }

    pub fn from_msg(msg: Message) -> Self {
        Self { msg }
    }

    pub fn xid(&self) -> u32 {
        self.msg.xid()
    }

    pub fn yiaddr(&self) -> Ipv4Addr {
        self.msg.yiaddr()
    }

    pub fn subnet_mask(&self) -> Option<&DhcpOption> {
        self.msg.opts().get(OptionCode::SubnetMask)
    }

    pub fn address_lease_time(&self) -> Option<&DhcpOption> {
        self.msg.opts().get(OptionCode::AddressLeaseTime)
    }

    pub fn msg_type(&self) -> Option<MessageType> {
        self.msg.opts().msg_type()
    }

    pub fn requests_boot_info(&self) -> bool {
        match self.msg.opts().get(OptionCode::ParameterRequestList) {
            Some(DhcpOption::ParameterRequestList(params)) => {
                params.contains(&OptionCode::BootfileName)
            }
            _ => false,
        }
    }

    pub fn set_src_ipv4(&mut self, src_ipv4: &Ipv4Addr) {
        let opts = self.msg.opts_mut();
        opts.insert(DhcpOption::ServerIdentifier(*src_ipv4));
        self.msg.set_siaddr(*src_ipv4);
    }

    fn set_boot_info_from_conf(
        &mut self,
        conf: &ConfEntryRef,
        client: &String,
        my_ipv4: Option<&Ipv4Addr>,
    ) -> Result<()> {
        let opts = self.msg.opts_mut();

        let boot_filename = conf.boot_file.as_ref().ok_or(anyhow!(
            "Cannot determine boot file path for client having MAC address: {client}."
        ))?;
        let tfpt_srv_addr = conf.boot_server_ipv4.or(my_ipv4).ok_or(anyhow!(
            "Cannot determine TFTP server IPv4 address for client having MAC address: {client}"
        ))?;

        opts.insert(DhcpOption::BootfileName(boot_filename.as_bytes().to_vec()));
        opts.insert(DhcpOption::TFTPServerAddress(*tfpt_srv_addr));
        opts.insert(DhcpOption::ServerIdentifier(*tfpt_srv_addr));

        self.msg
            .set_siaddr(*tfpt_srv_addr)
            .set_fname_str(boot_filename);

        Ok(())
    }

    pub fn client_mac_address<'a>(&'a self) -> Option<&'a MacAddress> {
        self.msg.chaddr().first_chunk()
    }

    pub fn client_mac_address_str(&self) -> Result<String> {
        Self::client_mac_address_str_from_msg(&self.msg)
    }

    pub fn client_mac_address_str_from_msg(msg: &Message) -> Result<String> {
        let bytes: &[u8; 6] = msg.chaddr().first_chunk().ok_or(anyhow!(
            "The client MAC address does not fit the size requirements of exactly 6 bytes."
        ))?;
        let str_parts: Vec<String> = bytes
            .into_iter()
            .map(|byte| format!("{:0>2X}", byte))
            .collect();
        Ok(str_parts.join(":"))
    }

    pub fn should_handle(&self) -> bool {
        let msg_opts = self.msg.opts();
        let has_boot_file_name = msg_opts.get(OptionCode::BootfileName).is_some();
        let is_request = msg_opts.has_msg_type(MessageType::Request);
        let is_offer = msg_opts.has_msg_type(MessageType::Offer);
        let is_ack = msg_opts.has_msg_type(MessageType::Ack);
        let is_discover = msg_opts.has_msg_type(MessageType::Discover);
        let is_offer_without_boot = !has_boot_file_name && is_offer;

        let matches = is_offer_without_boot | is_request | is_ack | is_discover;
        if !matches {
            debug!(
                "Ignoring DHCP message broadcast, unfulfilled requirements. \
              Expecting any of the following to be true: is_offer_without_boot: {is_offer_without_boot}, is_request: {is_request} \
              is_offer: {is_offer}, is_ack: {is_ack}, is_discover: {is_discover}. Likely this is just regular DHCP chatter on the network which we do not action.",
            );
        }

        matches
    }
}

impl TryInto<serde_json::Value> for DhcpMsgWrapper {
    type Error = serde_json::Error;

    fn try_into(self) -> core::result::Result<serde_json::Value, Self::Error> {
        serde_json::to_value(self.msg)
    }
}

impl Debug for DhcpMsgWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.msg.fmt(f)
    }
}

#[derive(Builder)]
#[builder(build_fn(skip))]
#[builder(setter(skip))]
pub struct DhcpServer {
    interfaces: Option<Arc<Interfaces>>,
    sessions: Arc<RwLock<SessionMap>>,
    io_poller: Arc<IOPoller>,

    #[builder(setter)]
    config: Conf,
}

impl DhcpServerBuilder {
    pub fn build(&mut self) -> Result<DhcpServer> {
        let interfaces = None;
        let config = self
            .config
            .take()
            .ok_or(anyhow!("No server config provided."))?;
        let sessions = Arc::new(RwLock::new(SessionMap::new(config.get_max_sessions())));
        let io_poller = Arc::new(IOPoller::new().context("Setting up OS IO polling.")?);

        // TODO: Decouple DhcpServer from Conf and Interfaces
        Ok(DhcpServer {
            io_poller,
            interfaces,
            sessions,
            config,
        })
    }
}

impl<'a> DhcpServer {
    pub async fn serve(mut self) -> Result<()> {
        self.interfaces = Some(self.get_listen_interfaces()?);
        self.enlist_sockets_for_events()?;
        task::spawn(Self::session_cleaner_entrypoint(Arc::clone(&self.sessions)));

        let instance = Arc::new(self);
        loop {
            let io_poller = Arc::clone(&instance.io_poller);
            let mut events = async_std::task::spawn_blocking(move || {
                let mut events = Events::new();
                io_poller.wait(&mut events, None)?;

                Ok(events)
            })
            .await?; // blocks until we get notified by the OS

            // TODO: Investigate thread safety for re-enlisting sockets
            instance.re_enlist_sockets_for_events()?;

            for event in events.iter() {
                let task_interfaces = Arc::clone(&instance.interfaces.as_ref().unwrap());
                let instance = Arc::clone(&instance);
                task::spawn(async move {
                    let incoming_iface = task_interfaces
                        .interface_from_event(&event)
                        .ok_or(anyhow!(
                            "No interface found for event with key: {}. Very likely a bug.",
                            event.key
                        ))
                        .unwrap();
                    let incoming_socket = task_interfaces
                        .socket_from_event(&event)
                        .ok_or(anyhow!(
                            "No socket found for event with key: {}. Very likely a bug.",
                            event.key
                        ))
                        .unwrap();
                    let _ = instance
                        .handle_dhcp_message(incoming_socket, incoming_iface)
                        .await
                        .map_err(|e| error!("{}", e));
                });
            }

            events.clear();
        }
    }

    async fn handle_dhcp_message(
        &self,
        receiving_socket: &UdpSocket,
        incoming_interface: &Interface,
    ) -> Result<()> {
        let mut rcv_data = [0u8; 576]; // https://www.rfc-editor.org/rfc/rfc1122, 3.3.3 Fragmentation
        let (bytes_read, peer) = receiving_socket.recv_from(&mut rcv_data).await?;
        if bytes_read == 0 {
            return Ok(());
        }

        let receiving_interface = &incoming_interface.iface;
        let self_ipv4: &Ipv4Addr = receiving_interface
            .addr
            .iter()
            .filter(|addr| addr.ip().is_ipv4())
            .take(1)
            .map(|addr| match addr {
                Addr::V4(ipv4) => &ipv4.ip,
                _ => unreachable!(),
            })
            .collect::<Vec<&Ipv4Addr>>()
            .first()
            .context(format!(
                "No IPv4 address found on interface {}",
                receiving_interface.name
            ))?;

        let incoming_msg = DhcpMsgWrapper::from_buffer(&rcv_data[..bytes_read])?;
        let client_xid = incoming_msg.xid();
        let msg_type = incoming_msg.msg_type().ok_or(anyhow!(
            "No message type found in DHCP message with XID: {client_xid}. Skipping.",
        ))?;
        let client_mac_address_str = incoming_msg.client_mac_address_str()?;

        debug!(
            "Received from IP: {} on {}, port: {}, DHCP Msg type: {:?}",
            peer.ip(),
            receiving_interface.name,
            peer.port(),
            msg_type
        );
        trace!("{:#?}", incoming_msg);

        if !incoming_msg.should_handle() {
            return Ok(());
        }

        let reply = match msg_type {
            MessageType::Discover => {
                if !incoming_msg.requests_boot_info() {
                    return Ok(());
                }

                info!(
                    "Received {:?} from client {client_mac_address_str} with XID {client_xid} on interface {}.",
                    msg_type,
                    receiving_interface.name,
                );

                let mut sessions = write_unlock(&self.sessions).await?;
                let mut session = sessions.remove(&client_xid).unwrap_or(Session {
                    client_ip: None,
                    gateway_ip: None,
                    ciaddr: None,
                    subnet: None,
                    lease_time: None,
                    start_time: std::time::SystemTime::now(),
                    discover_message: None,
                });
                session.discover_message = Some(incoming_msg.msg);
                sessions.insert(client_xid, session)?;
                drop(sessions);

                /*
                We will not respond to the discover message until the authoritative
                DHCP server responds first, which it should with an Offer that we
                duplicate below with adding the boot information to the message.
                */
                debug!("Saved message {client_xid} to sessions.");
                debug!("Awaiting for authoritative DHCP server to respond with OFFER to {client_mac_address_str}...");
                return Ok(());
            }
            MessageType::Offer => {
                debug!("External Offer received from DHCP server for {client_mac_address_str} client XID: {client_xid}.");
                let mut sessions = write_unlock(&self.sessions).await?;
                let session = sessions.get_mut(&client_xid);
                if session.is_none() {
                    debug!(
                        "No session with XID: {client_xid}. Most likely regular DHCP on the network. Ignoring.",
                    );
                    return Ok(());
                }

                let session = session.unwrap();
                session.client_ip = Some(incoming_msg.yiaddr());
                session.subnet = incoming_msg.subnet_mask().cloned();
                session.lease_time = incoming_msg.address_lease_time().cloned();
                session.gateway_ip = incoming_msg.msg.giaddr().into();
                session.ciaddr = incoming_msg.msg.ciaddr().into();

                let initial_discover_msg = session.discover_message.clone().ok_or(anyhow!(
                    "Initial discovery message for XID {client_xid} not found due to either a bug or incorrect DHCP server behavior. Skipping.",
                ))?;
                drop(sessions);

                let discover_msg_doc = serde_json::to_value(initial_discover_msg)?;
                let client_cfg = self.config.get_from_doc(discover_msg_doc)?.ok_or(anyhow!(
                    "No configuration found for client {client_mac_address_str}. Skipping",
                ))?;

                let mut response = incoming_msg.clone();
                response.set_src_ipv4(&self_ipv4);
                response.set_boot_info_from_conf(
                    &client_cfg,
                    &client_mac_address_str,
                    Some(&self_ipv4),
                )?;

                info!(
                    "Responding with boot information to client {client_mac_address_str}, boot file: {}, server: {}, client XID: {client_xid}.",
                    client_cfg.boot_file.unwrap_or(&"None".to_string()),
                    client_cfg.boot_server_ipv4
                        .map(|ip| ip.to_string())
                        .unwrap_or(self_ipv4.to_string()),
                );

                response.msg
            }
            MessageType::Request => {
                let sessions = read_unlock(&self.sessions).await?;
                let session = sessions.get(&client_xid);
                if session.is_none() {
                    debug!("No session found for client {client_mac_address_str}, XID: {client_xid}, ignoring.");
                    return Ok(());
                }
                let session = session.unwrap();
                let mut ack = Message::default();
                let mut opts = DhcpOptions::default();
                opts.insert(DhcpOption::MessageType(MessageType::Ack));
                opts.insert(
                    session
                        .subnet
                        .clone()
                        .unwrap_or(DhcpOption::SubnetMask(Ipv4Addr::new(255, 255, 255, 0))),
                );
                opts.insert(
                    session
                        .lease_time
                        .clone()
                        .unwrap_or(DhcpOption::AddressLeaseTime(60)),
                ); // in minutes

                ack.set_flags(Flags::new(0).set_broadcast())
                    .set_yiaddr(session.client_ip.unwrap_or(Ipv4Addr::new(0, 0, 0, 0)))
                    .set_giaddr(session.gateway_ip.unwrap_or(Ipv4Addr::new(0, 0, 0, 0)))
                    .set_ciaddr(session.ciaddr.unwrap_or(Ipv4Addr::new(0, 0, 0, 0)))
                    .set_opcode(Opcode::BootReply)
                    .set_opts(opts)
                    .set_chaddr(
                        incoming_msg
                            .client_mac_address()
                            .ok_or(anyhow!("Cannot determine client MAC address."))?,
                    )
                    .set_xid(client_xid);
                drop(sessions);

                let client_cfg =
                    self.config
                        .get_from_doc(incoming_msg.try_into()?)?
                        .ok_or(anyhow!(
                            "No configuration found for client {client_mac_address_str}. Skipping",
                        ))?;

                let mut response = DhcpMsgWrapper::from_msg(ack);
                response.set_src_ipv4(&self_ipv4);
                response.set_boot_info_from_conf(
                    &client_cfg,
                    &client_mac_address_str,
                    Some(&self_ipv4),
                )?;

                response.msg
            }
            MessageType::Decline | MessageType::Ack => {
                let mut sessions = write_unlock(&self.sessions).await?;
                sessions.remove(&client_xid);
                drop(sessions);
                debug!("Session for XID: {client_xid} ended.");

                return if msg_type == MessageType::Decline {
                    bail!("Client {client_mac_address_str} declined REQUEST.")
                } else {
                    Ok(())
                };
            }
            _ => return Ok(()),
        };

        
        let iface_name = &receiving_interface.name;
        let to_addr = "255.255.255.255:68";
        let mut buf = Vec::new();
        let mut encoder = Encoder::new(&mut buf);
        reply.encode(&mut encoder)?;

        let reply_mac = DhcpMsgWrapper::client_mac_address_str_from_msg(&reply)
            .unwrap_or_else(|_| "Unknown".to_string());
        let reply_type = reply.opts().msg_type().unwrap_or(MessageType::Unknown(0));
        debug!("Responding with {:?} on interface {iface_name} to client {reply_mac}.", reply_type);
        trace!("{:#?}", reply);

        let socket = &incoming_interface.server;
        socket.send_to(&buf, to_addr).await?;

        Ok(())
    }

    async fn session_cleaner_entrypoint(active_sessions: Arc<RwLock<SessionMap>>) {
        loop {
            task::sleep(Duration::from_secs(10)).await;
            let now = std::time::SystemTime::now();
            let mut items_to_remove = Vec::with_capacity(50);
            let sessions = read_unlock(&active_sessions).await;
            if sessions.is_err() {
                debug!("Session cleaner could not acquire read lock. Skipping.");
                debug!("{:?}", sessions.err().unwrap());
                continue;
            }
            let sessions = sessions.unwrap();

            for (_, (client_xid, session)) in sessions.iter().enumerate() {
                if let Some(age) = now.duration_since(session.start_time).ok() {
                    if age > Duration::from_secs(30) {
                        items_to_remove.push(*client_xid); 
                        if session.client_ip.is_none() {
                            info!(
                                "Session for client XID: {} timed out after {} seconds. Was expecting IP information from an authoritative DHCP server. Is there a DHCP service running on the network?",
                                client_xid,
                                age.as_secs()
                            );
                        } else {
                            info!(
                                "Session for client XID: {} timed out after {} seconds. Was expecting client to follow up with a Request on the offer made.",
                                client_xid,
                                age.as_secs(),
                            );
                        }

                    }
                }
            }
            drop(sessions); // unlock the RwLock
            if items_to_remove.is_empty() {
                continue;
            }

            let sessions = write_unlock(&active_sessions).await;

            if sessions.is_err() {
                debug!("Session cleaner could not acquire write lock. Skipping.");
                debug!("{:?}", sessions.err().unwrap());
                continue;
            }
            let mut sessions = sessions.unwrap();

            sessions.retain(|client_xid, _| !items_to_remove.contains(&client_xid));
            drop(sessions); // unlock the RwLock
                            // would have been dropped anyway at the end of the loop
                            // but best to keep awareness of this happing to avoid deadlocks

            trace!(
                "Session cleaner removed {} timed out sessions.",
                items_to_remove.len()
            );
        }
    }

    fn get_listen_interfaces(&self) -> Result<Arc<Interfaces>> {
        let listen_ips = ["0.0.0.0:67", "255.255.255.255:68"];
        let network_interfaces = NetworkInterface::show()
            .context("Listing network interfaces")?
            .into_iter()
            .filter(|iface| {
                // only listen on the configured network interfaces
                self.config
                    .get_ifaces()
                    .map(|ifaces| ifaces.contains(&iface.name))
                    .unwrap_or(true) // or on all if no interfaces are configured
            })
            .collect::<Vec<NetworkInterface>>();

        Ok(Arc::new(
            network_interfaces
                .iter()
                .map(|iface| {
                    let server = socket_from_iface_ip(iface, &listen_ips[0])?;
                    let client = socket_from_iface_ip(iface, &listen_ips[1])?;
                    Ok(Interface {
                        iface: iface.clone(),
                        client,
                        server,
                    })
                })
                .collect::<Result<Vec<Interface>>>()?
                .into(),
        ))
    }

    fn enlist_sockets_for_events(&self) -> Result<()> {
        self.interfaces
            .as_ref()
            .unwrap()
            .sockets()
            .iter()
            .enumerate()
            .map(|(index, socket)| {
                // SAFETY: sources have to be deleted before the poller is dropped
                unsafe { self.io_poller.add(*socket, polling::Event::readable(index)) }
            })
            .collect::<std::io::Result<()>>()?;
        Ok(())
    }

    fn re_enlist_sockets_for_events(&self) -> Result<()> {
        self.interfaces
            .as_ref()
            .unwrap()
            .sockets()
            .iter()
            .enumerate()
            .map(|(index, socket)| {
                unsafe {
                    // SAFETY: The resource pointed to by fd must remain open for the duration of the returned BorrowedFd, and it must not have the value -1.
                    let fd = BorrowedFd::borrow_raw(socket.as_raw_fd());

                    // SAFETY: sources have to be deleted before the poller is dropped
                    self.io_poller.modify(fd, polling::Event::readable(index))
                }
            })
            .collect::<std::io::Result<()>>()?;
        Ok(())
    }
}

fn socket_from_iface_ip(iface: &NetworkInterface, ip: &&str) -> Result<UdpSocket> {
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
    Ok(socket2_to_async_std(socket))
}

fn socket2_to_async_std(socket: Socket) -> UdpSocket {
    let std_socket = Into::<std::net::UdpSocket>::into(socket);
    UdpSocket::from(std_socket)
}

async fn read_unlock<'a, T>(
    lock: &'a RwLock<T>,
) -> core::result::Result<async_std::sync::RwLockReadGuard<'a, T>, async_std::future::TimeoutError>
{
    timeout(LOCK_TIMEOUT, lock.read()).await
}

async fn write_unlock<'a, T>(
    lock: &'a RwLock<T>,
) -> core::result::Result<async_std::sync::RwLockWriteGuard<'a, T>, async_std::future::TimeoutError>
{
    timeout(LOCK_TIMEOUT, lock.write()).await
}
