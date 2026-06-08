use unshell::protocol::{Endpoint, Packet, Procedure, ProcedureOut};

use crate::{constants::PROC_PING, state::FakePtyState};

/// One-shot echo procedure used to exercise generated procedure dispatch.
///
/// The fake PTY leaf is primarily session-oriented, so this deliberately small
/// procedure gives tests a non-session packet family. It also gives the future
/// Ratatui interface a stable procedure branch to render if procedure-owned blobs are
/// added later without reintroducing the removed global interface view store.
///
/// TODO(interface-ratatui): add `Procedure::render_interface_ratatui` only after a
/// real operator view needs procedure state in addition to serialized sessions.
pub(crate) struct PingProcedure;

impl Procedure<FakePtyState> for PingProcedure {
    const PROCEDURE_ID: u32 = PROC_PING;

    fn handle(_: &mut FakePtyState, _: &mut Endpoint, packet: Packet, out: &mut ProcedureOut) {
        out.send_final(&packet.data);
    }
}
