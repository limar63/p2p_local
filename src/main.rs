use crate::cli::cli_loop;
use crate::tcp_shenanigans::{client_task, server_task, PeerNode};

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::{broadcast, Mutex};

mod cli;
mod messages;
mod tcp_shenanigans;

#[tokio::main]
async fn main() {
    let created_peer: PeerNode = cli_loop();

    let (message_broadcast, _): (Sender<String>, Receiver<String>) = broadcast::channel(50);
    let (address_sender, address_receiver): (
        UnboundedSender<(SocketAddr, bool)>,
        UnboundedReceiver<(SocketAddr, bool)>,
    ) = tokio::sync::mpsc::unbounded_channel();

    let addresses: Arc<Mutex<HashSet<SocketAddr>>> = Arc::new(Mutex::new(HashSet::new()));

    server_task(
        created_peer.port,
        created_peer.period,
        message_broadcast.clone(),
        Arc::clone(&addresses),
    );

    //client task gets address receiver because I decided to delegate connecting to a node to that address receiver channel
    client_task(
        address_receiver,
        address_sender.clone(),
        message_broadcast.clone(),
        Arc::clone(&addresses),
        created_peer.port,
    );
    if let Some(addr) = created_peer.peer_to_connect {
        address_sender.send((addr, true)).unwrap();
    }
    tokio::time::sleep(tokio::time::Duration::MAX).await;
}
