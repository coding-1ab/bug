
pub mod error;
pub mod validator;
pub mod util;

// 지렁이 몸통을 이루는 것들을 좌표계로..
// 지렁이 색깔도 각각 달라야 서로 구분이 될듯..
#[derive(Debug)]
struct WormBody {
    client_id: usize,
    // color: ???
    position: Vec<(usize, usize)>
}

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
    //      N+1     |       201     |   client id(u16), 지렁이 몸통 정보(N bytes)
    ReqMove {
        client_id: usize,
        worm_body: WormBody,    // 각 클라이언트는 자기 위치 움직일 때, 자신의 몸통 좌표들을 전송
    },
    //      3       |       202     |   client id(u16), 먹이(u16)
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
    pub fn new(message_bytes: &[u8]) -> Result<Self, error::RuleError> {
        // 패킷 유형
        let type_num = message_bytes[0] as usize;

        // todo 응답 메세지 구성
        // 해당 패킷 유형의 내용물
        let message_body_bytes = &message_bytes[1..];

        match type_num {
            101 => {
                let client_id = util::bytes_to_u16_be(message_body_bytes).unwrap() as usize;
                Ok(MessageFromClient::ReqJoin { client_id })
            },
            102 => {
                let client_id = util::bytes_to_u16_be(message_body_bytes).unwrap() as usize;
                Ok(MessageFromClient::ReqLeave { client_id })
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
            n => Err(error::RuleError::InvalidPacketType(n))
        }
    }
}

pub enum MessageFromServer {
    // 1XX
    ResJoin {
        client_id: usize,
        worm_body: WormBody,    // 서버 조인 시, 초기 위치는 서버에서 정해서 내려준다.
    },
    ResLeave {
        client_id: usize,       // 클라 나갈 때, 그대로 다른 클라들에게 전부 echo
    },

    // 2XX
    ResMove {
        client_id: usize,
        worm_body: WormBody,    // 서버는 클라에게 받은 내용을 그대로 다른 클라들에게 echo
    },
    ResEat {
        client_id: usize,
        food_amount: usize,
        is_ok: bool,            // 성공 여부 판단
        worm_body: WormBody,    // 먹이를 먹은 클라이언트는 몸통 길이가 늘어나므로 그 정보를 모든 클라에게 전송
    },
    ResDie {                    // 죽은 클라 정보를 모든 클라에게 echo
        client_id: usize,
        is_ok: bool,
    },
}
