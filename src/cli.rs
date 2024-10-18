use crate::network::PeerNode;
use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;

fn two_argument_parse(params: Vec<&str>) -> Option<PeerNode> {
    if let Some(map_of_commands) = param_map_maker(params) {
        let maybe_port: Option<u16> = map_of_commands
            .get("port")
            .and_then(|port| port.parse::<u16>().ok());
        let maybe_period: Option<u64> = map_of_commands
            .get("period")
            .and_then(|period| period.parse::<u64>().ok());
        maybe_period.zip(maybe_port).map(|(period, port)| PeerNode {
            period,
            port,
            peer_to_connect: None,
        })
    } else {
        None
    }
}

fn three_argument_parse(params: Vec<&str>) -> Option<PeerNode> {
    if let Some(map_of_commands) = param_map_maker(params) {
        let maybe_port: Option<u16> = map_of_commands
            .get("port")
            .and_then(|port| port.parse::<u16>().ok());
        let maybe_period: Option<u64> = map_of_commands
            .get("period")
            .and_then(|period| period.parse::<u64>().ok());
        let maybe_connect: Option<SocketAddr> = map_of_commands
            .get("connect")
            .and_then(|addr| addr.replace("\"", "").parse::<SocketAddr>().ok());
        maybe_period
            .zip(maybe_port)
            .zip(maybe_connect)
            .map(|((period, port), connect)| PeerNode {
                period,
                port,
                peer_to_connect: Some(connect),
            })
    } else {
        None
    }
}

fn param_map_maker(params: Vec<&str>) -> Option<HashMap<&str, &str>> {
    let pairs: Vec<Vec<&str>> = params.iter().map(|str| str.split("=").collect()).collect();
    if pairs.iter().all(|p| p.len() == 2) {
        Some(pairs.iter().map(|p| (p[0], p[1])).into_iter().collect())
    } else {
        None
    }
}

fn figure_out_input(input: &str) -> Option<PeerNode> {
    let mut command_vector = input.split(" --").collect::<Vec<&str>>();
    if command_vector.len() > 0 {
        match (command_vector.remove(0), command_vector.len()) {
            ("./peer", 3) => three_argument_parse(command_vector),
            ("./peer", 2) => two_argument_parse(command_vector),
            _ => None,
        }
    } else {
        None
    }
}

pub fn cli_loop() -> PeerNode {
    loop {
        let mut input = String::new();

        println!("Enter the node startup command: ");

        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");

        if let Some(peer) = figure_out_input(input.trim()) {
            break peer;
        } else {
            println!("Malformed command, try again")
        }
    }
}
