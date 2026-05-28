use crate::{Endpoint, Leaf, Packet};

use alloc::{vec, vec::Vec};
use crossbeam_channel::{Receiver, Sender};

pub(super) const ENDPOINT_A: u32 = 0;
pub(super) const ENDPOINT_B: u32 = 1;
pub(super) const ENDPOINT_C: u32 = 2;

const LEAF_CONTROLLER: u32 = 100;
const LEAF_COMMS: u32 = 101;
const LEAF_RESPONDER: u32 = 102;

/// Builds a test packet whose route is the only field varied by routing tests.
///
/// Keeping the payload stable makes each assertion about endpoint behavior rather
/// than packet construction, which is important because forged and malformed cases
/// should fail before any leaf-level procedure handling would matter.
pub(super) fn echo_packet(path: Vec<u32>, hook_id: u16) -> Packet {
    echo_packet_with_end(path, hook_id, false)
}

/// Builds a test packet with an explicit hook-lifetime marker.
pub(super) fn echo_packet_with_end(path: Vec<u32>, hook_id: u16, end_hook: bool) -> Packet {
    Packet {
        hook_id,
        end_hook,
        path,
        procedure_id: 1,
        data: "ABC123".as_bytes().to_vec(),
    }
}

/// Creates a bare endpoint at a known absolute path.
///
/// Most routing tests do not need leaves; they only need the endpoint's local path,
/// connection table, and hook table. This helper keeps that setup explicit without
/// hiding the routing state that each test is validating.
pub(super) fn endpoint_at(id: u32, path: Vec<u32>) -> Endpoint {
    let mut endpoint = Endpoint::new(id, vec![]);
    endpoint.path = path;
    endpoint
}

/// Returns the only outbound packet queued for `next_hop`.
///
/// Routing bugs often show up as packets being sent to the final destination rather
/// than the immediate neighbor. Tests use this helper to assert both that exactly one
/// packet exists and that it was queued for the expected adjacent endpoint.
pub(super) fn single_outbound_packet(endpoint: &Endpoint, next_hop: u32) -> &Packet {
    let queue = endpoint
        .outbound
        .get(&next_hop)
        .unwrap_or_else(|| panic!("expected one outbound queue for {next_hop}"));
    assert_eq!(queue.len(), 1, "expected exactly one outbound packet");
    queue.front().unwrap()
}

/// Returns the only inbound packet delivered to `local_id`.
///
/// Local delivery is intentionally separate from transit forwarding, so the tests
/// assert against the local inbound queue instead of only checking that routing did
/// not produce an error.
pub(super) fn single_inbound_packet(endpoint: &Endpoint, local_id: u32) -> &Packet {
    let queue = endpoint
        .inbound
        .get(&local_id)
        .unwrap_or_else(|| panic!("expected one inbound queue for {local_id}"));
    assert_eq!(queue.len(), 1, "expected exactly one inbound packet");
    queue.front().unwrap()
}

/// Asserts that local hook state still contains `hook_id`.
///
/// Tests use this instead of open-coded map checks so every lifecycle assertion
/// explains the intended routing invariant when it fails.
pub(super) fn assert_hook_present(endpoint: &Endpoint, hook_id: u16) {
    assert!(
        endpoint.has_hook(hook_id),
        "expected hook {hook_id} to remain registered"
    );
}

/// Asserts that local hook state no longer contains `hook_id`.
///
/// Upward `end_hook` packets are the only cases that should remove hook state;
/// downward and local packets with the same flag must leave hooks alone.
pub(super) fn assert_hook_removed(endpoint: &Endpoint, hook_id: u16) {
    assert!(
        !endpoint.has_hook(hook_id),
        "expected hook {hook_id} to be cleaned up"
    );
}

pub(super) struct ControllerLeaf {
    pub(super) has_run: bool,
}

pub(super) struct CommsLeaf {
    pub(super) tx: Sender<Vec<u8>>,
    pub(super) rx: Receiver<Vec<u8>>,

    pub(super) remote_id: u32,
    pub(super) is_authority: bool,
    pub(super) started: bool,
}

pub(super) struct ResponderLeaf;

impl Leaf for ControllerLeaf {
    fn get_id(&self) -> u32 {
        LEAF_CONTROLLER
    }

    fn update(&mut self, endpoint: &mut Endpoint) {
        if !self.has_run {
            // The controller starts exactly one request so the end-to-end test can
            // assert deterministic routing without accumulating retries.
            let hook_id = endpoint.get_hook_id();
            let packet = echo_packet(vec![ENDPOINT_A, ENDPOINT_B], hook_id);
            let _ = endpoint.add_outbound(packet);
            self.has_run = true;
        }
    }
}

impl Leaf for CommsLeaf {
    fn get_id(&self) -> u32 {
        LEAF_COMMS
    }

    fn update(&mut self, endpoint: &mut Endpoint) {
        if !self.started {
            endpoint
                .connections
                .insert((self.remote_id, self.is_authority));
            self.started = true;
        }

        while !self.rx.is_empty() {
            let data = self.rx.recv().unwrap();

            // Transport bytes are untrusted. Dropping malformed frames here keeps
            // the oneshot harness faithful to a router boundary: invalid wire data
            // must not panic or poison later valid packets on the same connection.
            if let Ok(packet) = Packet::deserialize(&data) {
                let _ = endpoint.add_inbound_from(self.remote_id, packet);
            }
        }

        endpoint.take_outbound_clear(self.remote_id, |packet| {
            let data = packet.serialize().unwrap();
            let _ = self.tx.send(data);
        });
    }
}

impl Leaf for ResponderLeaf {
    fn get_id(&self) -> u32 {
        LEAF_RESPONDER
    }

    fn update(&mut self, endpoint: &mut Endpoint) {
        let local_id = endpoint.path.last().cloned().unwrap_or(0);
        let mut packets = Vec::new();

        endpoint.take_inbound_clear(local_id, |packet| {
            let mut response = echo_packet_with_end(vec![ENDPOINT_A], packet.hook_id, true);
            response.hook_id = packet.hook_id;
            response.data = packet.data.clone();
            packets.push(response);
        });

        for packet in packets {
            let _ = endpoint.add_outbound(packet);
        }
    }
}
