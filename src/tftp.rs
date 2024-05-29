use std::net::{Ipv4Addr, SocketAddr};

use anyhow::{Context, Error};
use async_std::task;
use async_tftp::server::TftpServerBuilder;
use log::info;
use network_interface::{Addr, NetworkInterface, NetworkInterfaceConfig};

use crate::conf::Conf;
use crate::Result;

pub fn spawn_tftp_service_async(conf: &Conf) -> Result<()> {
    if let Some(tftp_path) = conf.get_tftp_serve_path() {
        let network_interfaces = NetworkInterface::show().context("Listing network interfaces")?;
        let listen_ips: Vec<Ipv4Addr> = network_interfaces
            .iter()
            .filter(|iface| {
                // only listen on the configured network interfaces
                conf.get_ifaces()
                    .map(|ifaces| ifaces.contains(&iface.name))
                    .unwrap_or(true) // or on all if no interfaces are configured
            })
            .map(|iface| {
                iface
                    .addr
                    .iter()
                    .filter_map(|ip| match ip {
                        Addr::V4(v4) => Some(v4.ip),
                        Addr::V6(_) => None,
                    })
                    .collect::<Vec<_>>()
            })
            .flatten()
            .collect();
        for ip in listen_ips {
            let tftp_dir = tftp_path.clone();
            task::spawn(async move {
                let mut tftp_builder = TftpServerBuilder::with_dir_ro(tftp_dir.clone())?;
                tftp_builder = tftp_builder.bind(SocketAddr::new(ip.into(), 69));
                let server = tftp_builder.build().await?;

                info!("TFTP server started on {ip}:69 path: {tftp_dir}");
                server.serve().await?;
                async_tftp::Result::<(), Error>::Ok(())
            });
        }
    } else {
        info!("TFTP server not started, no path configured.");
    }

    Ok(())
}
