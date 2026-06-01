use alloc::vec::Vec;

use crate::protocol::{Endpoint, Packet, Procedure, ProcedureOut};

use super::LeafOutbox;

/// Dispatches one packet into a generated one-shot procedure.
pub fn dispatch_procedure<L, P>(
    leaf: &mut L,
    endpoint: &mut Endpoint,
    packet: Packet,
    outbox: &mut LeafOutbox,
) where
    P: Procedure<L>,
{
    let hook_id = packet.hook_id;
    let mut procedure_out =
        ProcedureOut::new(hook_id, parent_reply_path(endpoint), P::PROCEDURE_ID);

    P::handle(leaf, endpoint, packet, &mut procedure_out);

    let packets = procedure_out.into_packets();
    outbox.extend(packets);
}

/// Flushes a generated leaf-level outbox through endpoint routing.
pub fn flush_leaf_outbox(endpoint: &mut Endpoint, outbox: &mut LeafOutbox) -> bool {
    while let Some(entry) = outbox.packets.front() {
        if endpoint.add_outbound(entry.packet.clone()).is_err() {
            return false;
        }

        outbox.packets.pop_front();
    }

    true
}

/// Returns the path used by generated procedure responses.
pub(super) fn parent_reply_path(endpoint: &Endpoint) -> Vec<u32> {
    if endpoint.path.len() > 1 {
        endpoint.path[..endpoint.path.len() - 1].to_vec()
    } else {
        endpoint.path.clone()
    }
}
