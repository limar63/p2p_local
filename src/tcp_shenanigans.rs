use crate::messages::Msg;

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;

const LOCAL_IP: &str = "127.0.0.1";
const MESSAGE: &str = "ZDAROVA\n";
#[derive(Debug)]
pub(crate) struct PeerNode {
    pub(crate) period: u64,
    pub(crate) port: u16,
    pub(crate) peer_to_connect: Option<SocketAddr>,
}

pub fn client_task(
    mut address_receiver: UnboundedReceiver<(SocketAddr, bool)>,
    address_sender: UnboundedSender<(SocketAddr, bool)>,
    writing_sync_channel: Sender<String>,
    addresses: Arc<Mutex<HashSet<SocketAddr>>>,
    port: u16,
) {
    let client_addr = format!("{}:{}", LOCAL_IP, port)
        .parse::<SocketAddr>()
        .unwrap();
    tokio::spawn(async move {
        loop {
            if let Some((addr, freshness)) = address_receiver.recv().await {
                let sender_loop_clone = address_sender.clone();
                let sender = writing_sync_channel.subscribe();
                let looped_clone = Arc::clone(&addresses);
                tokio::spawn(async move {
                    if let Err(e) = start_and_handle_client_connection(
                        addr,
                        sender_loop_clone,
                        sender,
                        freshness,
                        looped_clone,
                        client_addr,
                    )
                    .await
                    {
                        eprintln!("{}", e)
                    }
                });
            }
        }
    });
}

async fn start_and_handle_client_connection(
    addr: SocketAddr,
    address_sender: UnboundedSender<(SocketAddr, bool)>,
    writing_sync_channel: Receiver<String>,
    freshness: bool,
    addresses: Arc<Mutex<HashSet<SocketAddr>>>,
    client_address: SocketAddr,
) -> Result<(), String> {
    match client_handshaking(&addr, address_sender, freshness, client_address).await {
        Ok(stream) => {
            println!("Connected to the node {}", addr.to_string());
            addresses.lock().await.insert(addr);
            let (listener, sender) = stream.into_split();
            let writing_process =
                node_writing(sender, writing_sync_channel, addr.to_string().clone());
            let listening_process = listening_to_a_node(listener, addr, addresses).await;

            writing_process.and_then(|_| listening_process)
        }
        Err(e) => Err(e),
    }
}

async fn client_handshaking(
    server_address: &SocketAddr,
    address_sender: UnboundedSender<(SocketAddr, bool)>,
    freshness: bool,
    client_address: SocketAddr,
) -> Result<TcpStream, String> {
    let mut buf = [0; 1024];
    let mut stream = TcpStream::connect(server_address).await.unwrap();
    let encoded_msg = if freshness {
        &*Msg::FirstHandClientHandshake(client_address.clone())
            .encode_message()
            .unwrap()
    } else {
        &*Msg::SecondHandClientHandshake(client_address.clone())
            .encode_message()
            .unwrap()
    };
    let reading_stream = stream
        .write_all(&encoded_msg)
        .await
        .map_err(|e| e.to_string());

    let getting_and_decoding_message = match stream.read(&mut buf).await {
        Ok(0) => Err("connection closed".to_string()),
        Ok(n) => {
            let valid_bytes = &buf[..n];
            match Msg::decode_message(valid_bytes) {
                Ok(Msg::FirstHandServerResponse(set)) => {
                    for addr in set {
                        address_sender.send((addr, false)).unwrap();
                    }
                    Ok(stream)
                }
                Ok(Msg::SecondHandServerResponse) => Ok(stream),
                _ => Ok(stream),
            }
        }
        Err(e) => Err(e.to_string()),
    };
    reading_stream.and_then(|_| getting_and_decoding_message)
}

