use alloc::vec::Vec;

use crate::protocol::{Endpoint, Packet};

pub(crate) const ENDPOINT_A: u32 = 0;
pub(crate) const ENDPOINT_B: u32 = 1;
pub(crate) const ENDPOINT_C: u32 = 2;

/// Creates a bare endpoint at a known absolute path.
///
/// Most routing tests do not need leaves; they only need the endpoint's local path,
/// connection table, and hook table. This helper keeps that setup explicit without
/// hiding the routing state that each test is validating.
pub(crate) fn endpoint_at(id: u32, path: Vec<u32>) -> Endpoint {
    let mut endpoint = Endpoint::new(id);
    endpoint.path = path;
    endpoint
}

/// Returns the only outbound packet queued for `next_hop`.
///
/// Routing bugs often show up as packets being sent to the final destination rather
/// than the immediate neighbor. Tests use this helper to assert both that exactly one
/// packet exists and that it was queued for the expected adjacent endpoint.
pub(crate) fn single_outbound_packet(endpoint: &Endpoint, next_hop: u32) -> &Packet {
    let queue = Endpoint::route_get(next_hop, &endpoint.outbound)
        .unwrap_or_else(|| panic!("expected one outbound queue for {next_hop}"));
    assert_eq!(queue.len(), 1, "expected exactly one outbound packet");
    queue.first().unwrap()
}

/// Returns the only inbound packet delivered to `local_id`.
///
/// Local delivery is intentionally separate from transit forwarding, so the tests
/// assert against the local inbound queue instead of only checking that routing did
/// not produce an error.
pub(crate) fn single_inbound_packet(endpoint: &Endpoint, local_id: u32) -> &Packet {
    let queue = Endpoint::route_get(local_id, &endpoint.inbound)
        .unwrap_or_else(|| panic!("expected one inbound queue for {local_id}"));
    assert_eq!(queue.len(), 1, "expected exactly one inbound packet");
    queue.first().unwrap()
}
