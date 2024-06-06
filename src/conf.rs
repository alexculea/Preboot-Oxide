use anyhow::{Context, Result};
use log::trace;
use once_cell::sync::Lazy;
use std::{
    collections::HashMap,
    io::Read,
    net::Ipv4Addr,
    path::{Path, PathBuf},
    str::FromStr,
};
use yaml_rust2::Yaml;

pub type MacAddress = [u8; 6];
type FieldConverter = for<'a> fn(&'a serde_json::Value) -> Result<String>;
type FieldConverterMap = Lazy<HashMap<&'static str, FieldConverter>>;

#[derive(Clone, Debug)]
pub struct Conf {
    default: Option<ConfEntry>,
    ifaces: Option<Vec<String>>,
    match_map: Option<Vec<MatchEntry>>,
    tftp_server_dir: Option<String>,
    max_sessions: u64,
}

#[derive(Default, Clone, Debug)]
pub struct ConfEntry {
    pub boot_file: Option<String>,
    pub boot_server_ipv4: Option<Ipv4Addr>,
}

impl ConfEntry {
    pub fn merge(self, other_option: Option<&ConfEntry>) -> ConfEntry {
        let other = if let Some(other) = other_option {
            other
        } else {
            return self;
        };

        let boot_file = self.boot_file.or(other.boot_file.clone());
        let boot_server_ipv4 = self.boot_server_ipv4.or(other.boot_server_ipv4);

        ConfEntry {
            boot_file,
            boot_server_ipv4,
        }
    }
}

#[derive(Clone, Debug)]
enum MatchType {
    Any,
    All,
}
#[derive(Clone, Debug)]
struct MatchEntry<T = String> {
    fields_values: HashMap<String, T>,
    conf: ConfEntry,
    match_type: MatchType,
    regex: bool,
}

pub const DEFAULT_MAX_SESSIONS: u64 = 500;
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

pub const FIELD_MAP: phf::Map<&'static str, &'static str> = phf_map! {
    "ClientMacAddress" => "chaddr",
    "HardwareType" => "htype",
};
static FIELD_CONVERTERS: FieldConverterMap = Lazy::new(|| {
    HashMap::from([
        (
            "ClientMacAddress",
            (|input: &serde_json::Value| Conf::get_mac_from_doc_string(input)) as FieldConverter,
        ),
        (
            "ClassIdentifier",
            |input: &serde_json::Value| -> Result<String> {
                input
                    .as_array()
                    .map(|arr| {
                        Ok(arr
                            .iter()
                            .map(|item| Ok(char::try_from(item.as_u64().unwrap_or(0) as u32)?))
                            .collect::<Result<Vec<char>>>()?
                            .iter()
                            .collect::<String>())
                    })
                    .unwrap_or(Ok(String::default()))
            },
        ),
    ])
});

pub struct ProcessEnvConf {
    conf: ConfEntry,
    ifaces: Option<Vec<String>>,
    tftp_server_dir: Option<String>,
    max_sessions: Option<u64>,
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
        let max_sessions = std::env::var(format!("{ENV_VAR_PREFIX}MAX_SESSIONS"))
            .map(|s| s.parse::<u64>().ok())
            .ok()
            .flatten();

        Self {
            conf: ConfEntry {
                boot_server_ipv4,
                boot_file,
            },
            tftp_server_dir,
            ifaces,
            max_sessions,
        }
    }
}

impl From<ProcessEnvConf> for Conf {
    fn from(env_conf: ProcessEnvConf) -> Self {
        let mut conf = Self {
            default: None,
            ifaces: None,
            max_sessions: env_conf.max_sessions.unwrap_or(DEFAULT_MAX_SESSIONS),
            match_map: None,
            tftp_server_dir: None,
        };

        conf.merge_left_into_default(&env_conf.conf);
        conf.ifaces = env_conf.ifaces;
        conf.tftp_server_dir = env_conf.tftp_server_dir;

        conf
    }
}

