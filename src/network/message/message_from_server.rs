use crate::network::error::ProtocolError;
use crate::network::message::worm_body::WormBody;
use crate::network::{error, util};
use crate::network::util::u16_be_to_bytes;

// Req*는 Client -> Server 요청,
// Res*는 Server -> Client 응답.
#[derive(Debug)]
pub enum MessageFromServer {
    // 1XX
    // 길이(2bytes)  |   유형(1byte)   |   메세지(N bytes)

    //      3 + N   |       101     |   client id(u16), worm_body(N bytes)
    ResJoin {
        client_id: usize,
        worm_body: WormBody,    // 서버 조인 시, 초기 위치는 서버에서 정해서 내려준다.
    },
    //      3       |       102     |   client id(u16)
    ResLeave {
        client_id: usize,       // 클라 나갈 때, 그대로 다른 클라들에게 전부 echo
    },

    // 2XX
    //      3 + N   |       201     |   client id(u16), 지렁이 몸통 정보(N bytes)
    ResMove {
        client_id: usize,
        worm_body: WormBody,    // 서버는 클라에게 받은 내용을 그대로 다른 클라들에게 echo
    },
    //      6       |       202     |   client id(u16), 먹이(u16), 성공여부(u8)
    ResEat {
        client_id: usize,
        food_amount: usize,
        is_ok: bool,            // 성공 여부 판단
    },
    //      3       |       203     |   client id(u16)
    ResDie {                    // 죽은 클라 정보를 모든 클라에게 echo
        client_id: usize,
    },
}

impl MessageFromServer {

    // 검열된 바이트 배열을 가지고, 서버 응답 구조체를 생성
    pub fn new(message_bytes: &[u8]) -> Result<Self, ProtocolError> {
        // 패킷 유형
        let type_num = message_bytes[0] as usize;

        // 해당 패킷 유형의 내용물
        let message_body_bytes = &message_bytes[1..];

        match type_num {
            101 => {
                let client_id = util::bytes_to_u16_be(&message_body_bytes[..2])? as usize;
                let worm_body = WormBody::new(client_id, &message_body_bytes[2..])?;
                Ok(MessageFromServer::ResJoin { client_id, worm_body })
            },
            102 => {
                let client_id = util::bytes_to_u16_be(&message_body_bytes[..2])? as usize;
                Ok(MessageFromServer::ResLeave { client_id })
            },
            201 => {
                let client_id = util::bytes_to_u16_be(&message_body_bytes[..2])? as usize;
                let worm_body = WormBody::new(client_id, &message_body_bytes[2..])?;
                Ok(MessageFromServer::ResMove { client_id, worm_body })
            },
            202 => {
                let client_id = util::bytes_to_u16_be(&message_body_bytes[..2])? as usize;
                let food_amount = util::bytes_to_u16_be(&message_body_bytes[2..4])? as usize;
                let is_ok = if message_body_bytes[4] == 1 { true } else { false };
                Ok(MessageFromServer::ResEat { client_id, food_amount, is_ok })
            },
            203 => {
                let client_id = util::bytes_to_u16_be(&message_body_bytes[..2])? as usize;
                Ok(MessageFromServer::ResDie { client_id })
            },
            n => Err(ProtocolError::from(error::RuleError::InvalidPacketType(n))),
        }
    }

    pub fn make_bytes(&self) -> Vec<u8> {
        match *self {
            MessageFromServer::ResJoin { client_id, ref worm_body } => {
                let worm_body_bytes = worm_body.make_bytes();
                let mut packet = Vec::with_capacity(5 + worm_body_bytes.len());
                packet.extend(u16_be_to_bytes(1 + worm_body_bytes.len() as u16));
                packet.push(101u8);
                packet.extend(worm_body_bytes);
                packet
            },
            MessageFromServer::ResLeave { client_id } => {
                let mut packet = Vec::with_capacity(5);
                packet.extend(u16_be_to_bytes(3));
                packet.push(102u8);
                packet.extend(u16_be_to_bytes(client_id as u16));
                packet
            },
            MessageFromServer::ResMove { client_id, ref worm_body } => {
                let worm_body_bytes = worm_body.make_bytes();

                // message type length (1 bytes) + client id (2 bytes) + worm positions (N bytes)
                let mut packet = Vec::with_capacity(5 + worm_body_bytes.len());
                packet.extend(u16_be_to_bytes(1 + worm_body_bytes.len() as u16));
                packet.push(201u8);
                packet.extend(worm_body_bytes);
                packet
            },
            MessageFromServer::ResEat { client_id, food_amount, is_ok } => {
                let mut packet = Vec::with_capacity(8);
                packet.extend(u16_be_to_bytes(6));
                packet.push(202u8);
                packet.extend(u16_be_to_bytes(client_id as u16));
                packet.extend(u16_be_to_bytes(food_amount as u16));
                packet.push(if is_ok { 1 } else { 0 });
                packet
            },
            MessageFromServer::ResDie { client_id } => {
                let mut packet = Vec::with_capacity(5);
                packet.extend(u16_be_to_bytes(3));
                packet.push(203u8);
                packet.extend(u16_be_to_bytes(client_id as u16));
                packet
            },
        }
    }

}

