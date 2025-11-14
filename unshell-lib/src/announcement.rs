use bincode::{Decode, Encode};

use crate::config::RuntimeConfig;

#[derive(Debug, Encode, Decode)]
pub enum Announcement {
    TestAnnouncement(String),

    GetRuntimes,
    GetRuntimesAck(usize),

    StartRuntime(RuntimeConfig),
    StartRuntimeAck(bool),
}

const BINCODE_CONFIG: bincode::config::Configuration = bincode::config::standard();

impl Announcement {
    pub fn encode(&self) -> Vec<u8> {
        bincode::encode_to_vec(self, BINCODE_CONFIG).unwrap()
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if let Ok((decoded, _)) = bincode::decode_from_slice(&bytes[..], BINCODE_CONFIG) {
            Some(decoded)
        } else {
            None
        }
    }
}
