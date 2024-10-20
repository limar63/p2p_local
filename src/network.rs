use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch::{Receiver, Sender};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use crate::messages::{decode_message, encode_message, HandshakeType, Req, Resp};
use crate::messages::HandshakeType::{Initial, Peer};


const LOCAL_IP: &str = "127.0.0.1";
const MESSAGE: &str = "Gossip message";

pub async fn client_task(
    mut address_receiver: UnboundedReceiver<(SocketAddr, HandshakeType)>,
    address_sender: UnboundedSender<(SocketAddr, HandshakeType)>,
    reading_sync_channel: Receiver<()>,
    addresses: Arc<Mutex<HashSet<SocketAddr>>>,
    port: u16
) {
    let client_addr = format!("{}:{}", LOCAL_IP, port)
        .parse::<SocketAddr>()
        .unwrap();
    loop {
        if let Some((server_addr, freshness)) = address_receiver.recv().await {
            let address_sender = address_sender.clone();
            let reading_sync_channel = reading_sync_channel.clone();
            let addresses = addresses.clone();
            tokio::spawn(async move {
                let (server_connection_stream, server_addr) =
                    tokio::spawn(async move {
                        initiating_handshake(server_addr, freshness, client_addr).await
                    }).await.map_err(|e|e.to_string())??;
                let (read_half, write_half, server_addr, addresses) =
                    tokio::spawn(async move {
                        reading_handshake_response(address_sender, server_connection_stream, server_addr, addresses).await
                    }).await.map_err(|e|e.to_string())??;
                let connection_maintenance =
                    tokio::spawn(async move {
                        maintaining_connection(read_half, write_half, server_addr, addresses, reading_sync_channel).await
                    }).await.map_err(|e|e.to_string())?;
                eprintln!("Connection with node {} is gone for the reason: {:?}", &server_addr, connection_maintenance);
                connection_maintenance
            });
        }
    }
}

async fn initiating_handshake(
    server_address: SocketAddr,
    freshness: HandshakeType,
    client_address: SocketAddr
)   -> Result<(TcpStream, SocketAddr), String> {
    let mut stream = TcpStream::connect(server_address).await.unwrap();
    let encoded_msg = encode_message(&Req::Handshake(client_address.clone(), freshness)).unwrap();
    stream
        .write_all(&encoded_msg)
        .await
        .map_err(|e| e.to_string()).map(|_| (stream, server_address))
}

async fn reading_handshake_response(
    address_sender: UnboundedSender<(SocketAddr, HandshakeType)>,
    mut stream: TcpStream,
    server_addr: SocketAddr,
    addresses: Arc<Mutex<HashSet<SocketAddr>>>
) -> Result<(OwnedReadHalf, OwnedWriteHalf, SocketAddr, Arc<Mutex<HashSet<SocketAddr>>>), String> {
    let mut buf = [0; 1024];
    match stream.read(&mut buf).await {
        Ok(0) => Err("connection closed".to_string()),
        Ok(n) => {
            let valid_bytes = &buf[..n];
            match decode_message(valid_bytes) {
                Ok(Resp::Handshake(set)) => {
                    addresses.lock().unwrap().insert(server_addr);
                    for addr in set {
                        address_sender.send((addr, Peer)).unwrap();
                    };
                    Ok(stream.into_split())
                },
                _ => return Err(String::from("Decoding message failed or not a handshake response sent"))
            }.map(|(r, w)| {
                println!("Connected to server node {}", server_addr.to_string()); (r, w, server_addr, addresses)})
        }
        Err(e) => Err(e.to_string()),
    }
}

pub async fn server_task(
    port: u16,
    period: u64,
    writing_sync_channel: Sender<()>,
    addresses: Arc<Mutex<HashSet<SocketAddr>>>
) {
    start_sender_task(period, writing_sync_channel.clone(), addresses.clone());
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
                let addresses = addresses.clone();
                let reading_sync_channel = writing_sync_channel.subscribe();

                // Spawn a new task to handle the connected node
                tokio::spawn(async move {
                    let (msg, client_addr, stream, addresses) =
                        tokio::spawn(async move {
                            server_handshake_reading(socket, addresses).await
                        }).await.map_err(|e| e.to_string())??;
                    let (read_half, write_half, client_addr, addresses) =
                        tokio::spawn(async move {
                            server_handshake_responding(msg, client_addr, stream, addresses).await
                        }).await.map_err(|e| e.to_string())??;
                    let connection_maintenance =
                        tokio::spawn(async move {
                            maintaining_connection(read_half, write_half, client_addr, addresses, reading_sync_channel).await
                        }).await.map_err(|e| e.to_string())?;
                    eprintln!("Connection with node {} is gone for the reason: {:?}", &client_addr, connection_maintenance);
                    connection_maintenance
                });
            }
            Err(e) => {
                eprintln!("Error accepting connection: {:?}", e);
            }
        }
    }
}

