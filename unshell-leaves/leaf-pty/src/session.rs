use alloc::vec::Vec;

use unshell::protocol::{
    HookID, Packet, PacketQueue, Session, SessionCtx, SessionInit, SessionInitResult, SessionStatus,
};

use crate::{
    codec::{
        decode_open_reply_path, error_packet, frame_opcode, frame_payload,
        reply_path_from_destination,
    },
    constants::{
        OP_ABORT, OP_ERROR, OP_EXIT, OP_INPUT, OP_OPEN, OP_OPENED, OP_OUTPUT, OP_STDIN_EOF,
        OP_TERMINATE, PROC_PTY,
    },
    state::FakePtyState,
};

/// Session contract for one hook-backed fake PTY.
pub struct PtySession;

/// Per-hook fake PTY session state.
///
/// A real PTY leaf will replace the pending flags with a worker handle. The reply path
/// and hook lifecycle behavior should stay the same.
pub struct PtySessionState {
    hook_id: HookID,
    reply_path: Vec<u32>,
    opened_pending: bool,
    stdin_closed: bool,
}

impl Session<FakePtyState> for PtySession {
    const PROCEDURE_ID: u32 = PROC_PTY;

    type State = PtySessionState;

    fn reply_path(session: &Self::State) -> &[u32] {
        &session.reply_path
    }

    fn init(
        leaf: &mut FakePtyState,
        packet: Packet,
        ctx: &mut SessionInit,
    ) -> SessionInitResult<Self::State> {
        if frame_opcode(&packet) != Some(OP_OPEN) {
            return SessionInitResult::RejectedWith(error_packet(
                ctx.hook_id(),
                reply_path_from_destination(ctx.packet_path()),
                b"unknown-session",
            ));
        }

        let reply_path = decode_open_reply_path(frame_payload(&packet))
            .unwrap_or_else(|| reply_path_from_destination(ctx.packet_path()));

        leaf.active_count += 1;
        leaf.total_opened += 1;

        SessionInitResult::Created(PtySessionState {
            hook_id: ctx.hook_id(),
            reply_path,
            opened_pending: true,
            stdin_closed: false,
        })
    }

    fn update(
        leaf: &mut FakePtyState,
        session: &mut Self::State,
        incoming: &mut PacketQueue,
        ctx: &mut SessionCtx<'_>,
    ) -> SessionStatus {
        if session.opened_pending {
            ctx.send(OP_OPENED, &[]);
            session.opened_pending = false;
        }

        while let Some(packet) = incoming.pop_front() {
            match frame_opcode(&packet) {
                Some(OP_INPUT) => ctx.send(OP_OUTPUT, frame_payload(&packet)),
                Some(OP_STDIN_EOF) => {
                    session.stdin_closed = true;
                    leaf.last_stdin_eof_hook = Some(session.hook_id);
                }
                Some(OP_TERMINATE) => {
                    ctx.send_final(OP_EXIT, &[0]);
                    close_session(leaf);
                    return SessionStatus::Closed;
                }
                Some(OP_ABORT) => {
                    close_session(leaf);
                    return SessionStatus::Closed;
                }
                Some(OP_OPEN) => {
                    ctx.send_final(OP_ERROR, b"duplicate-open");
                    close_session(leaf);
                    return SessionStatus::Closed;
                }
                _ => {
                    ctx.send_final(OP_ERROR, b"unknown-opcode");
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
