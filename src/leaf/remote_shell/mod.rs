//! Stateful remote shell leaf used by the protocol examples.
//!
//! This module intentionally lives outside the core `protocol::tree` runtime.
//! The protocol runtime stays generic, while this leaf layers one concrete
//! application contract on top: one opening `Call`, then one bidirectional hook
//! stream whose lifetime is tied to the spawned shell process.

mod errors;
mod session;
mod transport;

use std::collections::BTreeMap;
use std::io::Write;

use unshell::protocol::tree::{
    Call, CallLeaf, HookKey, IncomingData, IncomingFault, LeafRuntime, OutgoingData,
    ProtocolEndpoint,
};
use unshell::{Leaf, procedures};

pub use errors::ShellLeafError;
use session::{ShellSession, close_session};
pub use transport::LISTEN_ADDR;

#[derive(Default, Leaf)]
#[leaf(org = "org", product = "example", version = "v1", leaf_name = "shell")]
pub struct RemoteShellLeaf {
    sessions: BTreeMap<HookKey, ShellSession>,
}

#[procedures(error = ShellLeafError)]
impl RemoteShellLeaf {
    #[call]
    fn open(&mut self, call: Call<()>) -> Result<(), ShellLeafError> {
        let hook_key = call.response_hook.ok_or(ShellLeafError::MissingHook)?;
        let session = ShellSession::spawn(
            hook_key.return_path.clone(),
            hook_key.hook_id,
            call.procedure_id,
        )?;

        if let Some(mut previous) = self.sessions.insert(hook_key, session) {
            previous.terminate()?;
        }
        Ok(())
    }
}

impl CallLeaf for RemoteShellLeaf {
    type Error = ShellLeafError;

    fn on_data(&mut self, data: IncomingData) -> Result<Vec<OutgoingData>, Self::Error> {
        let Some(session) = self.sessions.get_mut(&data.hook_key) else {
            return Ok(Vec::new());
        };

        if !data.message.data.is_empty() {
            let Some(stdin) = session.stdin.as_mut() else {
                return Ok(Vec::new());
            };
            stdin.write_all(&data.message.data)?;
            stdin.flush()?;
        }

        if !data.message.end_hook {
            return Ok(Vec::new());
        }

        let session = self
            .sessions
            .remove(&data.hook_key)
            .ok_or(ShellLeafError::MissingSession)?;
        close_session(session)
    }

    fn on_fault(&mut self, fault: IncomingFault) -> Result<(), Self::Error> {
        if let Some(mut session) = self.sessions.remove(&fault.hook_key) {
            session.terminate()?;
        }
        Ok(())
    }

    fn poll(&mut self) -> Result<Vec<OutgoingData>, Self::Error> {
        let mut outgoing = Vec::new();
        let mut closed = Vec::new();

        for key in self.sessions.keys().cloned().collect::<Vec<_>>() {
            let Some(session) = self.sessions.get_mut(&key) else {
                continue;
            };

            session.drain_output(&mut outgoing);

            if session.local_end_sent {
                continue;
            }

            if session.exit_status.is_none() {
                session.exit_status = session.child.try_wait()?;
            }

            if session.exit_status.is_some() && session.readers_closed >= 2 {
                outgoing.push(session.packet(Vec::new(), true));
                session.local_end_sent = true;
                closed.push(key);
            }
        }

        for key in closed {
            self.sessions.remove(&key);
        }

        Ok(outgoing)
    }
}

pub fn agent_path() -> Vec<String> {
    path(&["agent"])
}

#[allow(dead_code)]
pub fn build_controller_endpoint() -> ProtocolEndpoint {
    ProtocolEndpoint::new(
        Vec::new(),
        None,
        vec![unshell::protocol::tree::ChildRoute::registered(agent_path())],
        Vec::new(),
    )
}

#[allow(dead_code)]
pub fn build_agent_runtime() -> LeafRuntime<RemoteShellLeaf> {
    let endpoint = ProtocolEndpoint::new(
        agent_path(),
        Some(Vec::new()),
        Vec::new(),
        vec![RemoteShellLeaf::protocol_leaf_spec()],
    );
    LeafRuntime::new(endpoint, RemoteShellLeaf::default())
}

#[allow(dead_code)]
pub fn shell_leaf_name() -> String {
    RemoteShellLeaf::protocol_leaf_name()
}

#[allow(dead_code)]
pub fn shell_open_procedure() -> String {
    RemoteShellLeaf::protocol_procedure_id("open")
        .expect("remote shell leaf declares an open procedure")
}

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