pub fn server_task(
    port: u16,
    period: u64,
    writing_sync_channel: Sender<String>,
    addresses: Arc<Mutex<HashSet<SocketAddr>>>,
) {
    start_sender_task(period, writing_sync_channel.clone(), Arc::clone(&addresses));
    tokio::spawn(async move {
        let listener = TcpListener::bind(format!("{}:{}", LOCAL_IP, port))
            .await
            .unwrap();

        println!(
            "Node started, listening on the address on {}:{}",
            LOCAL_IP, port
        );

        loop {
            match listener.accept().await {
                Ok((socket, _)) => {
                    let loop_addresses = Arc::clone(&addresses);
                    let sender = writing_sync_channel.subscribe();

                    // Spawn a new task to handle the connection
                    tokio::spawn(async move {
                        if let Err(e) =
                            handle_server_connection(socket, loop_addresses, sender).await
                        {
                            eprintln!("{}", e)
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Error accepting connection: {:?}", e);
                }
            }
        }
    });
}

fn start_sender_task(
    seconds: u64,
    channel: Sender<String>,
    connections: Arc<Mutex<HashSet<SocketAddr>>>,
) {
    let duration = Duration::from_secs(seconds);
    tokio::spawn(async move {
        loop {
            let connections = connections.lock().await;
            if connections.len() > 0 {
                match channel.send("Send".to_string()) {
                    Ok(_) => {
                        println!(
                            "Sending message {} to {}",
                            MESSAGE,
                            format!("{:?}", *connections).replace("\"", "")
                        )
                    }
                    Err(e) => eprintln!("Error sending message {}", e),
                }
            }

            tokio::time::sleep(duration).await;
        }
    });
}

async fn handle_server_connection(
    socket: TcpStream,
    addresses: Arc<Mutex<HashSet<SocketAddr>>>,
    writing_sync_channel: Receiver<String>,
) -> Result<(), String> {
    match server_handshaking(socket, Arc::clone(&addresses)).await {
        Ok((stream, addr)) => {
            println!("Connection with the node {} established", addr.to_string());
            let (listener, writing_stream) = stream.into_split();
            let writing_process = node_writing(
                writing_stream,
                writing_sync_channel,
                addr.to_string().clone(),
            );

            let listening_process = listening_to_a_node(listener, addr, addresses).await;
            writing_process.and_then(|_| listening_process)
        }
        Err(e) => Err(e),
    }
}

async fn server_handshaking(
    mut socket: TcpStream,
    addresses: Arc<Mutex<HashSet<SocketAddr>>>,
) -> Result<(TcpStream, SocketAddr), String> {
    let mut buf = [0; 1024];
    loop {
        match socket.read(&mut buf).await {
            Ok(0) => return Err("connection closed".to_string()),

            Ok(n) => {
                let valid_bytes = &buf[..n];
                match Msg::decode_message(valid_bytes) {
                    Ok(Msg::FirstHandClientHandshake(address)) => {
                        let message = Msg::FirstHandServerResponse(addresses.lock().await.clone())
                            .encode_message()
                            .unwrap();
                        return writing_helper(socket, message, address, addresses).await;
                    }
                    Ok(Msg::SecondHandClientHandshake(address)) => {
                        let message = Msg::SecondHandServerResponse.encode_message().unwrap();
                        return writing_helper(socket, message, address, addresses).await;
                    }
                    _ => continue,
                }
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

async fn listening_to_a_node(
    mut read_stream: OwnedReadHalf,
    addr: SocketAddr,
    addresses: Arc<Mutex<HashSet<SocketAddr>>>,
) -> Result<(), String> {
    let mut buf = [0; 1024];
    loop {
        match read_stream.read(&mut buf).await {
            Ok(0) => {
                addresses.lock().await.remove(&addr);
                return Err(format!("Connection closed: {}", addr.to_string()));
            }

            Ok(n) => println!(
                "Received message {} from {}",
                String::from_utf8_lossy(&buf[..n]).into_owned(),
                addr
            ),

            Err(e) => return Err(format!("Failed to read from stream: {}", e.to_string())),
        }
    }
}
fn node_writing(
    mut writing_stream: OwnedWriteHalf,
    mut writing_sync_channel: Receiver<String>,
    addr: String,
) -> Result<(), String> {
    tokio::spawn(async move {
        loop {
            match writing_sync_channel.recv().await {
                Ok(msg) => {
                    if msg == "Send" {
                        if let Err(e) = writing_stream.write_all(MESSAGE.as_ref()).await {
                            return Err(e.to_string());
                        }
                    }
                }
                Err(e) => {
                    return Err::<(), String>(format!(
                        "Error sending message {} to the socket {}",
                        e, addr
                    ))
                }
            }
        }
    });
    Ok(())
}

async fn writing_helper(
    mut socket: TcpStream,
    message: Vec<u8>,
    addr: SocketAddr,
    addresses: Arc<Mutex<HashSet<SocketAddr>>>,
) -> Result<(TcpStream, SocketAddr), String> {
    match socket.write_all(&*message).await {
        Ok(_) => {
            addresses.lock().await.insert(addr);
            Ok((socket, addr))
        }
        Err(e) => Err(e.to_string()),
    }
}
