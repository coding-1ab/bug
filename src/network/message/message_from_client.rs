use crate::network::error::ProtocolError;
use crate::network::{error, util};
use crate::network::message::worm_body::WormBody;
use crate::network::util::u16_be_to_bytes;

// Req*는 Client -> Server 요청,
// Res*는 Server -> Client 응답.
#[derive(Debug)]
pub enum MessageFromClient {
    // 1XX
    // 길이(2bytes)  |   유형(1byte)   |   메세지(N bytes)

    //      3       |       101     |   client id(u16)
    ReqJoin {
        client_id: usize,
    },
    //      3       |       102     |   client id(u16)
    ReqLeave {
        client_id: usize,
    },

    // 2XX
    //      3 + N   |       201     |   client id(u16), 지렁이 몸통 정보(N bytes)
    ReqMove {
        client_id: usize,
        worm_body: WormBody,    // 각 클라이언트는 자기 위치 움직일 때, 자신의 몸통 좌표들을 전송
    },
    //      5       |       202     |   client id(u16), 먹이(u16)
    ReqEat {
        client_id: usize,
        food_amount: usize, // 먹이의 크기
    },
    //      3       |       203     |   client id(u16)
    ReqDie {
        client_id: usize,
    },
}

impl MessageFromClient {

    // 검열된 바이트 배열을 가지고, 클라이언트 요청 구조체를 생성
    pub fn new(message_bytes: &[u8]) -> Result<Self, ProtocolError> {
        // 패킷 유형
        let type_num = message_bytes[0] as usize;

        // 해당 패킷 유형의 내용물
        let message_body_bytes = &message_bytes[1..];

        match type_num {
            101 => {
                let client_id = util::bytes_to_u16_be(&message_body_bytes[..2])? as usize;
                Ok(MessageFromClient::ReqJoin { client_id })
            },
            102 => {
                let client_id = util::bytes_to_u16_be(&message_body_bytes[..2])? as usize;
                Ok(MessageFromClient::ReqLeave { client_id })
            },
            201 => {
                let client_id = util::bytes_to_u16_be(&message_body_bytes[..2])? as usize;
                let worm_body = WormBody::new(client_id, &message_body_bytes[2..])?;
                Ok(MessageFromClient::ReqMove { client_id, worm_body })
            },
            202 => {
                let client_id = util::bytes_to_u16_be(&message_body_bytes[..2])? as usize;
                let food_amount = util::bytes_to_u16_be(&message_body_bytes[2..])? as usize;
                Ok(MessageFromClient::ReqEat { client_id, food_amount })
            },
            203 => {
                let client_id = util::bytes_to_u16_be(&message_body_bytes[..2])? as usize;
                Ok(MessageFromClient::ReqDie { client_id })
            },
            n => Err(ProtocolError::from(error::RuleError::InvalidPacketType(n))),
        }
    }

    pub fn make_bytes(&self) -> Vec<u8> {
        match *self {
            MessageFromClient::ReqJoin { client_id } => {
                let mut packet = Vec::with_capacity(5);
                packet.extend(u16_be_to_bytes(3));
                packet.push(101u8);
                packet.extend(u16_be_to_bytes(client_id as u16));
                packet
            },
            MessageFromClient::ReqLeave { client_id } => {
                let mut packet = Vec::with_capacity(5);
                packet.extend(u16_be_to_bytes(3));
                packet.push(102u8);
                packet.extend(u16_be_to_bytes(client_id as u16));
                packet
            },
            MessageFromClient::ReqMove { client_id, ref worm_body } => {
                let worm_body_bytes = worm_body.make_bytes();

                // message type length (1 bytes) + client id (2 bytes) + worm positions (N bytes)
                let mut packet = Vec::with_capacity(5 + worm_body_bytes.len());
                packet.extend(u16_be_to_bytes(1 + worm_body_bytes.len() as u16));
                packet.push(201u8);
                packet.extend(worm_body_bytes);
                packet
            },
            MessageFromClient::ReqEat { client_id, food_amount } => {
                let mut packet = Vec::with_capacity(7);
                packet.extend(u16_be_to_bytes(5));
                packet.push(202u8);
                packet.extend(u16_be_to_bytes(client_id as u16));
                packet.extend(u16_be_to_bytes(food_amount as u16));
                packet
            },
            MessageFromClient::ReqDie { client_id } => {
                let mut packet = Vec::with_capacity(5);
                packet.extend(u16_be_to_bytes(3));
                packet.push(203u8);
                packet.extend(u16_be_to_bytes(client_id as u16));
                packet
            },
        }
    }
}
