mod network;

use crate::network::MessageFromClient;
use crate::network::util;
use crate::network::error::NetworkError;
use std::io::Read;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::thread;

fn main() {
    let listen_port: u16 = 8888;
    let bind_info = format!("0.0.0.0:{}", listen_port);
    let listener = TcpListener::bind(bind_info).unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(move|| handle_client(stream));
            }
            Err(e) => {
                eprintln!("error: {}", e);
            }
        }
    }
    drop(listener);
}

fn handle_client(mut stream: TcpStream) {
    let client_access_info = stream.peer_addr().unwrap();
    println!("[{}] detected new client.", client_access_info);

    let mut buffer = Vec::with_capacity(2048);
    let mut read_packet = [0u8; 1024];
    let mut eof = false;

    'outer: loop {
        match stream.read(&mut read_packet) {
            Ok(0) => {
                println!("[{}] client disconnected by peer.", client_access_info);
                eof = true;
            },
            Ok(size) => {
                // 새로 들어온 패킷은 버퍼에 쌓아놓고, 로직은 버퍼를 사용한다.
                // 이전에 들어온 패킷에서 남는 패킷이 존재하는 경우, 쌓아놓고 다음 분석에 사용되도록 함.
                buffer.extend_from_slice(&read_packet[..size]);

                // bytes to hex str
                println!("[{}] packet received. (current buffer = {})", client_access_info, util::bytes_to_hex(&buffer));
            },
            Err(_) => {
                println!("[{}] client disconnected.", client_access_info);
                break 'outer;
            }
        }

        loop {
            match network::validator::validate_packet_length(&buffer) {
                Ok(remaining_byte_size) => {
                    // actual: 1,2,3,4,5 / expected: 1,2,3 => remaining: 4,5 (2개)
                    let message_bytes = buffer.drain(..buffer.len() - remaining_byte_size)
                        .skip(2).collect::<Vec<u8>>();
                    println!("[{}] message bytes = {}", client_access_info, util::bytes_to_hex(&message_bytes));

                    let result = MessageFromClient::new(&message_bytes);
                    if let Err(e) = &result {
                        eprintln!("{:?}", e);
                        continue;
                    }

                    let msg = result.unwrap();

                    // todo ...
                    process_message(msg, &client_access_info);
                },
                // 패킷이 아직 부족한 경우에는 아무것도 하지 않음. 필요한 경우, 얼마나 부족한지 로깅할 수 있음.
                Err(NetworkError::TooShortMsg) | Err(NetworkError::ShortMsg { .. }) => break,
                err @ _ => {
                    // todo 다른 오류 타입도 추가.
                    eprintln!("[{}] unexpected situation. (error: {:?})", client_access_info, err);
                    break 'outer;
                }
            }
        }

        if eof {
            break;
        }
    }

    // 버퍼가 아직 남아있음에도 통신을 종료하게되는 경우에는 남은 버퍼를 로깅
    if !buffer.is_empty() {
        // bytes to hex str
        eprintln!("[{}] dropping incomplete buffer. (buffer = {})", client_access_info, util::bytes_to_hex(&buffer));
    }

    // 명확하게 소켓을 종료 처리 시도
    if let Err(e) = stream.shutdown(Shutdown::Both) {
        eprintln!("[{}] failed to shutdown stream. {}", client_access_info, e);
    }
}

// fn process_message(msg: MessageFromClient) -> MessageFromServer {
fn process_message(msg: MessageFromClient, client_access_info: &SocketAddr) {
    match msg {
        MessageFromClient::ReqJoin { client_id} => {
            println!("[{}] client joined to the game. (id = {})", client_access_info, client_id);
        },
        MessageFromClient::ReqLeave { client_id } => {
            println!("[{}] client leaved to the game. (id = {})", client_access_info, client_id);
        },
        MessageFromClient::ReqMove { client_id, worm_body } => {
            println!("[{}] client moved in the game. (id = {}, positions = {:?})",
                     client_access_info, client_id, worm_body);
        },
        MessageFromClient::ReqEat { .. } => {
            // todo ..
        },
        MessageFromClient::ReqDie { .. } => {
            // todo ..
        },
    }
}

