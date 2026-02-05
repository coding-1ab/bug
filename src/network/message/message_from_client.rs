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

#[cfg(test)]
mod tests {
    use std::io;
    use std::io::Write;
    use std::net::{Shutdown, TcpStream};
    use std::thread::sleep;
    use std::time::Duration;
    use tracing::{error, info};
    use crate::network::message::message_from_client::MessageFromClient;
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

    // 핏이 딱 맞는 메세지 테스트
    #[test]
    fn test_good_size_packet() -> Result<(), Box<dyn std::error::Error>> {
        init_tracing();
        let mut fixture = TestContext::new("127.0.0.1:8888")?;
        let client_id = 1234;

        let packet = MessageFromClient::ReqJoin { client_id }.make_bytes();
        let _ = fixture.stream.write_all(&packet)?;
        info!("join to the game");

        let packet = MessageFromClient::ReqLeave { client_id }.make_bytes();
        let _ = fixture.stream.write_all(&packet)?;
        info!("leave the game");

        info!("sleep 0.1s ..");
        sleep(Duration::from_millis(100));

        Ok(())
    }

    // 패킷이 두개로 파편화되는 경우 테스트
    #[test]
    fn test_divided_2_packets() -> Result<(), Box<dyn std::error::Error>> {
        init_tracing();
        let mut fixture = TestContext::new("127.0.0.1:8888")?;
        let client_id = 1234;

        let packet = MessageFromClient::ReqJoin { client_id }.make_bytes();
        let _ = fixture.stream.write_all(&packet[..2]);

        info!("sleep 0.1s ..");
        sleep(Duration::from_millis(100));

        let _ = fixture.stream.write_all(&packet[2..]);

        info!("sleep 0.1s ..");
        sleep(Duration::from_millis(100));

        Ok(())
    }

    // 패킷이 3개로 파편화되는 경우 테스트
    #[test]
    fn test_divided_3_packets() -> Result<(), Box<dyn std::error::Error>> {
        init_tracing();
        let mut fixture = TestContext::new("127.0.0.1:8888")?;
        let client_id = 1234;

        let packet = MessageFromClient::ReqJoin { client_id }.make_bytes();
        let _ = fixture.stream.write_all(&packet[..2]);

        info!("sleep 0.1s ..");
        sleep(Duration::from_millis(100));

        let _ = fixture.stream.write_all(&[packet[2]]);

        info!("sleep 0.1s ..");
        sleep(Duration::from_millis(100));

        let _ = fixture.stream.write_all(&packet[3..]);

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

        Ok(())
    }
}