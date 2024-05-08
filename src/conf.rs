use anyhow::Result;
use std::net::Ipv4Addr;

#[derive(Default, Clone, Debug)]
pub struct Conf {
    boot_file: Option<String>,
    boot_server_ipv4: Option<Ipv4Addr>,
    ifaces: Option<Vec<String>>,
}

pub const ENV_VAR_PREFIX: &str = "PO_";

impl Conf {
    fn new() -> Self {
        Default::default()
    }

    pub fn validate(&self) -> Result<()> {
        if self.boot_file.is_none() {
            bail!("No path to the boot file was configured.")
        }

        if self.boot_server_ipv4.is_none() {
            bail!("No self IPv4 was configured")
        }
        Ok(())
    }

    pub fn from_proccess_env() -> Result<Self> {
        let boot_server_ipv4: Option<Ipv4Addr> =
            std::env::var(format!("{ENV_VAR_PREFIX}SERVER_IPV4"))
                .unwrap_or_default()
                .parse()
                .ok();
        let boot_file = std::env::var(format!("{ENV_VAR_PREFIX}BOOT_FILE")).ok();
        let ifaces_csv = std::env::var(format!("{ENV_VAR_PREFIX}IFACES")).ok();
        let ifaces = ifaces_csv.map(|csv| csv.split(",").map(|s| s.to_string()).collect());

        Ok(Self {
            boot_server_ipv4,
            boot_file,
            ifaces,
        })
    }

    pub fn get_boot_file(&self) -> Option<String> {
        self.boot_file.clone()
    }

    pub fn get_boot_server_ipv4(&self, self_ip_v4: Option<Ipv4Addr>) -> Option<Ipv4Addr> {
        if self.boot_server_ipv4.is_some() {
            return self.boot_server_ipv4;
        } else {
            return self_ip_v4;
        }
    }

    pub fn get_ifaces(&self) -> Option<&Vec<String>> {
        self.ifaces.as_ref()
    }
}