fn start_sender_task(
    seconds: u64,
    channel: Sender<()>,
    connections: Arc<Mutex<HashSet<SocketAddr>>>
) {
    let duration = Duration::from_secs(seconds);
    tokio::spawn(async move {
        loop {
            //creating scope to make sure compiler doesn't complain about std mutex
            {
                let connections = connections.lock().unwrap();
                if connections.len() > 0 {
                    match channel.send(()) {
                        Ok(_) => {
                            println!(
                                "Sending message \"{}\" to {}",
                                MESSAGE,
                                connections.iter().map(|addr| addr.to_string()).collect::<Vec<_>>().join(", ")
                            )
                        }
                        Err(e) => eprintln!("Error while sending synced write command: {}", e),
                    }
                }
            }

            tokio::time::sleep(duration).await;
        }
    });
}


async fn server_handshake_reading(
    mut socket: TcpStream,
    addresses: Arc<Mutex<HashSet<SocketAddr>>>
) -> Result<(Vec<u8>, SocketAddr, TcpStream, Arc<Mutex<HashSet<SocketAddr>>>), String> {
    let mut buf = [0; 1024];
    match socket.read(&mut buf).await {
        Ok(0) => Err("connection closed".to_string()),

        Ok(n) => {
            let valid_bytes = &buf[..n];
            match decode_message(valid_bytes) {
                Ok(Req::Handshake(addr, handshake_type)) => {
                    let set = match handshake_type {
                        Initial => addresses.lock().unwrap().clone(),
                        Peer => HashSet::new()
                    };
                    let message = encode_message(&Resp::Handshake(set)).unwrap();
                    Ok((message, addr, socket, addresses))
                }
                _ => Err(String::from("Decoding message failed or not a handshake response sent"))
            }
        }
        Err(e) => Err(e.to_string())
    }
}

async fn server_handshake_responding(
    message: Vec<u8>,
    addr: SocketAddr,
    mut socket: TcpStream,
    addresses: Arc<Mutex<HashSet<SocketAddr>>>
) -> Result<(OwnedReadHalf, OwnedWriteHalf, SocketAddr, Arc<Mutex<HashSet<SocketAddr>>>), String> {
    match socket.write_all(&*message).await {
        Ok(_) => {
            addresses.lock().unwrap().insert(addr);
            let (reader, writer) = socket.into_split();
            println!("Node {} succesfully connected", addr);
            Ok((reader, writer, addr, addresses))
        }
        Err(e) => Err(e.to_string()),
    }
}

async fn listening_to_a_node(
    mut read_stream: OwnedReadHalf,
    addr: SocketAddr,
    addresses: Arc<Mutex<HashSet<SocketAddr>>>
) -> Result<(), String> {
    let mut buf = [0; 1024];
    loop {
        match read_stream.read(&mut buf).await {
            Ok(0) => {
                addresses.lock().unwrap().remove(&addr);
                return Err(format!("Connection closed: {}", addr.to_string()));
            }

            Ok(n) => println!(
                "Received message \"{}\" from {}",
                String::from_utf8_lossy(&buf[..n]).into_owned(),
                addr
            ),
            Err(e) => {
                addresses.lock().unwrap().remove(&addr);
                return Err(format!("Failed to read from stream: {}", e.to_string()))
            }
        }
    }
}

async fn node_writing(
    mut writing_stream: OwnedWriteHalf,
    mut reading_sync_channel: Receiver<()>,
    addr: SocketAddr,
    addresses: Arc<Mutex<HashSet<SocketAddr>>>
) -> Result<(), String> {
    while reading_sync_channel.changed().await.is_ok() {
        if let Err(e) = writing_stream.write_all(MESSAGE.as_ref()).await {
            return Err(e.to_string());
        }
    }
    addresses.lock().unwrap().remove(&addr);
    Err(format!("Receiver channel for {} is not available", addr.to_string()))
}

async fn maintaining_connection(
    read_half: OwnedReadHalf,
    write_half: OwnedWriteHalf,
    peer_address: SocketAddr,
    addresses: Arc<Mutex<HashSet<SocketAddr>>>,
    reading_sync_channel: Receiver<()>
) -> Result<(), String> {
    let addresses_clone = addresses.clone();
    let write_task = tokio::spawn(async move{node_writing(write_half, reading_sync_channel, peer_address.clone(), addresses).await});
    let listening_task = tokio::spawn(async move{listening_to_a_node(read_half, peer_address, addresses_clone).await});

    let result = tokio::select! {
        res = write_task => res,
        res = listening_task => res
    };

    result.map_err(|err|err.to_string())?

}
