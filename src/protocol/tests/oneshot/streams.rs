use crate::protocol::{Endpoint, Leaf, Packet};

#[cfg(feature = "interface")]
use crate::protocol::LeafMeta;

use alloc::{format, vec, vec::Vec};

use super::support::{CommsLeaf, ENDPOINT_A, ENDPOINT_B, assert_hook_present, assert_hook_removed};

const LEAF_STREAM_CALLER: u32 = 200;
const LEAF_STREAM_RESPONDENT: u32 = 201;

/// Builds the initial downwards packet that opens the stream on the respondent.
///
/// The request keeps `end_hook = false` because it expects a return stream. Downward
/// routing now paves that hook automatically at every endpoint that accepts or
/// forwards the request.
fn stream_open_packet(hook_id: u16) -> Packet {
    Packet {
        hook_id,
        end_hook: false,
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
/// Keeping the caller this small makes the per-loop stream assertions about
/// respondent behavior rather than caller retries. The allocated hook id is read
/// back from endpoint state because the counter may start at a randomized offset.
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

/// Concrete stream test harness that keeps leaves outside endpoint routing state.
///
/// This mirrors firmware-style ownership: the endpoint only routes packets while the
/// caller, respondent, and connection leaves are updated explicitly in the same
/// order the old boxed endpoint dispatcher used.
struct StreamHarness {
    endpoint_a: Endpoint,
    endpoint_b: Endpoint,
    caller_a: StreamCallerLeaf,
    comms_a: CommsLeaf,
    respondent_b: StreamRespondentLeaf,
    comms_b: CommsLeaf,
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

    #[cfg(feature = "interface")]
    fn get_meta(&self) -> LeafMeta {
        LeafMeta {
            name: "Stream Caller Leaf",
            identifier: "dev.unshell.test.stream_caller_leaf",
            version: "v0",
            authors: vec!["ASTATIN3"],
        }
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

    #[cfg(feature = "interface")]
    fn get_meta(&self) -> LeafMeta {
        LeafMeta {
            name: "Stream Respondant Leaf",
            identifier: "dev.unshell.test.stream_respondent_leaf",
            version: "v0",
            authors: vec!["ASTATIN3"],
        }
    }

    fn update(&mut self, endpoint: &mut Endpoint) {
        self.open_stream_from_pending_request(endpoint);
        self.send_next_frame(endpoint);
    }
}

impl StreamRespondentLeaf {
    /// Opens stream state from the first locally delivered request packet.
    ///
    /// Downward request routing has already paved the hook before the packet reaches
    /// this leaf. The leaf only owns stream ordering; endpoint routing owns hook
    /// authorization and cleanup.
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
fn stream_endpoints(total_packets: usize) -> StreamHarness {
    let (tx_a, rx_a) = crossbeam_channel::unbounded();
    let (tx_b, rx_b) = crossbeam_channel::unbounded();

    let mut endpoint_a = Endpoint::new(ENDPOINT_A);
    endpoint_a.path = vec![ENDPOINT_A];

    let mut endpoint_b = Endpoint::new(ENDPOINT_B);
    endpoint_b.path = vec![ENDPOINT_A, ENDPOINT_B];

    // Register routes before the first application packet so leaf order is not a
    // hidden prerequisite for the initial request leaving endpoint A.
    endpoint_a.add_connection(ENDPOINT_B, false);
    endpoint_b.add_connection(ENDPOINT_A, true);

    StreamHarness {
        endpoint_a,
        endpoint_b,
        caller_a: StreamCallerLeaf { has_run: false },
        comms_a: CommsLeaf {
            tx: tx_b,
            rx: rx_a,
            remote_id: ENDPOINT_B,
            is_authority: false,
            started: false,
        },
        respondent_b: StreamRespondentLeaf::new(total_packets),
        comms_b: CommsLeaf {
            tx: tx_a,
            rx: rx_b,
            remote_id: ENDPOINT_A,
            is_authority: true,
            started: false,
        },
    }
}

/// Asserts the requested two-endpoint, four-leaf topology.
fn assert_four_leaf_topology(harness: &StreamHarness) {
    assert_eq!(harness.caller_a.get_id(), LEAF_STREAM_CALLER);
    assert_eq!(harness.comms_a.get_id(), 101);
    assert_eq!(harness.respondent_b.get_id(), LEAF_STREAM_RESPONDENT);
    assert_eq!(harness.comms_b.get_id(), 101);
}

/// Drives the initial request until it is queued locally on endpoint B.
fn deliver_stream_request(harness: &mut StreamHarness) {
    harness.caller_a.update(&mut harness.endpoint_a);
    harness.comms_a.update(&mut harness.endpoint_a);
    harness.respondent_b.update(&mut harness.endpoint_b);
    harness.comms_b.update(&mut harness.endpoint_b);
}

/// Returns the single hook opened by the stream request on both endpoints.
///
/// The production counter intentionally does not promise that the first hook is
/// zero. Stream tests still need to prove that both endpoints agree on one routed
/// return channel, so this helper validates the topology and returns the actual id
/// allocated by `StreamCallerLeaf`.
fn opened_stream_hook_id(endpoint_a: &Endpoint, endpoint_b: &Endpoint) -> u16 {
    assert_eq!(
        endpoint_a.hook_count(),
        1,
        "caller endpoint should have exactly one stream hook"
    );
    assert_eq!(
        endpoint_b.hook_count(),
        1,
        "respondent endpoint should have exactly one stream hook"
    );

    let &(caller_hook, caller_peer) = endpoint_a
        .hooks
        .first()
        .expect("caller endpoint should expose the opened hook");
    let &(respondent_hook, respondent_peer) = endpoint_b
        .hooks
        .first()
        .expect("respondent endpoint should expose the opened hook");

    assert_eq!(
        caller_hook, respondent_hook,
        "stream endpoints should agree on the hook id"
    );
    assert_eq!(
        caller_peer, ENDPOINT_B,
        "caller hook should route stream frames through endpoint B"
    );
    assert_eq!(
        respondent_peer, ENDPOINT_A,
        "respondent hook should route stream frames back through endpoint A"
    );

    caller_hook
}

/// Drives one respondent stream loop and delivers any produced frame to endpoint A.
fn drive_stream_loop(harness: &mut StreamHarness) {
    harness.respondent_b.update(&mut harness.endpoint_b);
    harness.comms_b.update(&mut harness.endpoint_b);
    harness.caller_a.update(&mut harness.endpoint_a);
    harness.comms_a.update(&mut harness.endpoint_a);
}

/// Returns stream packets that endpoint A has received so far.
fn received_stream_packets(endpoint: &Endpoint) -> Vec<&Packet> {
    Endpoint::route_get(ENDPOINT_A, &endpoint.inbound)
        .map(|queue| queue.iter().collect())
        .unwrap_or_default()
}

/// Verifies ordered stream payloads and final-frame markers.
fn assert_received_stream(
    endpoint: &Endpoint,
    expected_count: usize,
    final_seen: bool,
    expected_hook_id: u16,
) {
    let packets = received_stream_packets(endpoint);
    assert_eq!(packets.len(), expected_count);

    for (index, packet) in packets.iter().enumerate() {
        assert_eq!(packet.hook_id, expected_hook_id);
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
    let mut harness = stream_endpoints(total_packets);
    assert_four_leaf_topology(&harness);

    deliver_stream_request(&mut harness);
    let stream_hook_id = opened_stream_hook_id(&harness.endpoint_a, &harness.endpoint_b);

    assert_received_stream(&harness.endpoint_a, 0, false, stream_hook_id);
    assert_hook_present(&harness.endpoint_a, stream_hook_id);
    assert_hook_present(&harness.endpoint_b, stream_hook_id);

    for index in 0..total_packets {
        drive_stream_loop(&mut harness);
        let final_seen = index + 1 == total_packets;

        assert_received_stream(&harness.endpoint_a, index + 1, final_seen, stream_hook_id);

        if final_seen {
            assert_hook_removed(&harness.endpoint_a, stream_hook_id);
            assert_hook_removed(&harness.endpoint_b, stream_hook_id);
        } else {
            assert_hook_present(&harness.endpoint_a, stream_hook_id);
            assert_hook_present(&harness.endpoint_b, stream_hook_id);
        }
    }
}

#[test]
fn stream_does_not_emit_before_request_is_processed_by_respondent() {
    let mut harness = stream_endpoints(2);

    deliver_stream_request(&mut harness);
    let stream_hook_id = opened_stream_hook_id(&harness.endpoint_a, &harness.endpoint_b);

    assert_received_stream(&harness.endpoint_a, 0, false, stream_hook_id);
    assert!(Endpoint::routes_is_empty(&harness.endpoint_b.outbound));
    assert_hook_present(&harness.endpoint_a, stream_hook_id);
    assert_hook_present(&harness.endpoint_b, stream_hook_id);
}

#[test]
fn stream_stops_after_final_packet() {
    let total_packets = 2;
    let mut harness = stream_endpoints(total_packets);

    deliver_stream_request(&mut harness);
    let stream_hook_id = opened_stream_hook_id(&harness.endpoint_a, &harness.endpoint_b);
    drive_stream_loop(&mut harness);
    drive_stream_loop(&mut harness);
    assert_received_stream(&harness.endpoint_a, total_packets, true, stream_hook_id);
    assert_hook_removed(&harness.endpoint_b, stream_hook_id);

    drive_stream_loop(&mut harness);
    assert_received_stream(&harness.endpoint_a, total_packets, true, stream_hook_id);
    assert_hook_removed(&harness.endpoint_b, stream_hook_id);
}

#[test]
fn failed_final_stream_route_keeps_hook_and_retries() {
    let mut harness = stream_endpoints(1);

    deliver_stream_request(&mut harness);
    let stream_hook_id = opened_stream_hook_id(&harness.endpoint_a, &harness.endpoint_b);
    harness.endpoint_b.remove_connection(ENDPOINT_A, true);

    drive_stream_loop(&mut harness);
    assert_received_stream(&harness.endpoint_a, 0, false, stream_hook_id);
    assert_hook_present(&harness.endpoint_b, stream_hook_id);

    harness.endpoint_b.add_connection(ENDPOINT_A, true);
    drive_stream_loop(&mut harness);

    assert_received_stream(&harness.endpoint_a, 1, true, stream_hook_id);
    assert_hook_removed(&harness.endpoint_b, stream_hook_id);
}
