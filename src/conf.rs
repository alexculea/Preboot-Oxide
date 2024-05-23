use anyhow::Result;
use std::{
    collections::HashMap,
    io::Read,
    net::Ipv4Addr,
    path::{Path, PathBuf},
    str::FromStr,
};
use yaml_rust2::Yaml;

use crate::util::mac_address_to_bytes;

pub type MacAddress = [u8; 6];
pub type MacAddressConfigMap = HashMap<MacAddress, Option<ConfEntry>>;
pub type ArchConfigMap = HashMap<u16, ConfEntry>;

#[derive(Default, Clone, Debug)]
pub struct Conf {
    default: Option<ConfEntry>,
    ifaces: Option<Vec<String>>,
    mac_file_map: Option<MacAddressConfigMap>,
    arch_file_map: Option<ArchConfigMap>,
    tftp_server_dir: Option<String>,
}

#[derive(Default, Clone, Debug)]
pub struct ConfEntry {
    boot_file: Option<String>,
    boot_server_ipv4: Option<Ipv4Addr>,
}

pub const CONFIG_FOLDER: &str = "preboot-oxide";
pub const YAML_FILENAME: &str = "preboot-oxide.yaml";
pub const ENV_VAR_PREFIX: &str = "PO_";

// Unused for now, until we add support for architecture based configuration
pub const _DHCP_ARCHES: phf::Map<&'static str, u16> = phf_map! {
    "x86" => 0x0,
    "itanium" => 0x2,
    "x86-uefi" => 0x6,
    "x64-uefi" => 0x7,
    "arm32-uefi" => 0x0a,
    "arm64-uefi" => 0x0b,
    "arm32-uboot" => 0x15,
    "arm64-uboot" => 0x16,
    "arm32-rpiboot" => 0x29,
    "riscv32-uefi" => 0x19,
    "riscv64-uefi" => 0x1b,
    "riscv128-uefi" => 0x1d
};
// source: https://www.iana.org/assignments/dhcpv6-parameters/dhcpv6-parameters.xhtml#processor-architecture

pub struct ProcessEnvConf {
    conf: ConfEntry,
    ifaces: Option<Vec<String>>,
    tftp_server_dir: Option<String>,
}

impl ProcessEnvConf {
    pub fn from_process_env() -> ProcessEnvConf {
        let boot_server_ipv4: Option<Ipv4Addr> =
            std::env::var(format!("{ENV_VAR_PREFIX}TFTP_SERVER_IPV4"))
                .unwrap_or_default()
                .parse()
                .ok();
        let boot_file = std::env::var(format!("{ENV_VAR_PREFIX}BOOT_FILE")).ok();
        let tftp_server_dir = std::env::var(format!("{ENV_VAR_PREFIX}TFTP_SERVER_DIR_PATH")).ok();
        let ifaces_csv = std::env::var(format!("{ENV_VAR_PREFIX}IFACES")).ok();
        let ifaces = ifaces_csv.map(|csv| csv.split(",").map(|s| s.to_string()).collect());

        Self {
            conf: ConfEntry {
                boot_server_ipv4,
                boot_file,
            },
            tftp_server_dir,
            ifaces,
        }
    }
}

impl From<ProcessEnvConf> for Conf {
    fn from(env_conf: ProcessEnvConf) -> Self {
        let mut conf = Self::new();
        conf.merge_left_into_default(&env_conf.conf);
        conf.ifaces = env_conf.ifaces;
        conf.tftp_server_dir = env_conf.tftp_server_dir;

        conf
    }
}

/// Checks if a property is set in the default configuration or in any of the MAC address definitions. If any of them is set, returns true.
macro_rules! has_prop_deep {
    ($self:ident, $prop:ident) => {
        $self
            .default
            .as_ref()
            .map(|i| i.$prop.is_some())
            .unwrap_or_else(|| {
                $self
                    .mac_file_map
                    .as_ref()
                    .map(|mac_map| {
                        mac_map
                            .values()
                            .any(|conf| conf.as_ref().map(|c| c.$prop.is_some()).unwrap_or(false))
                    })
                    .unwrap_or(false)
            })
    };
}

/// Gets the value of the given property in MAC address configuration or in the default configuration. If the property is not set in any of them, returns None.
macro_rules! get_cloned_prop_by_mac_or_default {
    ($self:ident, $prop:ident, $mac: ident) => {
        $self.mac_file_map
        .as_ref()
        .map(|mac_map| {
            mac_map
                .get($mac)
                .map(|conf| conf.as_ref()?.$prop.clone())
        })
        .flatten()
        .flatten()
        .or_else(|| $self.default.as_ref()?.$prop.clone())
    };
}

impl Conf {
    fn new() -> Self {
        Default::default()
    }

