#[macro_use]
extern crate anyhow;

use anyhow::Context;
use async_std::task;
use clap::Parser;
use log::{debug, info};
use single_instance::SingleInstance;

use preboot_oxide::{
    conf::{Conf, ENV_VAR_PREFIX, ProcessEnvConf},
    dhcp,
    tftp::spawn_tftp_service_async,
    cli::Cli,
    Result,
};

fn main() -> Result<()> {
    Cli::parse();
    let instance = SingleInstance::new("preboot-oxide")?;
    if !instance.is_single() {
        return Err(anyhow!("Another instance is already running"));
    }
    let mut dot_env_path = std::env::current_exe().unwrap_or_default();
    dot_env_path.set_file_name(".env");

    let _ = dotenv::from_path(dot_env_path);
    let log_level = std::env::var(format!("{ENV_VAR_PREFIX}LOG_LEVEL")).unwrap_or("error".into());

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level))
        // .parse_filters(&log_level)
        .init();

    let conf_path = std::env::var(format!("{ENV_VAR_PREFIX}CONF_PATH"))
        .map(std::path::PathBuf::from)
        .ok();
    let server_config = Conf::from_yaml_config(conf_path.as_ref())
        .unwrap_or_else(|e| {
            info!("Not loading YAML configuration: {}\nFalling back to environment variables.", e.to_string());
            Conf::from(ProcessEnvConf::from_process_env())
        });
    server_config.validate()?;
    spawn_tftp_service_async(&server_config)?;

    let result: Result<()> =
        task::block_on(dhcp::server_loop(server_config)).context("Starting DHCP service");

    debug!("Exiting");
    result
}
