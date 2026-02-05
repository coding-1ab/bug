use crate::network::error::NetworkError;
use crate::network::util::bytes_to_u16_be;

pub mod message_from_client;
pub mod message_from_server;
pub mod worm_body;

// 수신된 패킷의 길이 체크.
// 충분하면, 사용하게 될 바이트를 제외한 잔여 패킷의 사이즈를 리턴한다.
//  길이    |   유형  |   메세지
// 2 bytes | 1 byte | N bytes ..
pub fn validate_packet_length(bytes: &[u8]) -> Result<usize, NetworkError> {
    let expected_length = bytes_to_u16_be(bytes)? as usize;
    let actual_length = bytes.len().saturating_sub(2);

    // 메세지의 길이 필드에 들어있는 값만큼의 실제 바이트 배열이 들어오지 않았을 경우
    if actual_length < expected_length {
        return Err(NetworkError::ShortMsg { expected_length, actual_length, });
    }

    Ok(actual_length - expected_length)
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

    // 핏이 딱 맞는 메세지 테스트
    #[test]
    fn test_good_size_packet() -> Result<(), Box<dyn std::error::Error>> {
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

        let server_message = fixture.read_response();
        info!("server message: {:?}", server_message);

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