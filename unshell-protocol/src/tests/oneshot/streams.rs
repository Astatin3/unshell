use crate::{Endpoint, Leaf, Packet};

use alloc::{boxed::Box, format, vec, vec::Vec};

use super::support::{CommsLeaf, ENDPOINT_A, ENDPOINT_B, assert_hook_present, assert_hook_removed};

const LEAF_STREAM_CALLER: u32 = 200;
const LEAF_STREAM_RESPONDENT: u32 = 201;
const STREAM_HOOK_ID: u16 = 0;

/// Builds the initial downwards packet that opens the stream on the respondent.
///
/// The request deliberately carries `end_hook = true` through `echo_packet`-style
/// semantics: downward routing must not treat that flag as local hook cleanup. The
/// respondent turns this into local stream state keyed by the caller's hook id.
fn stream_open_packet(hook_id: u16) -> Packet {
    Packet {
        hook_id,
        end_hook: true,
        path: vec![ENDPOINT_A, ENDPOINT_B],
        procedure_id: 2,
        data: b"open".to_vec(),
    }
}

/// Builds one upward stream frame for a previously opened hook.
///
/// `end_hook` is false for every intermediate frame and true only for the final
/// frame. This is the behavior the routing layer relies on to keep hook state until
/// the stream has actually finished sending upward.
fn stream_frame_packet(hook_id: u16, index: usize, end_hook: bool) -> Packet {
    Packet {
        hook_id,
        end_hook,
        path: vec![ENDPOINT_A],
        procedure_id: 3,
        data: format!("stream-{index}").into_bytes(),
    }
}

/// Caller leaf that opens exactly one stream request.
///
/// The first allocated hook id is deterministic in these tests (`0`) because the
/// endpoint starts with no existing hooks. Keeping the caller this small makes the
/// per-loop stream assertions about respondent behavior rather than caller retries.
struct StreamCallerLeaf {
    has_run: bool,
}

/// Respondent leaf that converts the first request into a one-way stream.
///
/// This mimics a leaf spawning stream state, not a new endpoint: once a request is
/// delivered locally, the leaf records the hook and emits at most one frame on each
/// later `update`. A failed route does not advance the stream, so retry behavior can
/// be tested by restoring the connection on a later loop.
struct StreamRespondentLeaf {
    stream: Option<StreamState>,
    total_packets: usize,
}

/// In-flight stream state owned by the respondent leaf.
///
/// The endpoint routing layer only knows hooks and packets. This leaf-level state is
/// the minimal application-side record needed to emit ordered frames one at a time.
struct StreamState {
    hook_id: u16,
    next_index: usize,
}

impl StreamRespondentLeaf {
    /// Creates a respondent that will emit `total_packets` stream frames.
    fn new(total_packets: usize) -> Self {
        Self {
            stream: None,
            total_packets,
        }
    }
}

impl Leaf for StreamCallerLeaf {
    fn get_id(&self) -> u32 {
        LEAF_STREAM_CALLER
    }

    fn update(&mut self, endpoint: &mut Endpoint) {
        if self.has_run {
            return;
        }

        let hook_id = endpoint.get_hook_id();
        let _ = endpoint.add_outbound(stream_open_packet(hook_id));
        self.has_run = true;
    }
}

impl Leaf for StreamRespondentLeaf {
    fn get_id(&self) -> u32 {
        LEAF_STREAM_RESPONDENT
    }

    fn update(&mut self, endpoint: &mut Endpoint) {
        self.open_stream_from_pending_request(endpoint);
        self.send_next_frame(endpoint);
    }
}

impl StreamRespondentLeaf {
    /// Opens stream state from the first locally delivered request packet.
    ///
    /// The hook is inserted before any upward frame is routed because upward routing
    /// is hook-gated. Additional requests are ignored while a stream is active so a
    /// caller cannot reset ordering mid-stream in this simple one-way harness.
    fn open_stream_from_pending_request(&mut self, endpoint: &mut Endpoint) {
        if self.stream.is_some() {
            return;
        }

        let local_id = endpoint.path.last().cloned().unwrap_or(0);
        let mut opened_hook = None;

        endpoint.take_inbound_clear(local_id, |packet| {
            if opened_hook.is_none() {
                opened_hook = Some(packet.hook_id);
            }
        });

        if let Some(hook_id) = opened_hook {
            endpoint.hooks.insert(hook_id, ENDPOINT_A);
            self.stream = Some(StreamState {
                hook_id,
                next_index: 0,
            });
        }
    }

    /// Emits at most one frame for the active stream.
    ///
    /// The stream only advances after the routing layer accepts the packet. This is
    /// important for final packets: a failed final route must leave hook state and
    /// stream progress intact so the next loop can retry instead of silently losing
    /// the end-of-stream marker.
    fn send_next_frame(&mut self, endpoint: &mut Endpoint) {
        let Some(stream) = self.stream.as_mut() else {
            return;
        };

        if stream.next_index >= self.total_packets {
            self.stream = None;
            return;
        }

        let index = stream.next_index;
        let end_hook = index + 1 == self.total_packets;
        let packet = stream_frame_packet(stream.hook_id, index, end_hook);

        if endpoint.add_outbound(packet).is_ok() {
            stream.next_index += 1;

            if end_hook {
                self.stream = None;
            }
        }
    }
}

