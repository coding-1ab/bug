

pub fn bytes_to_u16_be(bytes: &[u8]) -> Option<u16> {
    if bytes.len() < 2 {
        return None;
    }
    Some(u16::from_be_bytes([bytes[0], bytes[1]]))
}

pub fn bytes_to_hex(bytes: &Vec<u8>) -> String {
    if bytes.len() == 0 {
        return "".to_string()
    }
    format!("0x{}", hex::encode(bytes))
}
