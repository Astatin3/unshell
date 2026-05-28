use crate::{
    endpoint::{Endpoint, error::EndpointError},
    leaf::Leaf,
    packet::Packet,
};

use alloc::{boxed::Box, string::ToString, vec, vec::Vec};
use crossbeam_channel::{Receiver, Sender};

const ENDPOINT_A: u32 = 0;
const ENDPOINT_B: u32 = 1;

const LEAF_CONTROLLER: u32 = 100;
const LEAF_COMMS: u32 = 101;
const LEAF_RESPONDER: u32 = 102;
// const HOOK_ECHO: u16 = 500;

fn echo_packet(path: Vec<u32>, hook_id: u16) -> Packet {
    Packet {
        hook_id,
        end_hook: false,
        path,
        procedure_id: "echo".to_string(),
        data: "ABC123".as_bytes().to_vec(),
    }
}

struct ControllerLeaf {
    has_run: bool,
}
struct CommsLeaf {
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,

    remote_id: u32,
    is_authority: bool,
    started: bool,
}
struct ResponderLeaf;

impl Leaf for ControllerLeaf {
    fn get_id(&self) -> u32 {
        LEAF_CONTROLLER
    }

    fn update(&mut self, endpoint: &mut Endpoint) {
        if !self.has_run {
            // Get next free available hook id
            let hook_id = endpoint.get_hook_id();

            // Create packet
            let packet = echo_packet(vec![ENDPOINT_A, ENDPOINT_B], hook_id);

            // Add packet to queue
            let _ = endpoint.add_outbound(packet);

            // Don't run again
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
        }

        while !self.rx.is_empty() {
            let packet = Packet::deserialize(&self.rx.recv().unwrap()).unwrap();

            let _ = endpoint.add_inbound(packet);
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
            let mut response = echo_packet(vec![ENDPOINT_A], packet.hook_id);
            response.hook_id = packet.hook_id;
            response.data = packet.data.clone();
            packets.push(response);
        });

        for packet in packets {
            endpoint.hooks.insert(packet.hook_id, 0);
            let _ = endpoint.add_outbound(packet);
        }
    }
}

#[test]
fn test_oneshot() {
    let (tx_a, rx_a) = crossbeam_channel::unbounded();
    let (tx_b, rx_b) = crossbeam_channel::unbounded();

    let mut endpoint_a = crate::endpoint::Endpoint::new(
        ENDPOINT_A,
        vec![
            Box::new(ControllerLeaf { has_run: false }),
            Box::new(CommsLeaf {
                tx: tx_b,
                rx: rx_a,
                remote_id: ENDPOINT_B,
                is_authority: false,
                started: false,
            }),
        ],
    );
    endpoint_a.path = vec![ENDPOINT_A];

    let mut endpoint_b = crate::endpoint::Endpoint::new(
        ENDPOINT_B,
        vec![
            Box::new(ResponderLeaf),
            Box::new(CommsLeaf {
                tx: tx_a,
                rx: rx_b,
                remote_id: ENDPOINT_A,
                is_authority: true,
                started: false,
            }),
        ],
    );
    endpoint_b.path = vec![ENDPOINT_A, ENDPOINT_B];

    // Connections are registered routing state. The comms leaves also insert them
    // during updates, but the first application packet should not depend on leaf order.
    endpoint_a.connections.insert((ENDPOINT_B, false));
    endpoint_b.connections.insert((ENDPOINT_A, true));

    // Cycle 1: A sends request to B
    endpoint_a.update();
    endpoint_b.update();

    // Cycle 2: B receives request and sends response to A
    endpoint_b.update();
    endpoint_a.update();

    // Cycle 3: A's CommsLeaf needs one more update to pull the packet from the channel
    // and put it into the inbound queue.
    endpoint_a.update();

    // Assertions on state
    assert!(
        endpoint_a.inbound.contains_key(&ENDPOINT_A),
        "Endpoint A should have received response"
    );
    assert_eq!(
        endpoint_a.inbound.get(&ENDPOINT_A).unwrap().len(),
        1,
        "Endpoint A should have exactly one packet"
    );
    let response = &endpoint_a
        .inbound
        .get(&ENDPOINT_A)
        .unwrap()
        .front()
        .unwrap();
    assert_eq!(response.data, "ABC123".as_bytes());
    // assert_eq!(response.hook_id, HOOK_ECHO);
}

#[test]
fn upward_outbound_without_hook_is_rejected() {
    let mut endpoint = Endpoint::new(ENDPOINT_B, vec![]);
    endpoint.path = vec![ENDPOINT_A, ENDPOINT_B];
    endpoint.connections.insert((ENDPOINT_A, true));

    let new_hook = endpoint.get_hook_id();

    let error = endpoint
        .add_outbound(echo_packet(vec![ENDPOINT_A], new_hook))
        .unwrap_err();

    assert!(matches!(error, EndpointError::HookNotExist));
    assert!(endpoint.outbound.is_empty());
}

#[test]
fn downward_outbound_without_hook_is_allowed() {
    let mut endpoint = crate::endpoint::Endpoint::new(ENDPOINT_A, vec![]);
    endpoint.path = vec![ENDPOINT_A];
    endpoint.connections.insert((ENDPOINT_B, false));

    let new_hook = endpoint.get_hook_id();

    endpoint
        .add_outbound(echo_packet(vec![ENDPOINT_A, ENDPOINT_B], new_hook))
        .unwrap();

    assert_eq!(endpoint.outbound.get(&ENDPOINT_B).unwrap().len(), 1);
}

#[test]
fn deeper_upward_route_uses_parent_as_next_hop() {
    const ENDPOINT_C: u32 = 2;

    let mut endpoint = crate::endpoint::Endpoint::new(ENDPOINT_C, vec![]);
    let new_hook = endpoint.get_hook_id();

    endpoint.path = vec![ENDPOINT_A, ENDPOINT_B, ENDPOINT_C];
    endpoint.hooks.insert(new_hook, ENDPOINT_A);
    endpoint.connections.insert((ENDPOINT_B, true));

    endpoint
        .add_outbound(echo_packet(vec![ENDPOINT_A], new_hook))
        .unwrap();

    assert!(endpoint.outbound.contains_key(&ENDPOINT_B));
    assert!(!endpoint.outbound.contains_key(&ENDPOINT_A));
}
