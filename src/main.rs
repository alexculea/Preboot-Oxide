#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate phf;

mod conf;
mod dhcp;
mod tftp;
mod util;

use crate::{
    conf::{Conf, ProcessEnvConf},
    tftp::spawn_tftp_service_async,
};
use anyhow::Context;
use async_std::task;
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

    let conf_path = std::env::var(format!("{env_prefix}CONF_PATH"))
        .map(std::path::PathBuf::from)
        .ok();
    let server_config = Conf::from_yaml_config(conf_path.as_ref()).unwrap_or_else(|e| {
        info!("Not loading YAML configuration: {}", e.to_string());
        Conf::from(ProcessEnvConf::from_process_env())
    });
    server_config.validate()?;
    spawn_tftp_service_async(&server_config)?;

    let result: Result<()> =
        task::block_on(dhcp::server_loop(server_config)).context("Starting DHCP service");

    debug!("Exiting");
    result
}