#[cfg(test)]
mod tests {
    use crate::network::{util, MessageFromClient, WormBody};
    use std::io;
    use std::io::Write;
    use std::net::{Shutdown, TcpStream};
    use std::thread::sleep;
    use std::time::Duration;

    // fixture
    struct TestContext {
        stream: TcpStream,
    }

    impl TestContext {
        fn new(ip_port: &'static str) -> io::Result<Self> {
            // before each
            match TcpStream::connect(ip_port) {
                Ok(stream) => {
                    println!("connected to server..");
                    Ok(Self { stream })
                },
                Err(_) => {
                    eprintln!("failed to connect to server..");
                    Err(io::Error::new(io::ErrorKind::ConnectionRefused, "failed to connect to server.."))
                }
            }
        }
    }

    impl Drop for TestContext {
        fn drop(&mut self) {
            // after each
            println!("close ..");
            match self.stream.shutdown(Shutdown::Write) {
                Ok(_) => {},
                Err(_) => {}
            };
        }
    }

    // 핏이 딱 맞는 메세지 테스트
    #[test]
    fn test_good_size_packet() -> Result<(), Box<dyn std::error::Error>> {
        let mut fixture = TestContext::new("127.0.0.1:8888")?;
        let client_id = 1234;

        let packet = MessageFromClient::ReqJoin { client_id }.make_bytes();
        let _ = fixture.stream.write_all(&packet)?;
        println!("join to the game");

        let packet = MessageFromClient::ReqLeave { client_id }.make_bytes();
        let _ = fixture.stream.write_all(&packet)?;
        println!("leave the game");

        println!("sleep 0.1s ..");
        sleep(Duration::from_millis(100));

        Ok(())
    }

    // 패킷이 두개로 파편화되는 경우 테스트
    #[test]
    fn test_divided_2_packets() -> Result<(), Box<dyn std::error::Error>> {
        let mut fixture = TestContext::new("127.0.0.1:8888")?;
        let client_id = 1234;

        let packet = MessageFromClient::ReqJoin { client_id }.make_bytes();
        let _ = fixture.stream.write_all(&packet[..2]);

        println!("sleep 0.1s ..");
        sleep(Duration::from_millis(100));

        let _ = fixture.stream.write_all(&packet[2..]);

        println!("sleep 0.1s ..");
        sleep(Duration::from_millis(100));

        Ok(())
    }

    // 패킷이 3개로 파편화되는 경우 테스트
    #[test]
    fn test_divided_3_packets() -> Result<(), Box<dyn std::error::Error>> {
        let mut fixture = TestContext::new("127.0.0.1:8888")?;
        let client_id = 1234;

        let packet = MessageFromClient::ReqJoin { client_id }.make_bytes();
        let _ = fixture.stream.write_all(&packet[..2]);

        println!("sleep 0.1s ..");
        sleep(Duration::from_millis(100));

        let _ = fixture.stream.write_all(&[packet[2]]);

        println!("sleep 0.1s ..");
        sleep(Duration::from_millis(100));

        let _ = fixture.stream.write_all(&packet[3..]);

        println!("sleep 0.1s ..");
        sleep(Duration::from_millis(100));

        Ok(())
    }

    #[test]
    fn test_req_move() -> Result<(), Box<dyn std::error::Error>> {
        let mut fixture = TestContext::new("127.0.0.1:8888")?;
        let client_id = 1234;
        let worm_body = WormBody::new(
            client_id,
            &util::positions_to_bytes(&vec![(1u64, 1u64), (2u64, 2u64), (3u64, 3u64)])
        )?;

        let packet = MessageFromClient::ReqMove { client_id, worm_body }.make_bytes();
        let _ = fixture.stream.write_all(&packet);

        println!("sleep 0.1s ..");
        sleep(Duration::from_millis(100));

        Ok(())
    }
}