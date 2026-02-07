mod network;

use crate::network::message::message_from_client::MessageFromClient;
use crate::network::util;
use crate::network::error::NetworkError;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{error, info, warn};
use network::message;
use crate::network::message::message_from_server::MessageFromServer;
use crate::network::message::worm_body::WormBody;

#[tokio::main]
async fn main() {
    // initialize logging library
    // only needs to be called once in the main function.
    tracing_subscriber::fmt()
        .with_target(true)
        .with_level(true)
        .with_thread_ids(true)
        .init();

    let listen_port: u16 = 8888;
    let bind_info = format!("0.0.0.0:{}", listen_port);
    let listener = TcpListener::bind(&bind_info).await.unwrap();
    info!("server started. listening on {}", bind_info);

    while let Ok((socket, client_access_info)) = listener.accept().await {
        tokio::spawn(async move {
            info!("[{}] detected new client.", client_access_info);
            let _ = handle_client(socket, client_access_info).await;
            info!("[{}] client disconnected.", client_access_info);
        });
    }
    drop(listener);
}

// todo 아래 두 가지 경우를 처리해야 한다.
//  - 클라이언트에서 패킷이 온 경우 (일반적인 경우)
//  - 여러 클라이언트에게 응답을 브로드캐스팅해야 하는 경우
//  cf)
//   rust channel을 써서 각 클라이언트 스레드에 리시버를 하나씩 두고
//   tokio의 select! macro를 활용해서, 소켓 수신 future와 리시버 수신 future 간 경쟁하며 각각을 처리
async fn handle_client(mut stream: TcpStream, client_access_info: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = Vec::with_capacity(2048);
    let mut read_packet = [0u8; 1024];
    let mut eof = false;

    'outer: loop {
        // 패킷 수신
        let size = stream.read(&mut read_packet).await?;
        if size == 0 {
            // 클라이언트가 먼저 소켓 close하면 0이 된다. (클라이언트가 active closer인 경우)
            // 서버는 바로 끊어내는 것이 아니라, 혹시 아직 버퍼에 잔여 패킷이 있을 수 있으므로 확인 후 종료되도록 한다.
            eof = true;
        } else {
            // TCP는 메세지 경계가 보장되지 않으므로 새로 들어온 패킷은 일단 버퍼에 쌓아놓고, 로직에서 버퍼를 메세지 단위로 소비한다.
            // 로직에서는 메세지를 읽어낼 수 있다고 판단된 경우에만 버퍼에서 그만큼 소비하므로 패킷이 쪼개져 들어와도 상관없음.
            buffer.extend_from_slice(&read_packet[..size]);
        }

        // 루프 돌면서, 메세지를 정확히 파싱할 수 없을때까지 버퍼를 소비한다.
        loop {
            match message::validate_packet_length(&buffer) {
                Ok(remaining_byte_size) => {
                    // actual: 1,2,3,4,5 / expected: 1,2,3 => remaining: 4,5 (2개)
                    // 맨 앞 2바이트는 메세지 경계를 판단하기 위한 길이 필드. 로직에서는 필요없으므로 버린다.
                    let message_bytes = buffer.drain(..buffer.len() - remaining_byte_size)
                        .skip(2).collect::<Vec<u8>>();
                    info!("[{}] message bytes = {}", client_access_info, util::bytes_to_hex(&message_bytes));

                    // 클라이언트에서 온 메세지
                    let result = MessageFromClient::new(&message_bytes);
                    if let Err(e) = &result {
                        error!("{:?}", e);
                        continue;
                    }
                    let msg = result?;

                    // 클라이언트의 메세지에 따라 서버 응답을 생성하여 전송
                    // todo 응답 메세지 유형에 따라, 모든 유저들에게 브로드캐스트할지 해당 클라이언트에게만 응답할지 분기되어야 함.
                    let response_bytes = process_message(msg, &client_access_info).make_bytes();
                    info!("[{}] response bytes = {}", client_access_info, util::bytes_to_hex(&response_bytes));
                    let _ = stream.write_all(&response_bytes).await;
                },
                // 패킷이 아직 부족한 경우에는 아무것도 하지 않음. 필요한 경우, 얼마나 부족한지 로깅할 수 있음.
                Err(NetworkError::TooShortMsg) | Err(NetworkError::ShortMsg { .. }) => break,
                err @ _ => {
                    // 필요하면 다른 오류 타입도 추가.
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

    // 명확하게 소켓을 종료 처리
    // FIN
    if let Err(e) = stream.shutdown().await {
        warn!("[{}] failed to shutdown stream. {}", client_access_info, e);
    }
    // socket FD close
    drop(stream);

    Ok(())
}

fn process_message(msg: MessageFromClient, client_access_info: &SocketAddr) -> MessageFromServer {
    match msg {
        MessageFromClient::ReqJoin { client_id} => {
            info!("[{}] client joined to the game. (id = {})", client_access_info, client_id);
            MessageFromServer::ResJoin { client_id, worm_body: WormBody::random(client_id) }
        },
        MessageFromClient::ReqLeave { client_id } => {
            info!("[{}] client leaved to the game. (id = {})", client_access_info, client_id);
            MessageFromServer::ResLeave { client_id }
        },
        MessageFromClient::ReqMove { client_id, worm_body } => {
            info!("[{}] client moved in the game. (id = {}, positions = {:?})",
                     client_access_info, client_id, worm_body);
            MessageFromServer::ResMove { client_id, worm_body }
        },
        MessageFromClient::ReqEat { .. } => {
            todo!()
        },
        MessageFromClient::ReqDie { .. } => {
            todo!()
        },
    }
}