    pub fn validate(&self) -> Result<()> {
        if self
            .default
            .as_ref()
            .map(|i| i.boot_file.as_ref())
            .is_none()
            && self.mac_file_map.is_none()
            && self.arch_file_map.is_none()
        {
            bail!("No path to the boot file was configured.")
        }

        let has_external_tftp_server = has_prop_deep!(self, boot_server_ipv4);
        let has_boot_file = has_prop_deep!(self, boot_file);
        let has_tftp_server_dir = self.tftp_server_dir.is_some();

        if !has_external_tftp_server && !has_tftp_server_dir {
            bail!("Neither a TFTP local directory path nor an external TFTP server IP address was configured.")
        }

        if !has_boot_file {
            bail!("No path to the boot file was configured. This is required to tell the clients what file to boot.")
        }

        Ok(())
    }

    pub fn from_yaml_config(path_override: Option<&PathBuf>) -> Result<Self> {
        let path = path_override
            .map(|path| PathBuf::from(path))
            .unwrap_or_else(|| {
                dirs::config_local_dir()
                    .map(|config_path| config_path.join(&CONFIG_FOLDER).join(&YAML_FILENAME))
                    .unwrap_or_else(|| PathBuf::from(&YAML_FILENAME))
            });

        Self::from_yaml_file(&path)
    }

    fn from_yaml_file(path: &Path) -> Result<Self> {
        let mut file = std::fs::File::open(&path)?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)?;

        let yaml_conf = yaml_rust2::YamlLoader::load_from_str(&buf)?;

        let default: Option<ConfEntry> = Conf::base_conf_from_yaml(&yaml_conf[0]["default"])?;
        let tftp_server_dir: Option<String> = yaml_conf[0]["tftp_server_dir"]
            .as_str()
            .map(|s| s.to_string());
        let ifaces: Option<Vec<String>> = yaml_conf[0]["ifaces"].as_vec().map(|v| {
            v.iter()
                .map(|i| i.as_str().map(|s| s.to_string()))
                .flatten()
                .collect()
        });

        let mac_file_map: Option<MacAddressConfigMap> = yaml_conf[0]["by_mac_address"]
            .as_hash()
            .map(
                |yaml_object: &yaml_rust2::yaml::Hash| -> Result<MacAddressConfigMap> {
                    let mut mac_file_map = HashMap::new();
                    for (mac_address, conf) in yaml_object.iter() {
                        let mac = mac_address
                            .as_str()
                            .map(mac_address_to_bytes)
                            .transpose()?
                            .ok_or(anyhow!("Expected a MAC address"))?;

                        let conf_entry = Conf::base_conf_from_yaml(conf)?;
                        mac_file_map.insert(mac, conf_entry);
                    }

                    Result::Ok(mac_file_map)
                },
            )
            .transpose()?;

        Ok(Self {
            default,
            ifaces,
            tftp_server_dir,
            mac_file_map,
            arch_file_map: None, // TODO: Add support for architecture based configuration
        })
    }

    fn base_conf_from_yaml(yaml_conf: &yaml_rust2::Yaml) -> Result<Option<ConfEntry>> {
        yaml_conf
            .as_hash()
            .map(|yaml_obj| {
                let boot_file = yaml_obj
                    .get(&Yaml::from_str("boot_file"))
                    .map(|v| v.as_str().map(|s| s.to_string()))
                    .flatten();
                let boot_server_ipv4 = yaml_obj
                    .get(&Yaml::from_str("boot_server_ipv4"))
                    .map(|v| {
                        v.as_str().map_or(Result::Ok(None), |s: &str| {
                            Ok(Some(Ipv4Addr::from_str(s).map_err(|o| {
                                anyhow!("IPv4 parsing error: {}", o.to_string())
                            })?))
                        })
                    })
                    .map_or(Ok(None), |i: Result<Option<Ipv4Addr>>| i)?;

                Ok(ConfEntry {
                    boot_file,
                    boot_server_ipv4,
                })
            })
            .transpose()
    }

    pub fn merge_left_into_default(&mut self, other: &ConfEntry) {
        self.default = self
            .default
            .as_ref()
            .map(|mine| ConfEntry {
                boot_file: mine.boot_file.clone().or(other.boot_file.clone()),
                boot_server_ipv4: mine.boot_server_ipv4.clone().or(other.boot_server_ipv4),
            })
            .or(Some(other.clone()));
    }

    pub fn get_boot_file(&self, mac_address: &MacAddress) -> Option<String> {
        get_cloned_prop_by_mac_or_default!(self, boot_file, mac_address)
    }

    pub fn get_boot_server_ipv4(
        &self,
        self_ip_v4: Option<&Ipv4Addr>,
        mac_address: &MacAddress,
    ) -> Option<Ipv4Addr> {
        let conf_tftp_server_ipv4 = get_cloned_prop_by_mac_or_default!(self, boot_server_ipv4, mac_address);

        if conf_tftp_server_ipv4.is_some() {
            return conf_tftp_server_ipv4;
        } else {
            return self_ip_v4.copied();
        }
    }

    pub fn get_ifaces(&self) -> Option<&Vec<String>> {
        self.ifaces.as_ref()
    }

    pub fn get_tftp_serve_path(&self) -> Option<String> {
        self.tftp_server_dir.clone()
    }
}
