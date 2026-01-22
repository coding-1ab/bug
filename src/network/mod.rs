use std::error::Error;

pub struct Packet {
    byte_length: u16,
    msg: MessageFromClient
}

impl Packet {
    pub fn new(bytes: &[u8]) -> Result<Self, String> {
        let byte_length = (bytes[0] as u16) << 8 | bytes[1] as u16;
        let msg = MessageFromClient::new(bytes[2..].to_vec())?;

        Ok(Self {
            byte_length, msg
        })
    }
}

// Req*는 Client -> Server 요청,
// Res*는 Server -> Client 응답.
enum MessageFromServer {
    // 1XX
    ResJoin {
        client_id: u16,
        worm_body: WormBody,    // 서버 조인 시, 초기 위치는 서버에서 정해서 내려준다.
    },
    ResLeave {
        client_id: u16,         // 클라 나갈 때, 그대로 다른 클라들에게 전부 echo
    },

    // 2XX
    ResMove {
        client_id: u16,
        worm_body: WormBody,    // 서버는 클라에게 받은 내용을 그대로 다른 클라들에게 echo
    },
    ResEat {
        client_id: u16,
        food_amount: u8,
        is_ok: bool,            // 성공 여부 판단
        worm_body: WormBody,    // 먹이를 먹은 클라이언트는 몸통 길이가 늘어나므로 그 정보를 모든 클라에게 전송
    },
    ResDie {                    // 죽은 클라 정보를 모든 클라에게 echo
        client_id: u16,
        is_ok: bool,
    },
}

enum MessageFromClient {
    // 1XX
    ReqJoin {
        client_id: u16,
    },
    ReqLeave {
        client_id: u16,
    },

    // 2XX
    ReqMove {
        client_id: u16,
        worm_body: WormBody,    // 각 클라이언트는 자기 위치 움직일 때, 자신의 몸통 좌표들을 전송
    },
    ReqEat {
        client_id: u16,
        food_amount: u8,        // 먹이의 크기
    },
    ReqDie {
        client_id: u16,
    },
}

impl MessageFromClient {
    fn new(bytes: Vec<u8>) -> Result<Self, String> {
        // 첫번째 바이트가 패킷 유형
        // 그 이후로는 해당 패킷 유형의 내용물
        let type_num = bytes[0];
        let message_body = &bytes[1..];

        // todo 응답 메세지 구성
        match type_num {
            101 => {
                Ok(MessageFromClient::ReqJoin { client_id: 0 })
            },
            102 => {
                Ok(MessageFromClient::ReqLeave { client_id: 0 })
            },
            201 => {
                Ok(MessageFromClient::ReqMove { client_id: 0, worm_body: WormBody { client_id: 0, position: vec![] } })
            },
            202 => {
                Ok(MessageFromClient::ReqEat { client_id: 0, food_amount: 0 })
            },
            203 => {
                Ok(MessageFromClient::ReqDie { client_id: 0 })
            }
            _ => Err(format!("unknown packet type. (received type number: {})", type_num))
        }
    }
}

// 지렁이 몸통을 이루는 것들을 좌표계로..
// 지렁이 색깔도 각각 달라야 서로 구분이 될듯..
struct WormBody {
    client_id: u16,
    // color: ???
    position: Vec<(u16, u16)>
}


