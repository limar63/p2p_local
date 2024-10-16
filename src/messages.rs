use bincode::error::DecodeError;
use bincode::{config, Decode, Encode};
use std::collections::HashSet;
use std::error::Error;
use std::net::SocketAddr;

#[derive(Encode, Decode, Debug)]
pub enum Msg {
    FirstHandClientHandshake(SocketAddr),
    SecondHandClientHandshake(SocketAddr),
    FirstHandServerResponse(HashSet<SocketAddr>),
    SecondHandServerResponse,
    Message(String),
}

impl Msg {
    pub fn encode_message(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        let config = config::standard();
        let mut encoded_bytes = Vec::new();
        bincode::encode_into_std_write(self, &mut encoded_bytes, config)?;
        Ok(encoded_bytes)
    }

    pub fn decode_message(bytes: &[u8]) -> Result<Msg, DecodeError> {
        let config = config::standard();
        let (decoded_msg, _) = bincode::decode_from_slice(bytes, config)?;
        Ok(decoded_msg)
    }
}
