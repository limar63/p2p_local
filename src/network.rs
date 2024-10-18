use crate::messages::Msg;

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch::{Receiver, Sender};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;

const LOCAL_IP: &str = "127.0.0.1";
const MESSAGE: &str = "Gossip message";
#[derive(Debug)]
pub(crate) struct PeerNode {
    pub(crate) period: u64,
    pub(crate) port: u16,
    pub(crate) peer_to_connect: Option<SocketAddr>,
}

pub fn client_task(
    mut address_receiver: UnboundedReceiver<(SocketAddr, bool)>,
    address_sender: UnboundedSender<(SocketAddr, bool)>,
    reading_sync_channel: Receiver<()>,
    addresses: Arc<Mutex<HashSet<SocketAddr>>>,
    port: u16,
) {
    let client_addr = format!("{}:{}", LOCAL_IP, port)
        .parse::<SocketAddr>()
        .unwrap();
    tokio::spawn(async move {
        loop {
            if let Some((server_addr, freshness)) = address_receiver.recv().await {
                let address_sender = address_sender.clone();
                let reading_sync_channel = reading_sync_channel.clone();
                let addresses = addresses.clone();
                tokio::spawn(async move {
                    let (server_connection_stream, server_addr) =
                        initiating_handshake(server_addr, freshness, client_addr).await.map_err(|_| "join_error")??;
                    let (read_half, write_half, server_addr) =
                        reading_handshake_response(address_sender, server_connection_stream, server_addr).await.map_err(|_| "join_error")??;
                    let connection_maintenance =
                        maintaining_connection(read_half, write_half, server_addr, addresses, reading_sync_channel).await.map_err(|_| "join_error")?;
                    eprintln!("Connection with node {} is gone for the reason: {:?}", &server_addr, connection_maintenance);
                    connection_maintenance
                });
            }
        }
    });
}

fn initiating_handshake(
    server_address: SocketAddr,
    freshness: bool,
    client_address: SocketAddr,
)   -> JoinHandle<Result<(TcpStream, SocketAddr), String>> {
    tokio::spawn(async move {
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
        stream
            .write_all(&encoded_msg)
            .await
            .map_err(|e| e.to_string()).map(|_| (stream, server_address))
    })
}

fn reading_handshake_response(
    address_sender: UnboundedSender<(SocketAddr, bool)>,
    mut stream: TcpStream,
    server_addr: SocketAddr
) -> JoinHandle<Result<(OwnedReadHalf, OwnedWriteHalf, SocketAddr), String>> {
    tokio::spawn(async move {
        let mut buf = [0; 1024];
        match stream.read(&mut buf).await {
            Ok(0) => Err("connection closed".to_string()),
            Ok(n) => {
                let valid_bytes = &buf[..n];
                match Msg::decode_message(valid_bytes) {
                    Ok(Msg::FirstHandServerResponse(set)) => {
                        for addr in set {
                            address_sender.send((addr, false)).unwrap();
                        }
                        Ok(stream.into_split())
                    }
                    Ok(Msg::SecondHandServerResponse) => Ok(stream.into_split()),
                    _ => Ok(stream.into_split()), //this message shouldn't happen
                }.map(|(r, w)| {
                    println!("Connected to server node {}", server_addr.to_string()); (r, w, server_addr)})
            }
            Err(e) => Err(e.to_string()),
        }
    })

}

