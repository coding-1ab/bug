use crate::network::error::NetworkError;
use crate::network::util;

// 지렁이는 몸통 요소 좌표들과 색상 rgba를 가짐
#[derive(Debug)]
pub struct WormBody {
    client_id: usize,
    color: (f32, f32, f32, f32),
    positions: Vec<(f32, f32)>
}

impl WormBody {
    pub fn new(client_id: usize, bytes: &[u8]) -> Result<Self, NetworkError> {
        let color = &bytes[..16];
        let positions = &bytes[16..];

        Ok(Self {
            client_id,
            color: util::bytes_to_color(color)?,
            positions: util::bytes_to_positions(positions)?
        })
    }

    pub fn random(client_id: usize) -> Self {
        // todo 색상, 위치 정보를 랜덤하게 만들어야 함
        Self {
            client_id,
            color: (0.0, 0.0, 0.0, 0.0),
            positions: vec![
                (-1126.9424, -128.45294),
                (-1150.8949, -126.944374),
                (-1144.9067, -127.32152),
                (-1138.9187, -127.698654),
                (-1132.9305, -128.0758),
                (-1126.9424, -128.45294)
            ]
        }
    }

    pub fn make_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(2 + self.positions.len() * 16);
        bytes.push((self.client_id >> 8 & 0xff) as u8);
        bytes.push((self.client_id & 0xff) as u8);
        bytes.extend(util::color_to_bytes(&self.color));
        bytes.extend(util::positions_to_bytes(&self.positions));
        bytes
    }
}