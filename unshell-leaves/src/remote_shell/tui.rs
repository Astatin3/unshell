//! Placeholder client-side TUI surface for the remote shell leaf.
//!
//! The first application-layer consumer will be a CLI and later a full GUI. This
//! stub keeps the leaf-specific interpretation point in place without forcing a
//! rendering-library decision yet.

use std::string::String;
use std::vec::Vec;

use unshell::protocol::DataMessage;

use crate::{LeafTui, TuiError};

/// Stub TUI surface for the remote shell leaf.
#[derive(Default)]
pub struct RemoteShellTui {
    transcript: Vec<u8>,
}

impl RemoteShellTui {
    /// Returns a short explanation of the current stub status.
    pub fn status_line(&self) -> &'static str {
        "remote shell TUI stub: rendering is placeholder-only for now"
    }
}

impl LeafTui for RemoteShellTui {
    fn leaf_name(&self) -> String {
        Self::protocol_leaf_name()
    }

    fn handle_data(&mut self, message: &DataMessage) -> Result<(), TuiError> {
        self.transcript.extend_from_slice(&message.data);
        Ok(())
    }

    fn render(&self) -> String {
        let body = String::from_utf8_lossy(&self.transcript);
        format!("{}\n\n{}", self.status_line(), body)
    }
}
