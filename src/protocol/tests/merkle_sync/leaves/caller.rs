use alloc::{collections::VecDeque, rc::Rc, vec::Vec};
use core::cell::RefCell;

use crate::protocol::{Endpoint, Leaf, Packet};

#[cfg(feature = "interface")]
use crate::protocol::LeafMeta;

use super::super::{
    codec::{decode_block_chunk, decode_child_summary, decode_u32},
    constants::{
        ENDPOINT_CALLER, LEAF_MERKLE_CALLER, PROC_BLOCK_CHUNK, PROC_CHILD_HASH_ENTRY,
        PROC_GET_BLOCK_STREAM, PROC_GET_CHILD_HASHES, PROC_GET_ROOT_HASH, PROC_ROOT_HASH,
        ROOT_NODE,
    },
    rpc::{block_stream_request, child_hashes_request, root_hash_request},
    state::{CallerPhase, CallerReport},
    tree::{ChildKind, MerkleStore},
};

/// Caller leaf that drives the Merkle synchronization algorithm.
pub(crate) struct MerkleCallerLeaf {
    local: MerkleStore,
    phase: CallerPhase,
    pending_nodes: VecDeque<u32>,
    pending_blocks: VecDeque<u32>,
    report: Rc<RefCell<CallerReport>>,
}

impl MerkleCallerLeaf {
    /// Creates a caller with a local store and externally visible report.
    pub(crate) fn new(local: MerkleStore, report: Rc<RefCell<CallerReport>>) -> Self {
        Self {
            local,
            phase: CallerPhase::NeedRoot,
            pending_nodes: VecDeque::new(),
            pending_blocks: VecDeque::new(),
            report,
        }
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
            authors: alloc::vec!["ASTATIN3"],
        }
    }

    fn update(&mut self, endpoint: &mut Endpoint) {
        self.receive_responses(endpoint);
        self.dispatch_next_request(endpoint);
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
