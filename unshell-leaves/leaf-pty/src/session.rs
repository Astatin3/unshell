use alloc::vec::Vec;

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

    fn serialize_interface_state(&self, out: &mut Vec<u8>) -> bool {
        out.extend_from_slice(&self.hook_id.to_be_bytes());
        out.push(u8::from(self.opened_pending));
        out.push(u8::from(self.stdin_closed));
        true
    }

    fn deserialize_interface_state(bytes: &[u8]) -> Option<Self> {
        let [hook_high, hook_low, opened_pending, stdin_closed] = *bytes else {
            return None;
        };

        // Booleans are kept strict so corrupted or future-version blobs are ignored
        // instead of being displayed as trustworthy audit history.
        let opened_pending = decode_bool(opened_pending)?;
        let stdin_closed = decode_bool(stdin_closed)?;

        Some(Self {
            hook_id: HookID::from_be_bytes([hook_high, hook_low]),
            opened_pending,
            stdin_closed,
        })
    }

    #[cfg(feature = "interface_ratatui")]
    fn render_interface_ratatui(
        _: &FakePtyState,
        session: &Self,
        _: &mut unshell::interface::InterfaceContext<'_>,
        frame: &mut unshell::protocol::ratatui::Frame<'_>,
        area: unshell::protocol::ratatui::layout::Rect,
    ) {
        use unshell::protocol::ratatui::{
            style::{Color, Style},
            text::{Line, Span},
            widgets::{Block, Borders, Paragraph},
        };

        use alloc::{string::ToString, vec};

        let status = if session.stdin_closed {
            "stdin closed"
        } else if session.opened_pending {
            "opening"
        } else {
            "active"
        };

        let widget = Paragraph::new(vec![Line::from(vec![
            Span::styled("hook ", Style::new().fg(Color::Gray)),
            Span::raw(session.hook_id.to_string()),
            Span::styled("  status ", Style::new().fg(Color::Gray)),
            Span::raw(status),
        ])])
        .block(Block::default().borders(Borders::ALL).title("PTY session"));

        frame.render_widget(widget, area);
    }
}

/// Decrements the active-session counter exactly once for a terminal session path.
fn close_session(leaf: &mut FakePtyState) {
    leaf.active_count = leaf.active_count.saturating_sub(1);
}

/// Decodes a strict boolean byte from a serialized PTY interface blob.
fn decode_bool(byte: u8) -> Option<bool> {
    match byte {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;

    #[test]
    fn interface_state_round_trips_compact_session_blob() {
        let session = PtySessionState {
            hook_id: 0x1234,
            opened_pending: false,
            stdin_closed: true,
        };
        let mut bytes = Vec::new();

        assert!(session.serialize_interface_state(&mut bytes));
        assert_eq!(bytes, vec![0x12, 0x34, 0, 1]);

        let decoded = PtySessionState::deserialize_interface_state(&bytes)
            .expect("serialized PTY session should decode");

        assert_eq!(decoded.hook_id, session.hook_id);
        assert_eq!(decoded.opened_pending, session.opened_pending);
        assert_eq!(decoded.stdin_closed, session.stdin_closed);
    }

    #[test]
    fn interface_state_rejects_unknown_boolean_values() {
        assert!(PtySessionState::deserialize_interface_state(&[0, 1, 2, 0]).is_none());
        assert!(PtySessionState::deserialize_interface_state(&[0, 1, 0, 2]).is_none());
    }
}
