use crate::network::error::NetworkError;

pub fn bytes_to_u16_be(bytes: &[u8]) -> Result<u16, NetworkError> {
    if bytes.len() < 2 {
        return Err(NetworkError::ShortMsg {
            expected_length: 2,
            actual_length: bytes.len(),
        });
    }
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

pub fn u16_be_to_bytes(num: u16) -> [u8; 2] {
    [(num >> 8 & 0xff) as u8, (num & 0xff) as u8]
}

pub fn bytes_to_hex(bytes: &[u8]) -> String {
    if bytes.len() == 0 {
        return "".to_string()
    }
    format!("0x{}", hex::encode(bytes))
}

// bytes -> color
pub fn bytes_to_color(bytes: &[u8]) -> Result<(f32, f32, f32, f32), NetworkError> {
    if bytes.len() < 16 {
        return Err(NetworkError::ShortMsg {expected_length: 16, actual_length: bytes.len()});
    }

    let r = f32::from_be_bytes(bytes[0..4].try_into().unwrap());
    let g = f32::from_be_bytes(bytes[4..8].try_into().unwrap());
    let b = f32::from_be_bytes(bytes[8..12].try_into().unwrap());
    let a = f32::from_be_bytes(bytes[12..16].try_into().unwrap());

    Ok((r, g, b, a))
}

// color -> bytes
pub fn color_to_bytes(color: &(f32, f32, f32, f32)) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(16);
    bytes.extend_from_slice(&color.0.to_be_bytes());
    bytes.extend_from_slice(&color.1.to_be_bytes());
    bytes.extend_from_slice(&color.2.to_be_bytes());
    bytes.extend_from_slice(&color.3.to_be_bytes());
    bytes
}

// bytes -> positions
pub fn bytes_to_positions(bytes: &[u8]) -> Result<Vec<(f32, f32)>, NetworkError> {
    if bytes.len() % 8 != 0 {
        return Err(NetworkError::InvalidMsg { input_length: bytes.len() });
    }

    let mut pairs = Vec::with_capacity(bytes.len() / 8);

    for chunk in bytes.chunks_exact(8) {
        let a = f32::from_be_bytes(chunk[0..4].try_into().unwrap());
        let b = f32::from_be_bytes(chunk[4..8].try_into().unwrap());
        pairs.push((a, b));
    }

    Ok(pairs)
}

// positions -> bytes
pub fn positions_to_bytes(positions: &Vec<(f32, f32)>) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(positions.len() * 8);

    for &(a, b) in positions {
        bytes.extend_from_slice(&a.to_be_bytes());
        bytes.extend_from_slice(&b.to_be_bytes());
    }

    bytes
}