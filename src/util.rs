pub fn bytes_to_mac_address(bytes: &[u8]) -> String {
    let str_parts: Vec<String> = bytes
        .into_iter()
        .map(|byte| format!("{:0>2X}", byte))
        .collect();
    str_parts.join(":")
}
