//! Remote shell leaf and its user-facing surfaces.
//!
//! The module always exports the protocol contract for the leaf. Role-specific
//! implementations live behind crate-wide features:
//! - `endpoint` builds the PTY-backed runtime leaf
//! - `tui` builds a placeholder client-side TUI surface

use rkyv::{Archive, Deserialize, Serialize};

#[cfg(feature = "endpoint")]
mod endpoint;
#[cfg(feature = "tui")]
mod tui;

#[cfg(feature = "endpoint")]
pub use endpoint::{
    LISTEN_ADDR, RemoteShellEndpoint, ShellLeafError, build_agent_runtime,
    build_controller_endpoint, send_forward, spawn_frame_reader, write_frames,
};
#[cfg(feature = "tui")]
pub use tui::RemoteShellTui;

use unshell::protocol::tree::encode_call_reply;

/// Open-request payload for the remote shell leaf.
///
/// The shell currently needs no structured arguments, but a named payload type is
/// easier for downstream code to discover than a bare `()`.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenRequest;

crate::role_leaf! {
    /// Feature-selected remote shell surface.
    pub type RemoteShell {
        endpoint => endpoint::RemoteShellEndpoint,
        tui => tui::RemoteShellTui,
    }
}

/// Returns the example endpoint path used by the remote shell samples.
pub fn agent_path() -> Vec<String> {
    path(&["agent"])
}

/// Returns the canonical leaf id used by endpoint and TUI code.
#[cfg(feature = "endpoint")]
pub fn shell_leaf_name() -> String {
    RemoteShellEndpoint::protocol_leaf_name()
}

/// Returns the canonical opening `procedure_id` for the shell leaf.
#[cfg(feature = "endpoint")]
pub fn shell_open_procedure() -> String {
    endpoint::ProcedureOpen::protocol_procedure_id()
}

/// Encodes the empty open-request payload used by the shell example.
#[cfg(all(not(feature = "endpoint"), feature = "tui"))]
pub fn shell_leaf_name() -> String {
    RemoteShellTui::protocol_leaf_name()
}

/// Returns the canonical opening `procedure_id` for the shell leaf.
#[cfg(all(not(feature = "endpoint"), feature = "tui"))]
pub fn shell_open_procedure() -> String {
    let mut procedure_id = shell_leaf_name();
    procedure_id.push_str(".open");
    procedure_id
}

/// Encodes the empty open-request payload used by the shell example.
#[cfg(not(any(feature = "endpoint", feature = "tui")))]
pub fn shell_leaf_name() -> String {
    String::from("remote_shell")
}

/// Returns the canonical opening `procedure_id` for the shell leaf.
#[cfg(not(any(feature = "endpoint", feature = "tui")))]
pub fn shell_open_procedure() -> String {
    let mut procedure_id = shell_leaf_name();
    procedure_id.push_str(".open");
    procedure_id
}

/// Encodes the empty open-request payload used by the shell example.
pub fn shell_open_payload() -> Vec<u8> {
    encode_call_reply(&OpenRequest).expect("remote shell open payload should encode")
}

fn path(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}
