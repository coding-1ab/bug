mod network;

use std::io::Read;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

/*
    // 클라이언트 TCP 통신 샘플
    
    let server_address = format!("127.0.0.1:8888");
    let connection = TcpStream::connect(server_address);
    match connection {
        Ok(mut stream) => {
            println!("connected to server..");

            let bytes: [u8; 1024] = [0; 1024];
            stream.write(&bytes);

            // 바로 꺼봄.
            stream.shutdown(Shutdown::Both).expect("failed to close..");
            // sleep(Duration::from_secs(100))
        }
        Err(_) => {
            eprintln!("failed to connect to server..");
        }
    }

 */

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