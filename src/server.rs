mod network;

use std::io::Read;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::thread;

fn main() {
    let listen_port: u16 = 8888;
    let bind_info = format!("0.0.0.0:{}", listen_port);
    let listener = TcpListener::bind(bind_info).unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("detected new client. ({})", stream.peer_addr().unwrap());
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

    let mut buffer = Vec::with_capacity(2048);
    let mut read_packet = [0u8; 1024];

    'top_loop: while match stream.read(&mut read_packet) {
        Ok(0) => {
            println!("client disconnected. ({})", client_access_info);
            false
        },
        Ok(size) => {
            // 새로 들어온 패킷은 버퍼에 쌓아놓고, 로직은 버퍼를 사용한다.
            // 이전에 들어온 패킷에서 남는 패킷이 존재하는 경우, 쌓아놓고 다음 분석에 사용되도록 함.
            buffer.extend_from_slice(&read_packet[..size]);

            // bytes to hex str
            let hex_str = bytes_to_hex(&buffer);
            println!("received data = {} (from = {})", hex_str, client_access_info);

            match network::validator::validate_packet_length(&buffer) {
                Ok(remaining_byte_size) => {
                    // actual: 1,2,3,4,5 / expected: 1,2,3 => remaining: 4,5 (2개)
                    let message_bytes: Vec<u8> = buffer.drain(..buffer.len() - remaining_byte_size)
                        .skip(2)
                        .collect();
                    println!("message bytes = {}", bytes_to_hex(&message_bytes));

                    let result = network::MessageFromClient::new(&message_bytes);
                    if let Err(e) = &result {
                        eprintln!("{:?}", e);
                        continue 'top_loop;
                    }

                    // todo ...
                    let msg = result.unwrap();

                    // todo 클라이언트에 응답 전송하는 방법은 이렇게 하면 될듯.
                    //  stream.write(text.as_bytes()).unwrap();

                    println!("{:?}", buffer);

                    true
                },
                Err(short_msg_error @ network::error::NetworkError::ShortMsg{expected_length, actual_length}) => {
                    // 패킷이 아직 부족한 경우에는 아무것도 하지 않음.
                    println!("{}", short_msg_error);
                    true
                }
                _ => {
                    // todo 다른 오류 타입도 추가.
                    false
                }
            }
        },
        Err(_) => {
            println!("client disconnected. ({})", client_access_info);
            if let Err(e) = stream.shutdown(Shutdown::Both) {
                eprintln!("failed to shutdown stream: {}", e);
            }
            false
        }
    } {}
}

fn bytes_to_hex(bytes: &Vec<u8>) -> String {
    bytes.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
}

#[cfg(test)]
mod tests {
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

        let mut bytes: [u8; 7] = [0; 7];
        bytes[0] = 0;
        bytes[1] = 5;
        bytes[2] = 101;
        bytes[3] = 97;
        bytes[4] = 98;
        bytes[5] = 99;
        bytes[6] = 100;
        let _ = fixture.stream.write(&bytes);

        Ok(())
    }

    // 패킷이 두개로 파편화되는 경우 테스트
    #[test]
    fn test_divided_2_packets() -> Result<(), Box<dyn std::error::Error>> {
        let mut fixture = TestContext::new("127.0.0.1:8888")?;

        // 총 7바이트 중 첫 3바이트..
        let mut bytes: [u8; 3] = [0; 3];
        bytes[0] = 0;
        bytes[1] = 5;
        bytes[2] = 101;
        let _ = fixture.stream.write(&bytes);

        println!("sleep 0.5s ..");
        sleep(Duration::from_millis(500));

        // 총 7바이트 중 이후 4바이트..
        let mut bytes: [u8; 4] = [0; 4];
        bytes[0] = 97;
        bytes[1] = 98;
        bytes[2] = 99;
        bytes[3] = 100;
        let _ = fixture.stream.write(&bytes);

        Ok(())
    }

    // 패킷이 3개로 파편화되는 경우 테스트
    #[test]
    fn test_divided_3_packets() -> Result<(), Box<dyn std::error::Error>> {
        let mut fixture = TestContext::new("127.0.0.1:8888")?;

        // 총 7바이트 중 첫 2바이트..
        let mut bytes: [u8; 2] = [0; 2];
        bytes[0] = 0;
        bytes[1] = 5;
        let _ = fixture.stream.write(&bytes);

        println!("sleep 0.5s ..");
        sleep(Duration::from_millis(500));

        // 총 7바이트 중 1바이트..
        let mut bytes: [u8; 1] = [0; 1];
        bytes[0] = 101;
        let _ = fixture.stream.write(&bytes);

        println!("sleep 0.5s ..");
        sleep(Duration::from_millis(500));

        // 총 7바이트 중 이후 4바이트..
        let mut bytes: [u8; 4] = [0; 4];
        bytes[0] = 97;
        bytes[1] = 98;
        bytes[2] = 99;
        bytes[3] = 100;
        let _ = fixture.stream.write(&bytes);

        Ok(())
    }
}