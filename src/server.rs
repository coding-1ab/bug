mod network;

use crate::network::message::message_from_client::MessageFromClient;
use crate::network::util;
use crate::network::error::NetworkError;
use std::io::Read;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::thread;
use tracing::{error, info, warn};
use network::message;

fn main() {
    // initialize logging library
    // only needs to be called once in the main function.
    tracing_subscriber::fmt()
        .with_target(true)
        .with_level(true)
        .with_thread_ids(true)
        .init();

    let listen_port: u16 = 8888;
    let bind_info = format!("0.0.0.0:{}", listen_port);
    let listener = TcpListener::bind(&bind_info).unwrap();
    info!("server started. listening on {}", bind_info);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(move|| handle_client(stream));
            }
            Err(e) => {
                error!("error: {}", e);
            }
        }
    }
    drop(listener);
}

fn handle_client(mut stream: TcpStream) {
    let client_access_info = stream.peer_addr().unwrap();
    info!("[{}] detected new client.", client_access_info);

    let mut buffer = Vec::with_capacity(2048);
    let mut read_packet = [0u8; 1024];
    let mut eof = false;

    'outer: loop {
        match stream.read(&mut read_packet) {
            Ok(0) => {
                info!("[{}] client disconnected by peer.", client_access_info);
                eof = true;
            },
            Ok(size) => {
                // 새로 들어온 패킷은 버퍼에 쌓아놓고, 로직은 버퍼를 사용한다.
                // 이전에 들어온 패킷에서 남는 패킷이 존재하는 경우, 쌓아놓고 다음 분석에 사용되도록 함.
                buffer.extend_from_slice(&read_packet[..size]);

                // bytes to hex str
                info!("[{}] packet received. (current buffer = {})", client_access_info, util::bytes_to_hex(&buffer));
            },
            Err(_) => {
                info!("[{}] client disconnected.", client_access_info);
                break 'outer;
            }
        }

        loop {
            match message::validate_packet_length(&buffer) {
                Ok(remaining_byte_size) => {
                    // actual: 1,2,3,4,5 / expected: 1,2,3 => remaining: 4,5 (2개)
                    let message_bytes = buffer.drain(..buffer.len() - remaining_byte_size)
                        .skip(2).collect::<Vec<u8>>();
                    info!("[{}] message bytes = {}", client_access_info, util::bytes_to_hex(&message_bytes));

                    let result = MessageFromClient::new(&message_bytes);
                    if let Err(e) = &result {
                        error!("{:?}", e);
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
                    error!("[{}] unexpected situation. (error: {:?})", client_access_info, err);
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
        error!("[{}] dropping incomplete buffer. (buffer = {})", client_access_info, util::bytes_to_hex(&buffer));
    }

    // 명확하게 소켓을 종료 처리 시도
    if let Err(e) = stream.shutdown(Shutdown::Both) {
        warn!("[{}] failed to shutdown stream. {}", client_access_info, e);
    }
}

// fn process_message(msg: MessageFromClient) -> MessageFromServer {
fn process_message(msg: MessageFromClient, client_access_info: &SocketAddr) {
    match msg {
        MessageFromClient::ReqJoin { client_id} => {
            info!("[{}] client joined to the game. (id = {})", client_access_info, client_id);
        },
        MessageFromClient::ReqLeave { client_id } => {
            info!("[{}] client leaved to the game. (id = {})", client_access_info, client_id);
        },
        MessageFromClient::ReqMove { client_id, worm_body } => {
            info!("[{}] client moved in the game. (id = {}, positions = {:?})",
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
