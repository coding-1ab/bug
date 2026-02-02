use crate::network::message::worm_body::WormBody;

// Req*는 Client -> Server 요청,
// Res*는 Server -> Client 응답.
#[derive(Debug)]
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