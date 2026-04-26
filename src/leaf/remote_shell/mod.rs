//! Stateful remote shell leaf used by the protocol examples.
//!
//! # Design
//!
//! The leaf owns all live hook sessions explicitly in `sessions`. Each entry in
//! that map is one `ProcedureOpen`, keyed by the caller-owned hook identity.
//! The protocol runtime still owns packet validation and transport close state,
//! while the procedure session owns application resources such as the spawned
//! shell process.
//!
//! This keeps the storage obvious:
//! - the leaf owns session maps
//! - the procedure type owns one hook conversation
//! - the runtime routes later `Data` and `Fault` packets automatically

mod errors;
mod session;
mod transport;

use std::collections::BTreeMap;

use unshell::Leaf;
use unshell::protocol::tree::{
    Call, HookKey, Procedure, ProcedureEffect, ProcedureRuntime, ProcedureStore, ProtocolEndpoint,
};

pub use errors::ShellLeafError;
pub use session::ProcedureOpen;
pub use transport::LISTEN_ADDR;

/// Leaf state for the remote shell example.
///
/// The map is explicit on purpose. Stateful procedures are easier to debug when
/// the leaf clearly owns its live sessions instead of relying on generated hidden
/// enums or side tables.
#[derive(Default, Leaf)]
#[leaf(leaf_name = "remote_shell")]
pub struct RemoteShellLeaf {
    sessions: BTreeMap<HookKey, ProcedureOpen>,
}

impl ProcedureStore<ProcedureOpen> for RemoteShellLeaf {
    fn procedure_sessions(&mut self) -> &mut BTreeMap<HookKey, ProcedureOpen> {
        &mut self.sessions
    }
}

impl Procedure<RemoteShellLeaf> for ProcedureOpen {
    type Error = ShellLeafError;
    type Input = ();

    fn open(_leaf: &mut RemoteShellLeaf, call: Call<Self::Input>) -> Result<Self, Self::Error> {
        let hook_key = call.response_hook.ok_or(ShellLeafError::MissingHook)?;
        ProcedureOpen::spawn(hook_key.return_path, hook_key.hook_id, call.procedure_id)
    }

    fn on_data(
        _leaf: &mut RemoteShellLeaf,
        session: &mut Self,
        data: unshell::protocol::tree::IncomingData,
    ) -> Result<ProcedureEffect, Self::Error> {
        session.on_data(data)
    }

    fn on_fault(
        _leaf: &mut RemoteShellLeaf,
        _session: &mut Self,
        _fault: unshell::protocol::tree::IncomingFault,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn poll(
        _leaf: &mut RemoteShellLeaf,
        session: &mut Self,
    ) -> Result<ProcedureEffect, Self::Error> {
        session.poll()
    }

    fn close(_leaf: &mut RemoteShellLeaf, mut session: Self) -> Result<(), Self::Error> {
        session.terminate()
    }
}

/// Returns the example endpoint path used by both shell binaries.
pub fn agent_path() -> Vec<String> {
    path(&["agent"])
}

/// Builds the controller endpoint used by the receiver example.
#[allow(dead_code)]
pub fn build_controller_endpoint() -> ProtocolEndpoint {
    ProtocolEndpoint::new(
        Vec::new(),
        None,
        vec![unshell::protocol::tree::ChildRoute::registered(agent_path())],
        Vec::new(),
    )
}

/// Builds the stateful shell runtime used by the endpoint example.
#[allow(dead_code)]
pub fn build_agent_runtime() -> ProcedureRuntime<RemoteShellLeaf, ProcedureOpen> {
    let endpoint = ProtocolEndpoint::new(
        agent_path(),
        Some(Vec::new()),
        Vec::new(),
        vec![unshell::protocol::tree::LeafSpec {
            name: RemoteShellLeaf::protocol_leaf_name(),
            procedures: vec![ProcedureOpen::protocol_procedure_id()],
        }],
    );
    ProcedureRuntime::new(endpoint, RemoteShellLeaf::default())
}

/// Returns the canonical leaf id used by the receiver example.
#[allow(dead_code)]
pub fn shell_leaf_name() -> String {
    RemoteShellLeaf::protocol_leaf_name()
}

/// Returns the opening `procedure_id` used to create one shell session.
#[allow(dead_code)]
pub fn shell_open_procedure() -> String {
    ProcedureOpen::protocol_procedure_id()
}

/// Encodes the empty opening payload used by the shell example.
#[allow(dead_code)]
pub fn shell_open_payload() -> Vec<u8> {
    unshell::protocol::tree::encode_call_reply(&()).expect("unit shell open payload should encode")
}

#[allow(dead_code)]
pub fn send_forward(
    stream: &mut std::net::TcpStream,
    outcome: unshell::protocol::tree::EndpointOutcome,
) -> std::io::Result<()> {
    transport::send_forward(stream, outcome)
}

#[allow(dead_code)]
pub fn write_frames(
    stream: &mut std::net::TcpStream,
    frames: &[unshell::protocol::FrameBytes],
) -> std::io::Result<()> {
    transport::write_frames(stream, frames)
}

#[allow(dead_code)]
pub fn spawn_frame_reader(
    stream: std::net::TcpStream,
) -> std::sync::mpsc::Receiver<std::io::Result<unshell::protocol::FrameBytes>> {
    transport::spawn_frame_reader(stream)
}

fn path(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}
