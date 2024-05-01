use std::{collections::HashMap, net::{Ipv4Addr, Ipv6Addr}};
use anyhow::Result;

#[derive(Default)]
pub struct Conf {
  mac_boot_file: HashMap<String, String>,
  boot_file: Option<String>,
  boot_server_ipv4: Option<Ipv4Addr>,
  boot_server_ipv6: Option<Ipv6Addr>,
  use_self_ip: bool,
}

impl Conf {
  fn new() -> Self {
    Default::default()
  }

  pub fn validate(&self) -> Result<()> {
    if self.boot_file.is_none() && self.mac_boot_file.len() == 0 {
      bail!("Neither bootfile nor MAC <> bootfile associations configured.")
    }
    Ok(())
  }

  pub fn from_proccess_env() -> Result<Self> {
    let boot_server_ipv4: Option<Ipv4Addr> = std::env::var("PXE_DHCP_BOOT_SERVER_IPV4")
      .unwrap_or_default()
      .parse()
      .ok();
    let boot_server_ipv6: Option<Ipv6Addr> = std::env::var("PXE_DHCP_BOOT_SERVER_IPV6")
      .unwrap_or_default()
      .parse()
      .ok();
    let use_self_ip = boot_server_ipv4.is_some() || boot_server_ipv6.is_some();

    let boot_file = std::env::var("PXE_DHCP_BOOT_FILE").ok();
    let mac_boot_file_str = std::env::var("PXE_DHCP_MAC_BOOT_FILES").ok();
    let mac_boot_file = parse_macs_boot_files(mac_boot_file_str);

    Ok(Self {
      boot_server_ipv4,
      boot_server_ipv6,
      use_self_ip,
      boot_file,
      mac_boot_file
    })
  }

  pub fn get_boot_file(&self) -> Option<String> {
    self.boot_file.clone()
  }  
  
  pub fn get_boot_server_ipv4(&self, self_ip_v4: Option<Ipv4Addr>) -> Option<Ipv4Addr> {
    if self.boot_server_ipv4.is_some()  {
      return self.boot_server_ipv4
    } else {
      return self_ip_v4
    }
  }
}



fn parse_macs_boot_files(input_str: Option<String>) -> HashMap<String, String> {
  let mut result = HashMap::default();
  if input_str.is_none() {
    return result;
  }

  input_str
    .unwrap()
    .split(";")
    .collect::<Vec<&str>>()
    .chunks(2)
    .for_each(| chunk | {
      let left = chunk[0];
      let right = chunk[1];
      result.insert((*left).to_string(), (*right).to_string());
    });

  return result
}