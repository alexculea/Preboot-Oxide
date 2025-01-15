#[macro_use]
extern crate anyhow;

use std::env;

use anyhow::Context;
use async_std::task;
use log::{debug, info};
use single_instance::SingleInstance;

use preboot_oxide::{
    cli,
    conf::{Conf, ProcessEnvConf, ENV_VAR_PREFIX},
    dhcp::DhcpServerBuilder,
    tftp::spawn_tftp_service_async,
    Result,
};

fn main() -> Result<()> {
    let arg_log_level = cli::parse();
    let instance = SingleInstance::new("preboot-oxide")?;
    if !instance.is_single() {
        bail!("Another instance is already running");
    }
    let mut dot_env_path = env::current_exe().unwrap_or_default();
    dot_env_path.set_file_name(".env");

    let _ = dotenv::from_path(dot_env_path);

    let log_level = arg_log_level
        .or(env::var(format!("{ENV_VAR_PREFIX}LOG_LEVEL")).ok())
        .unwrap_or("error".into());

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    let conf_path = env::var(format!("{ENV_VAR_PREFIX}CONF_PATH"))
        .map(std::path::PathBuf::from)
        .ok();
    let server_config = Conf::from_yaml_config(conf_path.as_ref()).unwrap_or_else(|e| {
        info!(
            "Not loading YAML configuration: {}\nFalling back to environment variables.",
            e.to_string()
        );
        Conf::from(ProcessEnvConf::from_process_env())
    });
    server_config.validate()?;
    spawn_tftp_service_async(&server_config)?;

    let dhcp_server = DhcpServerBuilder::default()
        .config(server_config)
        .build()?;
    let result: Result<()> = task::block_on(dhcp_server.serve()).context("Starting DHCP service");

    debug!("Exiting");
    result
}