/// Two endpoint, four leaf stream harness.
///
/// Each endpoint has exactly one application leaf and one mock connection leaf. The
/// channel leaves are intentionally the same `CommsLeaf` used by the oneshot tests
/// so stream behavior exercises the same serialization and routing boundary.
fn stream_endpoints(total_packets: usize) -> (Endpoint, Endpoint) {
    let (tx_a, rx_a) = crossbeam_channel::unbounded();
    let (tx_b, rx_b) = crossbeam_channel::unbounded();

    let mut endpoint_a = Endpoint::new(
        ENDPOINT_A,
        vec![
            Box::new(StreamCallerLeaf { has_run: false }),
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

    let mut endpoint_b = Endpoint::new(
        ENDPOINT_B,
        vec![
            Box::new(StreamRespondentLeaf::new(total_packets)),
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

    // Register routes before the first application packet so leaf order is not a
    // hidden prerequisite for the initial request leaving endpoint A.
    endpoint_a.connections.insert((ENDPOINT_B, false));
    endpoint_b.connections.insert((ENDPOINT_A, true));

    (endpoint_a, endpoint_b)
}

/// Asserts the requested two-endpoint, four-leaf topology.
fn assert_four_leaf_topology(endpoint_a: &Endpoint, endpoint_b: &Endpoint) {
    assert_eq!(
        endpoint_a.leaves.len(),
        2,
        "caller endpoint should have two leaves"
    );
    assert_eq!(
        endpoint_b.leaves.len(),
        2,
        "respondent endpoint should have two leaves"
    );
}

/// Drives the initial request until it is queued locally on endpoint B.
fn deliver_stream_request(endpoint_a: &mut Endpoint, endpoint_b: &mut Endpoint) {
    endpoint_a.update();
    endpoint_b.update();
}

/// Drives one respondent stream loop and delivers any produced frame to endpoint A.
fn drive_stream_loop(endpoint_a: &mut Endpoint, endpoint_b: &mut Endpoint) {
    endpoint_b.update();
    endpoint_a.update();
}

/// Returns stream packets that endpoint A has received so far.
fn received_stream_packets(endpoint: &Endpoint) -> Vec<&Packet> {
    endpoint
        .inbound
        .get(&ENDPOINT_A)
        .map(|queue| queue.iter().collect())
        .unwrap_or_default()
}

/// Verifies ordered stream payloads and final-frame markers.
fn assert_received_stream(endpoint: &Endpoint, expected_count: usize, final_seen: bool) {
    let packets = received_stream_packets(endpoint);
    assert_eq!(packets.len(), expected_count);

    for (index, packet) in packets.iter().enumerate() {
        assert_eq!(packet.hook_id, STREAM_HOOK_ID);
        assert_eq!(packet.data, format!("stream-{index}").as_bytes());
        assert_eq!(
            packet.end_hook,
            final_seen && index + 1 == expected_count,
            "only the last received packet should close the stream"
        );
    }
}

#[test]
fn one_directional_stream_returns_one_packet_per_loop() {
    let total_packets = 3;
    let (mut endpoint_a, mut endpoint_b) = stream_endpoints(total_packets);
    assert_four_leaf_topology(&endpoint_a, &endpoint_b);

    deliver_stream_request(&mut endpoint_a, &mut endpoint_b);

    assert_received_stream(&endpoint_a, 0, false);
    assert!(endpoint_b.hooks.is_empty());

    for index in 0..total_packets {
        drive_stream_loop(&mut endpoint_a, &mut endpoint_b);
        let final_seen = index + 1 == total_packets;

        assert_received_stream(&endpoint_a, index + 1, final_seen);

        if final_seen {
            assert_hook_removed(&endpoint_b, STREAM_HOOK_ID);
        } else {
            assert_hook_present(&endpoint_b, STREAM_HOOK_ID);
        }
    }
}

#[test]
fn stream_does_not_emit_before_request_is_processed_by_respondent() {
    let (mut endpoint_a, mut endpoint_b) = stream_endpoints(2);

    deliver_stream_request(&mut endpoint_a, &mut endpoint_b);

    assert_received_stream(&endpoint_a, 0, false);
    assert!(endpoint_b.outbound.is_empty());
    assert!(endpoint_b.hooks.is_empty());
}

#[test]
fn stream_stops_after_final_packet() {
    let total_packets = 2;
    let (mut endpoint_a, mut endpoint_b) = stream_endpoints(total_packets);

    deliver_stream_request(&mut endpoint_a, &mut endpoint_b);
    drive_stream_loop(&mut endpoint_a, &mut endpoint_b);
    drive_stream_loop(&mut endpoint_a, &mut endpoint_b);
    assert_received_stream(&endpoint_a, total_packets, true);
    assert_hook_removed(&endpoint_b, STREAM_HOOK_ID);

    drive_stream_loop(&mut endpoint_a, &mut endpoint_b);
    assert_received_stream(&endpoint_a, total_packets, true);
    assert_hook_removed(&endpoint_b, STREAM_HOOK_ID);
}

#[test]
fn failed_final_stream_route_keeps_hook_and_retries() {
    let (mut endpoint_a, mut endpoint_b) = stream_endpoints(1);

    deliver_stream_request(&mut endpoint_a, &mut endpoint_b);
    endpoint_b.connections.remove(&(ENDPOINT_A, true));

    drive_stream_loop(&mut endpoint_a, &mut endpoint_b);
    assert_received_stream(&endpoint_a, 0, false);
    assert_hook_present(&endpoint_b, STREAM_HOOK_ID);

    endpoint_b.connections.insert((ENDPOINT_A, true));
    drive_stream_loop(&mut endpoint_a, &mut endpoint_b);

    assert_received_stream(&endpoint_a, 1, true);
    assert_hook_removed(&endpoint_b, STREAM_HOOK_ID);
}
