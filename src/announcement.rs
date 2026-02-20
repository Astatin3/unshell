//! Announcement message types for server communication.
//!
//! This module defines message types for inter-component communication.
//! Currently minimal - most functionality is handled by the tree system.
//!
//! # Usage
//!
//! ```rust
//! use unshell::Announcement;
//!
//! let msg = Announcement::TestAnnouncement("Hello".to_string());
//! ```

/// Server message types for runtime communication.
///
/// These were previously used for binary encoding with bincode.
/// Currently unused - tree-based messaging handles all communication.
// #[derive(Clone, Debug, Encode, Decode)]
pub enum Announcement {
    /// Test announcement with string payload
    TestAnnouncement(String),
    // GetRuntimes,
    // GetRuntimesAck(usize),
    // StartRuntime(RuntimeConfig),
    // StartRuntimeAck(bool),
}

// const BINCODE_CONFIG: bincode::config::Configuration = bincode::config::standard();

// impl Announcement {
//     pub fn encode(&self) -> Vec<u8> {
//         bincode::encode_to_vec(self, BINCODE_CONFIG).unwrap()
//     }

//     pub fn decode(bytes: &[u8]) -> Option<Self> {
//         if let Ok((decoded, _)) = bincode::decode_from_slice(&bytes[..], BINCODE_CONFIG) {
//             Some(decoded)
//         } else {
//             None
//         }
//     }
// }
