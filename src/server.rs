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
                thread::spawn(move|| {
                    stream.set_nonblocking(false)
                        .expect("setting non blocking false fail..");
                    stream.set_read_timeout(Some(Duration::from_secs(1)))
                        .expect("setting read timeout fail..");

                    handle_client(stream)
                });
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }
    drop(listener);
}

fn handle_client(mut stream: TcpStream) {
    // todo 지금은 최대 1024 바이트만 읽는데, 패킷 설계 나오면 제대로 크기만큼 읽자.
    //
    //  패킷 설계 예상. header, 게임 데이터 구조로...
    //  | --- 고정 크기 바이트 Header --- | --- 게임 데이터 --- |
    //  [ body byte length | ...    ]   [      body      ]
    //
    //  이렇게 고정 길이의 Header를 설계해서 Header 먼저 읽고,
    //  그 안에 들어있는 byte length 필드를 읽어봐서 그만큼 더 읽자.
    let mut data = [0 as u8; 1024];

    while match stream.read(&mut data) {
        Ok(0) => true,  // busy wait ... todo 어떻게 고치냐.
        Ok(size) => {
            // bytes to hex str
            let hex_str = bytes_to_hex(&data[..size]);
            println!("Received data = {} (from = {})", hex_str, stream.peer_addr().unwrap());

            // todo 클라이언트에 응답 전송하는 방법은 이렇게 하면 될듯.
            // stream.write(text.as_bytes()).unwrap();
            true
        },
        Err(_) => {
            println!("client disconnected. ({})", stream.peer_addr().unwrap());
            stream.shutdown(Shutdown::Both).unwrap();
            false
        }
    } {}
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
}