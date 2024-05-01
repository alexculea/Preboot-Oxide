
pub fn bytes_to_mac_address(bytes: &[u8]) -> String {
  let str_parts: Vec<String> = bytes.chunks(2).into_iter().map(| bytes | format!("{:X}{:X}", bytes[0], bytes[1])).collect();
  
  str_parts.join(":")
}