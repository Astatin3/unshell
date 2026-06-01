use alloc::{collections::VecDeque, rc::Rc, vec, vec::Vec};
use core::cell::RefCell;

use crossbeam_channel::{Receiver, Sender};

use crate::protocol::{Endpoint, Leaf, Packet};

#[cfg(feature = "interface")]
use crate::protocol::LeafMeta;

use super::{
    codec::{decode_block_chunk, decode_child_summary, decode_u32},
    constants::{
        ENDPOINT_CALLER, ENDPOINT_RESPONDENT, LEAF_MERKLE_CALLER, LEAF_MERKLE_RESPONDENT,
        LEAF_MOCK_CONNECTION, PROC_BLOCK_CHUNK, PROC_CHILD_HASH_ENTRY, PROC_GET_BLOCK_STREAM,
        PROC_GET_CHILD_HASHES, PROC_GET_ROOT_HASH, PROC_ROOT_HASH, ROOT_NODE,
    },
    rpc::{
        block_chunk_frame, block_stream_request, child_hash_frame, child_hashes_request,
        root_hash_frame, root_hash_request,
    },
    state::{CallerPhase, CallerReport, RespondentReport, ResponseStream},
    tree::{BlockChunk, ChildKind, MerkleStore},
};

/// Leaf that simulates a serialized transport connection with crossbeam channels.
///
/// This is intentionally tiny and reusable. Both endpoints in the Merkle test have
/// exactly one of these leaves, giving the requested four-leaf topology: caller,
/// respondent, and two mock connections.
pub(super) struct MockConnectionLeaf {
    pub(super) tx: Sender<Vec<u8>>,
    pub(super) rx: Receiver<Vec<u8>>,
    pub(super) remote_id: u32,
    pub(super) is_authority: bool,
    pub(super) started: bool,
}

/// Caller leaf that drives the Merkle synchronization algorithm.
pub(super) struct MerkleCallerLeaf {
    local: MerkleStore,
    phase: CallerPhase,
    pending_nodes: VecDeque<u32>,
    pending_blocks: VecDeque<u32>,
    report: Rc<RefCell<CallerReport>>,
}

/// Respondent leaf that serves Merkle hash and block streams.
pub(super) struct MerkleRespondentLeaf {
    remote: MerkleStore,
    active_stream: Option<ResponseStream>,
    report: Rc<RefCell<RespondentReport>>,
}

impl MockConnectionLeaf {
    /// Creates one side of a mock connection.
    pub(super) fn new(
        tx: Sender<Vec<u8>>,
        rx: Receiver<Vec<u8>>,
        remote_id: u32,
        is_authority: bool,
    ) -> Self {
        Self {
            tx,
            rx,
            remote_id,
            is_authority,
            started: false,
        }
    }
}

impl MerkleCallerLeaf {
    /// Creates a caller with a local store and externally visible report.
    pub(super) fn new(local: MerkleStore, report: Rc<RefCell<CallerReport>>) -> Self {
        Self {
            local,
            phase: CallerPhase::NeedRoot,
            pending_nodes: VecDeque::new(),
            pending_blocks: VecDeque::new(),
            report,
        }
    }
}

impl MerkleRespondentLeaf {
    /// Creates a respondent backed by the authoritative remote store.
    pub(super) fn new(remote: MerkleStore, report: Rc<RefCell<RespondentReport>>) -> Self {
        Self {
            remote,
            active_stream: None,
            report,
        }
    }
}

impl Leaf for MockConnectionLeaf {
    fn get_id(&self) -> u32 {
        LEAF_MOCK_CONNECTION
    }

    #[cfg(feature = "interface")]
    fn get_meta(&self) -> LeafMeta {
        LeafMeta {
            name: "Merke Connection Leaf",
            identifier: "dev.unshell.test.merkle.connection",
            version: "v0",
            authors: vec!["ASTATIN3"],
        }
    }

