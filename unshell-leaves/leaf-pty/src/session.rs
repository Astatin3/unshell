use unshell::protocol::{
    Endpoint, HookID, Packet, PacketQueue, Session, SessionInitError, SessionStatus,
};

use crate::{
    codec::{encode_frame, frame_opcode, frame_payload},
    constants::{
        OP_ABORT, OP_ERROR, OP_EXIT, OP_INPUT, OP_OPEN, OP_OPENED, OP_OUTPUT, OP_STDIN_EOF,
        OP_TERMINATE, PROC_PTY,
    },
    state::FakePtyState,
};

/// Per-hook fake PTY session state.
///
/// A real PTY leaf will replace the pending flags with a worker handle. Hook routing
/// is owned by the generated runtime, so this state only tracks PTY behavior.
pub struct PtySessionState {
    hook_id: HookID,
    opened_pending: bool,
    stdin_closed: bool,
}

impl Session<FakePtyState> for PtySessionState {
    const PROCEDURE_ID: u32 = PROC_PTY;

    fn init(leaf: &mut FakePtyState, packet: Packet) -> Result<Self, SessionInitError> {
        if frame_opcode(&packet) != Some(OP_OPEN) {
            return Err(SessionInitError::response_final(encode_frame(
                OP_ERROR,
                b"unknown-session",
            )));
        }

        leaf.active_count += 1;
        leaf.total_opened += 1;

        Ok(Self {
            hook_id: packet.hook_id,
            opened_pending: true,
            stdin_closed: false,
        })
    }

    fn update(
        leaf: &mut FakePtyState,
        session: &mut Self,
        incoming: &mut PacketQueue,
        endpoint: &mut Endpoint,
    ) -> SessionStatus {
        if session.opened_pending {
            let _ = endpoint.send_hook_frame(
                session.hook_id,
                Self::PROCEDURE_ID,
                OP_OPENED,
                &[],
                false,
            );
            session.opened_pending = false;
        }

        while let Some(packet) = incoming.pop_front() {
            match frame_opcode(&packet) {
                Some(OP_INPUT) => {
                    let _ = endpoint.send_hook_frame(
                        session.hook_id,
                        Self::PROCEDURE_ID,
                        OP_OUTPUT,
                        frame_payload(&packet),
                        false,
                    );
                }
                Some(OP_STDIN_EOF) => {
                    session.stdin_closed = true;
                    leaf.last_stdin_eof_hook = Some(session.hook_id);
                }
                Some(OP_TERMINATE) => {
                    let _ = endpoint.send_hook_frame(
                        session.hook_id,
                        Self::PROCEDURE_ID,
                        OP_EXIT,
                        &[0],
                        true,
                    );
                    close_session(leaf);
                    return SessionStatus::Closed;
                }
                Some(OP_ABORT) => {
                    close_session(leaf);
                    return SessionStatus::Closed;
                }
                Some(OP_OPEN) => {
                    let _ = endpoint.send_hook_frame(
                        session.hook_id,
                        Self::PROCEDURE_ID,
                        OP_ERROR,
                        b"duplicate-open",
                        true,
                    );
                    close_session(leaf);
                    return SessionStatus::Closed;
                }
                _ => {
                    let _ = endpoint.send_hook_frame(
                        session.hook_id,
                        Self::PROCEDURE_ID,
                        OP_ERROR,
                        b"unknown-opcode",
                        true,
                    );
                    close_session(leaf);
                    return SessionStatus::Closed;
                }
            }
        }

        SessionStatus::Running
    }
}

/// Decrements the active-session counter exactly once for a terminal session path.
fn close_session(leaf: &mut FakePtyState) {
    leaf.active_count = leaf.active_count.saturating_sub(1);
}
