use crate::cli::cli_loop;
use crate::network::{client_task, server_task};

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::{watch};
use tokio::sync::watch::{Receiver, Sender};
use crate::messages::HandshakeType;
use crate::messages::HandshakeType::Initial;

mod cli;
mod messages;
mod network;

#[tokio::main]
async fn main() {
    let (period, port, maybe_connection) = cli_loop();

    let (tx, rx): (Sender<()>, Receiver<()>) = watch::channel(());
    let (address_sender, address_receiver): (
        UnboundedSender<(SocketAddr, HandshakeType)>,
        UnboundedReceiver<(SocketAddr, HandshakeType)>,
    ) = tokio::sync::mpsc::unbounded_channel();
    let sender_clone = address_sender.clone();

    let addresses: Arc<Mutex<HashSet<SocketAddr>>> = Arc::new(Mutex::new(HashSet::new()));
    let addresses_clone: Arc<Mutex<HashSet<SocketAddr>>> = addresses.clone();


    if let Some(addr) = maybe_connection {
        sender_clone.send((addr, Initial)).unwrap();
    }
    //client task gets address receiver because I decided to delegate connecting to a node to that address receiver channel
    tokio::spawn(async move {client_task(
        address_receiver,
        address_sender.clone(),
        rx.clone(),
        addresses_clone,
        port,
    ).await});

    tokio::spawn(async move {server_task(
        port,
        period,
        tx.clone(),
        addresses,
    ).await}).await.unwrap();
}
