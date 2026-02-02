use crate::network::error::NetworkError;
use crate::network::util;

// 지렁이 몸통을 이루는 것들을 좌표계로..
// 지렁이 색깔도 각각 달라야 서로 구분이 될듯..
#[derive(Debug)]
pub struct WormBody {
    client_id: usize,

    // color: ???

    positions: Vec<(f32, f32)>
}

impl WormBody {
    pub fn new(client_id: usize, positions: &[u8]) -> Result<Self, NetworkError> {
        Ok(Self {
            client_id,
            positions: util::bytes_to_positions(positions)?
        })
    }

    pub fn make_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(2 + self.positions.len() * 16);
        bytes.push((self.client_id >> 8 & 0xff) as u8);
        bytes.push((self.client_id & 0xff) as u8);
        bytes.extend(util::positions_to_bytes(&self.positions));
        bytes
    }
}