impl Conf {
    pub fn validate(&self) -> Result<()> {
        let has_external_tftp_server = self
            .match_map
            .as_ref()
            .map(|m| m.iter().any(|me| me.conf.boot_server_ipv4.is_some()))
            .or(self.default.as_ref().map(|d| d.boot_server_ipv4.is_some()))
            .unwrap_or(false);
        let has_tftp_path = self.tftp_server_dir.is_some();
        let has_boot_filename = self
            .match_map
            .as_ref()
            .map(|m| m.iter().any(|me| me.conf.boot_file.is_some()))
            .or(self.default.as_ref().map(|d| d.boot_file.is_some()))
            .unwrap_or(false);

        if !has_external_tftp_server && !has_tftp_path {
            return Err(anyhow!(
                "No TFTP server path or external TFTP server configured."
            ));
        }

        if !has_boot_filename {
            return Err(anyhow!("No boot filename configured."));
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
        let max_sessions = yaml_conf[0]["max_sessions"]
            .as_i64()
            .map(u64::try_from)
            .unwrap_or(Ok(DEFAULT_MAX_SESSIONS))
            .context("Parsing max_sessions from YAML file.")?;

        let match_map: Option<Vec<MatchEntry>> = yaml_conf[0]["match"]
            .as_vec()
            .map(|match_entry| -> Result<Vec<MatchEntry>> {
                Result::Ok(
                    match_entry
                        .iter()
                        .map(Self::match_entry_from_yaml)
                        .collect::<Result<Vec<MatchEntry>>>()?,
                )
            })
            .transpose()?;

        Ok(Self {
            default,
            ifaces,
            tftp_server_dir,
            max_sessions,
            match_map,
        })
    }

    fn match_entry_from_yaml(item: &yaml_rust2::Yaml) -> Result<MatchEntry> {
        let conf = Conf::base_conf_from_yaml(&item["conf"])?
            .ok_or(anyhow!("No configuration found for match entry"))?;

        let match_type = item["match_type"]
            .as_str()
            .map(|s| match s.to_lowercase().as_str() {
                "any" => Ok(MatchType::Any),
                "all" => Ok(MatchType::All),
                _ => Err(anyhow!("Invalid match type: {s}")),
            })
            .unwrap_or(Ok(MatchType::All))?;

        let fields_values = item["select"]
            .as_hash()
            .map(|yaml_obj| -> Result<HashMap<String, String>> {
                Result::Ok(
                    yaml_obj
                        .iter()
                        .map(|(key, value)| {
                            Ok((
                                key.as_str()
                                    .ok_or(anyhow!("Expected a string key"))?
                                    .to_string(),
                                value
                                    .as_str()
                                    .ok_or(anyhow!("Expected a string value"))?
                                    .to_string(),
                            ))
                        })
                        .collect::<Result<HashMap<String, String>>>()?,
                )
            })
            .transpose()?
            .ok_or(anyhow!("Expected a hash for select"))?;

        let regex = item["regex"].as_bool().unwrap_or(false);

        Ok(MatchEntry {
            conf,
            fields_values,
            match_type,
            regex,
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

    pub fn get_ifaces(&self) -> Option<&Vec<String>> {
        self.ifaces.as_ref()
    }

    pub fn get_tftp_serve_path(&self) -> Option<String> {
        self.tftp_server_dir.clone()
    }

    fn get_mac_from_doc_string(doc: &serde_json::Value) -> Result<String> {
        let client_mac: String = doc
            .as_array()
            .and_then(|list| {
                Some(
                    list.iter()
                        .take(6)
                        .map(|value| {
                            value
                                .as_u64()
                                .map(|byte| Some(format!("{:0>2X}", byte)))
                                .flatten()
                        })
                        .collect::<Option<Vec<String>>>(),
                )
            })
            .flatten()
            .ok_or(anyhow!("Expected MAC address to be an array of numbers."))?
            .join(":");

        Ok(client_mac)
    }

    fn is_match(doc: &serde_json::Value, match_entry: &MatchEntry) -> bool {
        let matcher = |cfg_key: &String, cfg_value| {
            let cfg_key = cfg_key.clone();
            move |doc_value: &serde_json::Value| {
                let default_converter: FieldConverter =
                    |v: &serde_json::Value| -> Result<String> { Ok(v.to_string()) };
                let doc_val_converter = FIELD_CONVERTERS
                    .get(cfg_key.as_str())
                    .unwrap_or(&default_converter);
                let converted_value = doc_val_converter(doc_value).unwrap_or(doc_value.to_string());

                let match_result = if match_entry.regex {
                    let re = regex::Regex::new(cfg_value).unwrap();
                    re.is_match(&converted_value)
                } else {
                    converted_value == cfg_value
                };

                let match_type = if match_entry.regex {
                    "regex"
                } else {
                    "exact"
                };

                trace!("Matching {match_type} field {cfg_key}=\"{converted_value}\" to \"{cfg_value}\", matching = {match_result}");
                match_result
            }
        };

        match match_entry.match_type {
            MatchType::Any => match_entry.fields_values.iter().any(|(key, config_value)| {
                doc.get(Self::get_remapped_key(key))
                    .or(doc
                        .get("opts")
                        .and_then(|opts| opts.get(key))
                        .and_then(|opts_key| opts_key.get(key)))
                    .map(matcher(key, config_value))
                    .unwrap_or(false)
            }),
            MatchType::All => match_entry.fields_values.iter().all(|(key, config_value)| {
                doc.get(Self::get_remapped_key(key))
                    .or(doc
                        .get("opts")
                        .and_then(|opts| opts.get(key))
                        .and_then(|opts_key| opts_key.get(key)))
                    .map(matcher(key, config_value))
                    .unwrap_or(false)
            }),
        }
    }

    fn get_remapped_key<'a>(key: &'a str) -> &'a str {
        FIELD_MAP.get(key).unwrap_or(&key)
    }

    pub fn get_from_doc(&self, doc: serde_json::Value) -> Result<Option<ConfEntry>> {
        let matched_conf = self
            .match_map
            .as_ref()
            .map(|matches| {
                matches
                    .iter()
                    .find(|match_entry| Self::is_match(&doc, match_entry))
            })
            .flatten()
            .map(|m| &m.conf)
            .inspect(|conf| trace!("Found matching entry from 'match' rule.\n{:#?}", conf))
            .or_else(|| {
                trace!("No matching entry found from 'match' rule.");
                self.default.as_ref()
            });

        let result = matched_conf
            .map(|cfg| cfg.clone())
            .map(|cfg| cfg.merge(self.default.as_ref()))
            .inspect(|conf| trace!("Final result combined with default:\n{:#?}", conf))
            .or_else(|| { 
                trace!("No configuration found for this client in either 'default' or 'match' rules.");
                None 
            });

        Ok(result)
    }

    pub fn get_max_sessions(&self) -> u64 {
        self.max_sessions
    }
}