    fn update(&mut self, endpoint: &mut Endpoint) {
        if !self.started {
            endpoint.add_connection(self.remote_id, self.is_authority);
            self.started = true;
        }

        while !self.rx.is_empty() {
            let data = self.rx.recv().unwrap();

            // Mock transports move untrusted bytes. Malformed frames are dropped so
            // the sync state machine is tested only after packet parsing succeeds.
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

impl Leaf for MerkleCallerLeaf {
    fn get_id(&self) -> u32 {
        LEAF_MERKLE_CALLER
    }

    #[cfg(feature = "interface")]
    fn get_meta(&self) -> LeafMeta {
        LeafMeta {
            name: "Merke Caller Leaf",
            identifier: "dev.unshell.test.merkle.caller",
            version: "v0",
            authors: vec!["ASTATIN3"],
        }
    }

    fn update(&mut self, endpoint: &mut Endpoint) {
        self.receive_responses(endpoint);
        self.dispatch_next_request(endpoint);
    }
}

impl Leaf for MerkleRespondentLeaf {
    fn get_id(&self) -> u32 {
        LEAF_MERKLE_RESPONDENT
    }

    #[cfg(feature = "interface")]
    fn get_meta(&self) -> LeafMeta {
        LeafMeta {
            name: "Merke Respondent Leaf",
            identifier: "dev.unshell.test.merkle.respondent",
            version: "v0",
            authors: vec!["ASTATIN3"],
        }
    }

    fn update(&mut self, endpoint: &mut Endpoint) {
        self.open_stream_from_request(endpoint);
        self.send_one_response_frame(endpoint);
    }
}

impl MerkleCallerLeaf {
    /// Consumes all response packets currently delivered to endpoint A.
    fn receive_responses(&mut self, endpoint: &mut Endpoint) {
        endpoint.take_inbound_clear(ENDPOINT_CALLER, |packet| {
            self.report
                .borrow_mut()
                .received_procedures
                .push(packet.procedure_id);
            self.handle_response_packet(packet);
        });
    }

    /// Handles one response packet according to the current caller phase.
    fn handle_response_packet(&mut self, packet: &Packet) {
        match &mut self.phase {
            CallerPhase::AwaitRoot { hook_id } => {
                assert_eq!(packet.hook_id, *hook_id);
                assert_eq!(packet.procedure_id, PROC_ROOT_HASH);
                let remote_root = decode_u32(&packet.data).expect("root hash payload");

                if packet.end_hook {
                    self.finish_root_response(remote_root);
                }
            }
            CallerPhase::AwaitChildren {
                hook_id,
                node_id: _,
                entries,
            } => {
                assert_eq!(packet.hook_id, *hook_id);
                assert_eq!(packet.procedure_id, PROC_CHILD_HASH_ENTRY);
                entries.push(decode_child_summary(&packet.data).expect("child summary payload"));

                if packet.end_hook {
                    self.finish_child_response();
                }
            }
            CallerPhase::AwaitBlock {
                hook_id,
                block_id: _,
                chunks,
            } => {
                assert_eq!(packet.hook_id, *hook_id);
                assert_eq!(packet.procedure_id, PROC_BLOCK_CHUNK);
                chunks.push(decode_block_chunk(&packet.data).expect("block chunk payload"));

                if packet.end_hook {
                    self.finish_block_response();
                }
            }
            CallerPhase::NeedRoot | CallerPhase::Ready | CallerPhase::Done => {
                panic!("unexpected Merkle response in phase {:?}", self.phase);
            }
        }
    }

    /// Applies the completed root response and decides whether tree walking is needed.
    fn finish_root_response(&mut self, remote_root: u32) {
        if self.local.root_hash() == remote_root {
            self.mark_done();
        } else {
            self.pending_nodes.push_back(ROOT_NODE);
            self.phase = CallerPhase::Ready;
        }
    }

    /// Applies a completed child-hash stream.
    fn finish_child_response(&mut self) {
        let CallerPhase::AwaitChildren {
            hook_id: _,
            node_id: _,
            entries,
        } = core::mem::replace(&mut self.phase, CallerPhase::Ready)
        else {
            unreachable!();
        };

        for entry in entries {
            if self.local.hash_for(entry.kind, entry.id) == entry.hash {
                continue;
            }

            match entry.kind {
                ChildKind::Branch => self.pending_nodes.push_back(entry.id),
                ChildKind::Block => self.pending_blocks.push_back(entry.id),
            }
        }
    }

    /// Applies a completed block stream to the local store.
    fn finish_block_response(&mut self) {
        let CallerPhase::AwaitBlock {
            hook_id: _,
            block_id,
            mut chunks,
        } = core::mem::replace(&mut self.phase, CallerPhase::Ready)
        else {
            unreachable!();
        };

        chunks.sort_by_key(|chunk| chunk.index);
        assert_eq!(
            chunks.len(),
            chunks.first().map(|chunk| chunk.total).unwrap_or(0) as usize
        );

        let new_chunks: Vec<Vec<u8>> = chunks.into_iter().map(|chunk| chunk.data).collect();
        self.local.replace_block(block_id, new_chunks.clone());

        let mut report = self.report.borrow_mut();
        report.synchronized_blocks.push(block_id);
        report.applied_block_chunks.push((block_id, new_chunks));
    }

    /// Sends the next request if the caller is not waiting on a response stream.
    fn dispatch_next_request(&mut self, endpoint: &mut Endpoint) {
        match self.phase {
            CallerPhase::NeedRoot => {
                let hook_id = self.send_request(endpoint, PROC_GET_ROOT_HASH, Vec::new());
                endpoint.add_outbound(root_hash_request(hook_id)).unwrap();
                self.phase = CallerPhase::AwaitRoot { hook_id };
            }
            CallerPhase::Ready => {
                if let Some(node_id) = self.pending_nodes.pop_front() {
                    let hook_id = self.send_request(endpoint, PROC_GET_CHILD_HASHES, Vec::new());
                    endpoint
                        .add_outbound(child_hashes_request(hook_id, node_id))
                        .unwrap();
                    self.phase = CallerPhase::AwaitChildren {
                        hook_id,
                        node_id,
                        entries: Vec::new(),
                    };
                } else if let Some(block_id) = self.pending_blocks.pop_front() {
                    let hook_id = self.send_request(endpoint, PROC_GET_BLOCK_STREAM, Vec::new());
                    endpoint
                        .add_outbound(block_stream_request(hook_id, block_id))
                        .unwrap();
                    self.phase = CallerPhase::AwaitBlock {
                        hook_id,
                        block_id,
                        chunks: Vec::new(),
                    };
                } else {
                    self.mark_done();
                }
            }
            CallerPhase::AwaitRoot { .. }
            | CallerPhase::AwaitChildren { .. }
            | CallerPhase::AwaitBlock { .. }
            | CallerPhase::Done => {}
        }
    }

    /// Reserves a hook id and records the logical RPC request.
    fn send_request(&mut self, endpoint: &mut Endpoint, procedure_id: u32, _data: Vec<u8>) -> u16 {
        let hook_id = endpoint.get_hook_id();
        self.report
            .borrow_mut()
            .requested_procedures
            .push(procedure_id);
        hook_id
    }

    /// Marks the synchronization complete and records the final local root.
    fn mark_done(&mut self) {
        self.phase = CallerPhase::Done;
        let mut report = self.report.borrow_mut();
        report.done = true;
        report.final_root_hash = Some(self.local.root_hash());
    }
}

impl MerkleRespondentLeaf {
    /// Opens one response stream from the first pending local request.
    fn open_stream_from_request(&mut self, endpoint: &mut Endpoint) {
        if self.active_stream.is_some() {
            return;
        }

        let mut request = None;
        endpoint.take_inbound_clear(ENDPOINT_RESPONDENT, |packet| {
            if request.is_none() {
                request = Some((packet.hook_id, packet.procedure_id, packet.data.clone()));
            }
        });

        let Some((hook_id, procedure_id, data)) = request else {
            return;
        };

        let frames = self.frames_for_request(procedure_id, &data);

        self.report.borrow_mut().requests_seen.push(procedure_id);
        if !frames.is_empty() {
            self.report.borrow_mut().streams_started += 1;
            self.active_stream = Some(ResponseStream::new(hook_id, frames));
        }
    }

    /// Builds response frames for one request procedure.
    fn frames_for_request(&self, procedure_id: u32, data: &[u8]) -> Vec<super::rpc::OutgoingFrame> {
        match procedure_id {
            PROC_GET_ROOT_HASH => vec![root_hash_frame(self.remote.root_hash())],
            PROC_GET_CHILD_HASHES => {
                let node_id = decode_u32(data).expect("child hash request node id");
                self.remote
                    .child_summaries(node_id)
                    .into_iter()
                    .map(child_hash_frame)
                    .collect()
            }
            PROC_GET_BLOCK_STREAM => {
                let block_id = decode_u32(data).expect("block stream request block id");
                let chunks = self.remote.block_chunks(block_id);
                let total = chunks.len() as u32;
                chunks
                    .into_iter()
                    .enumerate()
                    .map(|(index, data)| {
                        block_chunk_frame(BlockChunk {
                            block_id,
                            index: index as u32,
                            total,
                            data,
                        })
                    })
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    /// Sends at most one response frame per update loop.
    fn send_one_response_frame(&mut self, endpoint: &mut Endpoint) {
        let Some(stream) = self.active_stream.as_mut() else {
            return;
        };

        if stream.is_empty() {
            self.active_stream = None;
            return;
        }

        let packet = stream.next_packet().expect("active stream frame");
        if endpoint.add_outbound(packet).is_err() {
            return;
        }

        self.report.borrow_mut().frames_sent += 1;
        stream.advance();

        if stream.is_complete() {
            self.report.borrow_mut().streams_completed += 1;
            self.active_stream = None;
        }
    }
}
