//! PTY-backed endpoint implementation for the remote shell leaf.

mod errors;
mod session;
mod transport;

use std::collections::BTreeMap;

use unshell::protocol::tree::{Call, HookKey, Procedure, ProcedureEffect, ProcedureStore};

pub use errors::ShellLeafError;
pub use session::Open;
pub use transport::{LISTEN_ADDR, send_forward, spawn_frame_reader, write_frames};

use super::OpenRequest;

/// Leaf state for the remote shell endpoint runtime.
///
/// The endpoint keeps each live shell session in an explicit map keyed by the
/// caller-owned hook identity. That makes ownership and cleanup of hook-backed
/// shell processes easy to inspect during debugging.
#[derive(Default)]
pub struct RemoteShellEndpoint {
    sessions: BTreeMap<HookKey, Open>,
}

impl ProcedureStore<Open> for RemoteShellEndpoint {
    fn procedure_sessions(&mut self) -> &mut BTreeMap<HookKey, Open> {
        &mut self.sessions
    }
}

impl Procedure<RemoteShellEndpoint> for Open {
    type Error = ShellLeafError;
    type Input = OpenRequest;

    fn open(_leaf: &mut RemoteShellEndpoint, call: Call<Self::Input>) -> Result<Self, Self::Error> {
        let hook_key = call.response_hook.ok_or(ShellLeafError::MissingHook)?;
        Open::spawn(hook_key.return_path, hook_key.hook_id, call.procedure_id)
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
