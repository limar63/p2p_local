use bincode::error::DecodeError;
use bincode::{config, Decode, Encode};
use std::collections::HashSet;
use std::error::Error;
use std::net::SocketAddr;

#[derive(Encode, Decode, Debug)]
pub enum Req {
    Handshake(SocketAddr, HandshakeType),
    Message(String)
}

#[derive(Encode, Decode, Debug)]
pub enum HandshakeType {
    Initial,
    Peer
}

#[derive(Encode, Decode, Debug)]
pub enum Resp {
    Handshake(HashSet<SocketAddr>)
}

pub fn encode_message<T: Encode>(message: &T) -> Result<Vec<u8>, Box<dyn Error>> {
    let config = config::standard();
    let mut encoded_bytes = Vec::new();
    bincode::encode_into_std_write(message, &mut encoded_bytes, config)?;
    Ok(encoded_bytes)
}

// Generic function to decode any type that implements `Decode`
pub fn decode_message<T: Decode>(bytes: &[u8]) -> Result<T, DecodeError> {
    let config = config::standard();
    let (decoded_msg, _) = bincode::decode_from_slice(bytes, config)?;
    Ok(decoded_msg)
}