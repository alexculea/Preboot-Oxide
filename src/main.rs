#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate phf;

mod conf;
mod dhcp;
mod util;

use crate::conf::{Conf, ProcessEnvConf};
use anyhow::Context;
use async_std::task;
use async_tftp::server::TftpServerBuilder;
use log::{debug, info};
pub type Result<T> = anyhow::Result<T, anyhow::Error>;

fn main() -> Result<()> {
    let mut dot_env_path = std::env::current_exe().unwrap_or_default();
    dot_env_path.set_file_name(".env");

    let _ = dotenv::from_path(dot_env_path);
    let env_prefix = crate::conf::ENV_VAR_PREFIX;
    let log_level = std::env::var(format!("{env_prefix}LOG_LEVEL")).unwrap_or("error".into());

    pretty_env_logger::formatted_timed_builder()
        .parse_filters(&log_level)
        .init();

    let server_config = Conf::from_yaml_config(None).unwrap_or_else(|e| {
        info!("Not loading YAML configuration: {}", e.to_string());
        Conf::from(ProcessEnvConf::from_proccess_env())
    });
    server_config.validate()?;

    if let Some(tftp_path) = server_config.get_tftp_serve_path() {
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

    let result: Result<()> = task::block_on(
        dhcp::server_loop(server_config)
    ).context("Starting DHCP service");

    debug!("Exiting");
    result
}
