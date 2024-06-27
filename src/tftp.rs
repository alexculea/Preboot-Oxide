use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Component;
use std::path::{Path, PathBuf};

use anyhow::{Context, Error};
use async_std::task;
use async_tftp::{async_trait, packet, server::TftpServerBuilder, Error as TftpError};
use log::info;
use network_interface::{Addr, NetworkInterface, NetworkInterfaceConfig};

use crate::conf::Conf;
use crate::Result;

use async_std::fs::File;
use log::trace;

type TftpResult<T, E = TftpError> = std::result::Result<T, E>;

pub fn spawn_tftp_service_async(conf: &Conf) -> Result<()> {
    if let Some(tftp_path) = conf.get_tftp_serve_path() {
        let dir = Path::new(&tftp_path);
        if !dir.exists() || !dir.is_dir() {
            return Err(anyhow!(
                "TFTP path does not exist or is not directory: {:?}",
                dir
            ));
        }

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
                let mut tftp_builder = TftpServerBuilder::with_handler(DirHandler::new(
                    tftp_dir.clone(),
                    DirHandlerMode::ReadOnly,
                )?);
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

/// Handler that serves read requests for a directory.
pub struct DirHandler {
    dir: PathBuf,
    serve_rrq: bool,
    serve_wrq: bool,
}


#[allow(unused)]
pub enum DirHandlerMode {
    /// Serve only read requests.
    ReadOnly,
    /// Serve only write requests.
    WriteOnly,
    /// Server read and write requests.
    ReadWrite,
}

impl DirHandler {
    /// Create new handler for directory.
    pub fn new<P>(dir: P, flags: DirHandlerMode) -> TftpResult<Self>
    where
        P: AsRef<Path>,
    {
        let dir = std::fs::canonicalize(dir.as_ref())?;

        if !dir.is_dir() {
            return Err(TftpError::NotDir(dir));
        }

        trace!("TFTP directory: {}", dir.display());

        let serve_rrq = match flags {
            DirHandlerMode::ReadOnly => true,
            DirHandlerMode::WriteOnly => false,
            DirHandlerMode::ReadWrite => true,
        };

        let serve_wrq = match flags {
            DirHandlerMode::ReadOnly => false,
            DirHandlerMode::WriteOnly => true,
            DirHandlerMode::ReadWrite => true,
        };

        Ok(DirHandler {
            dir,
            serve_rrq,
            serve_wrq,
        })
    }
}

#[async_trait]
impl async_tftp::server::Handler for DirHandler {
    type Reader = File;
    type Writer = File;

    async fn read_req_open(
        &mut self,
        _client: &SocketAddr,
        path: &Path,
    ) -> TftpResult<(Self::Reader, Option<u64>), packet::Error> {
        if !self.serve_rrq {
            return Err(packet::Error::IllegalOperation);
        }

        let path = secure_path(&self.dir, path)?;

        // Send only regular files
        if !path.is_file() {
            return Err(packet::Error::FileNotFound);
        }

        let (reader, len) = open_file_ro(path.clone()).await?;
        trace!("TFTP sending file: {}", path.display());

        Ok((reader, len))
    }

    async fn write_req_open(
        &mut self,
        _client: &SocketAddr,
        path: &Path,
        size: Option<u64>,
    ) -> TftpResult<Self::Writer, packet::Error> {
        if !self.serve_wrq {
            return Err(packet::Error::IllegalOperation);
        }

        let path = secure_path(&self.dir, path)?;

        let path_clone = path.clone();
        let file = open_file_wo(path_clone, size).await?;
        // let writer = Unblock::new(file);

        trace!("TFTP receiving file: {}", path.display());

        Ok(file)
    }
}

fn secure_path(restricted_dir: &Path, path: &Path) -> TftpResult<PathBuf, packet::Error> {
    // Strip `/` and `./` prefixes
    let path = path
        .strip_prefix("/")
        .or_else(|_| path.strip_prefix("./"))
        .unwrap_or(path);

    // Avoid directory traversal attack by filtering `../`.
    if path.components().any(|x| x == Component::ParentDir) {
        return Err(packet::Error::PermissionDenied);
    }

    // Path should not start from root dir or have any Windows prefixes.
    // i.e. We accept only normal path components.
    match path.components().next() {
        Some(Component::Normal(_)) => {}
        _ => return Err(packet::Error::PermissionDenied),
    }

    Ok(restricted_dir.join(path))
}

async fn open_file_ro(path: PathBuf) -> io::Result<(File, Option<u64>)> {
    let file = async_std::fs::File::open(path).await?;
    let len = file.metadata().await.ok().map(|m| m.len());
    Ok((file, len))
}

async fn open_file_wo(path: PathBuf, size: Option<u64>) -> io::Result<File> {
    let file = async_std::fs::File::create(path).await?;

    if let Some(size) = size {
        file.set_len(size).await?;
    }

    Ok(file)
}
