use std::convert::TryInto;

type MacAddressBytes = [u8; 6];
type Result<T> = crate::Result<T>;

pub fn bytes_to_mac_address(bytes: &[u8]) -> String {
    let str_parts: Vec<String> = bytes
        .into_iter()
        .map(|byte| format!("{:0>2X}", byte))
        .collect();
    str_parts.join(":")
}

pub fn mac_address_to_bytes(mac_address: &str) -> Result<MacAddressBytes> {
    let res: MacAddressBytes = mac_address
        .split(":")
        .map(|c: &str| u8::from_str_radix(c, 16))
        .collect::<core::result::Result<Vec<u8>, _>>()?
        .try_into()
        .map_err(|_e| {
            anyhow!("Couldn't convert byte vector to fixed sized array when parsing MAC address.")
        })?;

    Ok(res)
}
