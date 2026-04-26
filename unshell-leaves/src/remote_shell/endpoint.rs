//! PTY-backed endpoint implementation for the remote shell leaf.

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
pub use transport::{LISTEN_ADDR, send_forward, spawn_frame_reader, write_frames};

use super::OpenRequest;

/// Leaf state for the remote shell endpoint runtime.
///
/// The endpoint keeps each live shell session in an explicit map keyed by the
/// caller-owned hook identity. That makes ownership and cleanup of hook-backed
/// shell processes easy to inspect during debugging.
#[derive(Default, Leaf)]
#[leaf(leaf_name = "remote_shell")]
pub struct RemoteShellEndpoint {
    sessions: BTreeMap<HookKey, ProcedureOpen>,
}

impl ProcedureStore<ProcedureOpen> for RemoteShellEndpoint {
    fn procedure_sessions(&mut self) -> &mut BTreeMap<HookKey, ProcedureOpen> {
        &mut self.sessions
    }
}

impl Procedure<RemoteShellEndpoint> for ProcedureOpen {
    type Error = ShellLeafError;
    type Input = OpenRequest;

    fn open(_leaf: &mut RemoteShellEndpoint, call: Call<Self::Input>) -> Result<Self, Self::Error> {
        let hook_key = call.response_hook.ok_or(ShellLeafError::MissingHook)?;
        ProcedureOpen::spawn(hook_key.return_path, hook_key.hook_id, call.procedure_id)
    }

    fn on_data(
        _leaf: &mut RemoteShellEndpoint,
        session: &mut Self,
        data: unshell::protocol::tree::IncomingData,
    ) -> Result<ProcedureEffect, Self::Error> {
        session.on_data(data)
    }

    fn on_fault(
        _leaf: &mut RemoteShellEndpoint,
        _session: &mut Self,
        _fault: unshell::protocol::tree::IncomingFault,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn poll(
        _leaf: &mut RemoteShellEndpoint,
        session: &mut Self,
    ) -> Result<ProcedureEffect, Self::Error> {
        session.poll()
    }

    fn close(_leaf: &mut RemoteShellEndpoint, mut session: Self) -> Result<(), Self::Error> {
        session.terminate()
    }
}

/// Builds the controller endpoint used by the receiver example.
pub fn build_controller_endpoint() -> ProtocolEndpoint {
    ProtocolEndpoint::new(
        Vec::new(),
        None,
        vec![unshell::protocol::tree::ChildRoute::registered(agent_path())],
        Vec::new(),
    )
}

/// Builds the stateful shell runtime used by the endpoint example.
pub fn build_agent_runtime() -> ProcedureRuntime<RemoteShellEndpoint, ProcedureOpen> {
    let endpoint = ProtocolEndpoint::new(
        agent_path(),
        Some(Vec::new()),
        Vec::new(),
        vec![unshell::protocol::tree::LeafSpec {
            name: RemoteShellEndpoint::protocol_leaf_name(),
            procedures: vec![ProcedureOpen::protocol_procedure_id()],
        }],
    );
    ProcedureRuntime::new(endpoint, RemoteShellEndpoint::default())
}

fn agent_path() -> Vec<String> {
    vec![String::from("agent")]
}