#[cfg(test)]
mod tests {
    use std::io;
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpStream};
    use std::thread::sleep;
    use std::time::Duration;
    use tracing::{error, info};
    use crate::network::message::message_from_client::MessageFromClient;
    use crate::network::message::message_from_server::MessageFromServer;
    use crate::network::message::worm_body::WormBody;
    use crate::network::util;

    static INIT: std::sync::Once = std::sync::Once::new();
    fn init_tracing() {
        INIT.call_once(|| {
            // initialize logging library
            tracing_subscriber::fmt()
                .with_target(true)
                .with_level(true)
                .with_thread_ids(true)
                .init();
        });
    }

    // fixture
    struct TestContext {
        stream: TcpStream,
    }

    impl TestContext {
        fn new(ip_port: &'static str) -> io::Result<Self> {
            // before each
            match TcpStream::connect(ip_port) {
                Ok(stream) => {
                    info!("connected to server..");
                    Ok(Self { stream })
                },
                Err(_) => {
                    error!("failed to connect to server..");
                    Err(io::Error::new(io::ErrorKind::ConnectionRefused, "failed to connect to server.."))
                }
            }
        }

        fn read_response(&mut self) -> MessageFromServer {
            let mut read_packet = [0u8; 1024];
            let read_count = self.stream.read(&mut read_packet).unwrap();
            let read_packet: Vec<u8> = read_packet[2..read_count].to_owned();
            // info!("read packet: {:?}", read_packet);
            MessageFromServer::new(&read_packet).unwrap()
        }
    }

    impl Drop for TestContext {
        fn drop(&mut self) {
            // after each
            info!("close ..");
            match self.stream.shutdown(Shutdown::Write) {
                Ok(_) => {},
                Err(_) => {}
            };
        }
    }

    #[test]
    fn test_join_and_leave() -> Result<(), Box<dyn std::error::Error>> {
        init_tracing();
        let mut fixture = TestContext::new("127.0.0.1:8888")?;
        let client_id = 1234;

        let packet = MessageFromClient::ReqJoin { client_id }.make_bytes();
        let _ = fixture.stream.write_all(&packet)?;
        info!("join to the game");

        info!("sleep 0.1s ..");
        sleep(Duration::from_millis(100));

        let server_message = fixture.read_response();
        info!("server message: {:?}", server_message);

        let packet = MessageFromClient::ReqLeave { client_id }.make_bytes();
        let _ = fixture.stream.write_all(&packet)?;
        info!("leave the game");

        info!("sleep 0.1s ..");
        sleep(Duration::from_millis(100));

        let server_message = fixture.read_response();
        info!("server message: {:?}", server_message);

        info!("sleep 0.1s ..");
        sleep(Duration::from_millis(100));

        Ok(())
    }

    #[test]
    fn test_req_move() -> Result<(), Box<dyn std::error::Error>> {
        init_tracing();
        let mut fixture = TestContext::new("127.0.0.1:8888")?;
        let client_id = 1234;
        let worm_body = WormBody::new(
            client_id,
            &[
                util::color_to_bytes(&(0.5019608_f32, 0.5019608_f32, 0.5019608, 1.0_f32)),
                util::positions_to_bytes(&vec![(1_f32, 1_f32), (2_f32, 2_f32), (3_f32, 3_f32)]),
            ].concat(),
        )?;

        let packet = MessageFromClient::ReqMove { client_id, worm_body }.make_bytes();
        let _ = fixture.stream.write_all(&packet);

        info!("sleep 0.1s ..");
        sleep(Duration::from_millis(100));

        let server_message = fixture.read_response();
        info!("server message: {:?}", server_message);

        info!("sleep 0.1s ..");
        sleep(Duration::from_millis(100));

        Ok(())
    }
}