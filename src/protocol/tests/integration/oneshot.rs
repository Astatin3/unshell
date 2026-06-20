use alloc::{vec, vec::Vec};

use crate::protocol::{Endpoint, Leaf};

#[cfg(feature = "interface")]
use crate::protocol::LeafMeta;

use super::super::support::{
    endpoints::{ENDPOINT_A, ENDPOINT_B},
    packets::{echo_packet, echo_packet_with_end},
    transport::CommsLeaf,
};

const LEAF_CONTROLLER: u32 = 100;
const LEAF_RESPONDER: u32 = 102;

struct ControllerLeaf {
    has_run: bool,
}

struct ResponderLeaf;

impl Leaf for ControllerLeaf {
    fn get_id(&self) -> u32 {
        LEAF_CONTROLLER
    }

    #[cfg(feature = "interface")]
    fn get_meta(&self) -> LeafMeta {
        LeafMeta {
            name: "Controller Leaf",
            identifier: "dev.unshell.test.controller_leaf",
            version: "v0",
            authors: alloc::vec!["ASTATIN3"],
        }
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

impl Leaf for ResponderLeaf {
    fn get_id(&self) -> u32 {
        LEAF_RESPONDER
    }

    #[cfg(feature = "interface")]
    fn get_meta(&self) -> LeafMeta {
        LeafMeta {
            name: "Responder Leaf",
            identifier: "dev.unshell.test.responder_leaf",
            version: "v0",
            authors: alloc::vec!["ASTATIN3"],
        }
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

#[test]
fn request_response_round_trip_over_mock_transport() {
    let (tx_a, rx_a) = crossbeam_channel::unbounded();
    let (tx_b, rx_b) = crossbeam_channel::unbounded();

    let mut endpoint_a = Endpoint::new(ENDPOINT_A);
    let mut controller_a = ControllerLeaf { has_run: false };
    let mut comms_a = CommsLeaf {
        tx: tx_b,
        rx: rx_a,
        remote_id: ENDPOINT_B,
        is_authority: false,
        started: false,
    };
    endpoint_a.path = vec![ENDPOINT_A];

    let mut endpoint_b = Endpoint::new(ENDPOINT_B);
    let mut responder_b = ResponderLeaf;
    let mut comms_b = CommsLeaf {
        tx: tx_a,
        rx: rx_b,
        remote_id: ENDPOINT_A,
        is_authority: true,
        started: false,
    };
    endpoint_b.path = vec![ENDPOINT_A, ENDPOINT_B];

    // Connections are registered routing state. The comms leaves also insert them
    // during updates, but the first application packet should not depend on leaf order.
    endpoint_a.add_connection(ENDPOINT_B, false);
    endpoint_b.add_connection(ENDPOINT_A, true);

    // Cycle 1: A sends request to B.
    controller_a.update(&mut endpoint_a);
    comms_a.update(&mut endpoint_a);
    responder_b.update(&mut endpoint_b);
    comms_b.update(&mut endpoint_b);

    // Cycle 2: B receives request and sends response to A.
    responder_b.update(&mut endpoint_b);
    comms_b.update(&mut endpoint_b);
    controller_a.update(&mut endpoint_a);
    comms_a.update(&mut endpoint_a);

    // Cycle 3: A's transport leaf needs one more update to pull the response bytes
    // from the channel and put the packet into the inbound queue.
    controller_a.update(&mut endpoint_a);
    comms_a.update(&mut endpoint_a);

    assert!(
        Endpoint::route_contains(ENDPOINT_A, &endpoint_a.inbound),
        "Endpoint A should have received response"
    );
    assert_eq!(
        Endpoint::route_get(ENDPOINT_A, &endpoint_a.inbound)
            .unwrap()
            .len(),
        1,
        "Endpoint A should have exactly one packet"
    );
    let response = &Endpoint::route_get(ENDPOINT_A, &endpoint_a.inbound)
        .unwrap()
        .first()
        .unwrap();
    assert!(response.end_hook);
    assert_eq!(response.data, "ABC123".as_bytes());
    assert_eq!(endpoint_b.hook_count(), 0);
}