pub fn server_task(
    port: u16,
    period: u64,
    writing_sync_channel: Sender<()>,
    addresses: Arc<Mutex<HashSet<SocketAddr>>>,
) {
    start_sender_task(period, writing_sync_channel.clone(), addresses.clone());
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
                    let addresses = addresses.clone();
                    let writing_sync_channel = writing_sync_channel.subscribe();

                    // Spawn a new task to handle the connected node
                    tokio::spawn(async move {
                        let (msg, client_addr, stream, addresses) =
                            server_handshake_reading(socket, addresses).await.map_err(|_| "join_error")??;
                        let (read_half, write_half, client_addr, addresses) =
                            server_handshake_responding(msg, client_addr, stream, addresses).await.map_err(|_| "join_error")??;
                        let connection_maintenance =
                            maintaining_connection(read_half, write_half, client_addr,addresses, writing_sync_channel).await.map_err(|_| "join_error")?;
                        eprintln!("Connection with node {} is gone for the reason: {:?}", &client_addr, connection_maintenance);
                        connection_maintenance
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
    channel: Sender<()>,
    connections: Arc<Mutex<HashSet<SocketAddr>>>,
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
                                "Sending message {} to {}",
                                MESSAGE,
                                format!("{:?}", *connections).replace("\"", "")
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


fn server_handshake_reading(
    mut socket: TcpStream,
    addresses: Arc<Mutex<HashSet<SocketAddr>>>,
) -> JoinHandle<Result<(Vec<u8>, SocketAddr, TcpStream, Arc<Mutex<HashSet<SocketAddr>>>), String>> {
    tokio::spawn(async move {
        let mut buf = [0; 1024];
        loop {
            match socket.read(&mut buf).await {
                Ok(0) => return Err("connection closed".to_string()),

                Ok(n) => {
                    let valid_bytes = &buf[..n];
                    match Msg::decode_message(valid_bytes) {
                        Ok(Msg::FirstHandClientHandshake(address)) => {
                            let message = Msg::FirstHandServerResponse(addresses.lock().unwrap().clone())
                                .encode_message()
                                .unwrap();
                            return Ok((message, address, socket, addresses))
                        }
                        Ok(Msg::SecondHandClientHandshake(address)) =>
                            return Ok((Msg::SecondHandServerResponse.encode_message().unwrap(), address, socket, addresses)),

                        _ => continue,
                    }
                }
                Err(e) => return Err(e.to_string()),
            }
        }
    })
}

fn server_handshake_responding(
    message: Vec<u8>,
    addr: SocketAddr,
    mut socket: TcpStream,
    addresses: Arc<Mutex<HashSet<SocketAddr>>>
) -> JoinHandle<Result<(OwnedReadHalf, OwnedWriteHalf, SocketAddr, Arc<Mutex<HashSet<SocketAddr>>>), String>> {
    tokio::spawn(async move {
        match socket.write_all(&*message).await {
            Ok(_) => {
                addresses.lock().unwrap().insert(addr);
                let (reader, writer) = socket.into_split();
                println!("Node {} succesfully connected", addr);
                Ok((reader, writer, addr, addresses))
            }
            Err(e) => Err(e.to_string()),
        }
    })

}

fn listening_to_a_node(
    mut read_stream: OwnedReadHalf,
    addr: SocketAddr,
    addresses: Arc<Mutex<HashSet<SocketAddr>>>,
) -> JoinHandle<Result<(), String>> {
    tokio::spawn(async move {
        let mut buf = [0; 1024];
        loop {
            match read_stream.read(&mut buf).await {
                Ok(0) => {
                    addresses.lock().unwrap().remove(&addr);
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
    })
}
fn node_writing(
    mut writing_stream: OwnedWriteHalf,
    mut reading_sync_channel: Receiver<()>,
    addr: String,
) -> JoinHandle<Result<(), String>> {
    tokio::spawn(async move {
        while reading_sync_channel.changed().await.is_ok() {
            if let Err(e) = writing_stream.write_all(MESSAGE.as_ref()).await {
                return Err(e.to_string());
            }
        }
        Err(format!("Receiver channel for {} is not available", addr))
    })
}

fn maintaining_connection(
    read_half: OwnedReadHalf,
    write_half: OwnedWriteHalf,
    peer_address: SocketAddr,
    addresses: Arc<Mutex<HashSet<SocketAddr>>>,
    reading_sync_channel: Receiver<()>) -> JoinHandle<Result<(), String>> {
    tokio::spawn(async move {
        let write_task = node_writing(write_half, reading_sync_channel, peer_address.to_string().clone());
        let listening_task = listening_to_a_node(read_half, peer_address, addresses);

        let result = tokio::select! {
            writing = write_task => writing,
            reading = listening_task => reading
        };

        result.map_err(|err|err.to_string()).and_then(|res| res)
    })
}
