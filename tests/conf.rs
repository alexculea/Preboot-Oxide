extern crate preboot_oxide;

use preboot_oxide::conf::*;
use std::net::Ipv4Addr;

mod utils;

#[test]
fn test_conf_from_env() {
    std::env::set_var(format!("{ENV_VAR_PREFIX}TFTP_SERVER_IPV4"), "1.1.1.1");
    std::env::set_var(format!("{ENV_VAR_PREFIX}BOOT_FILE"), "/bootfile");
    std::env::set_var(format!("{ENV_VAR_PREFIX}TFTP_SERVER_DIR_PATH"), "/tftpdir");
    std::env::set_var(format!("{ENV_VAR_PREFIX}IFACES"), "eth0,eth1");
    std::env::set_var(format!("{ENV_VAR_PREFIX}MAX_SESSIONS"), "100");
    let env_conf = ProcessEnvConf::from_process_env();
    let conf = Conf::from(env_conf);    
    let def = conf.get_from_doc(serde_json::Value::default()).unwrap().unwrap();

    assert_eq!(def.boot_server_ipv4, Some(&Ipv4Addr::new(1, 1, 1, 1)));
    assert_eq!(def.boot_file, Some(&"/bootfile".to_string()));
    assert_eq!(conf.get_tftp_serve_path(), Some("/tftpdir".to_string()));
    assert_eq!(conf.get_ifaces(), Some(vec!["eth0".to_string(), "eth1".to_string()].as_ref()));
    assert_eq!(conf.get_max_sessions(), 100);
}

#[test]
fn test_default_external_conf_from_yaml() {
    let yaml = r#"
default:
    boot_server_ipv4: 10.0.0.1
    boot_file: /bootfile
    "#;
    let yaml_mock = utils::YamlMockFile::from_yaml(yaml);
    let conf = Conf::from_yaml_config(Some(&yaml_mock.path)).unwrap();
    let def = conf.get_from_doc(serde_json::Value::default()).unwrap().unwrap();

    assert_eq!(def.boot_server_ipv4, Some(&Ipv4Addr::new(10, 0, 0, 1)));
    assert_eq!(def.boot_file, Some(&"/bootfile".to_string()));
}