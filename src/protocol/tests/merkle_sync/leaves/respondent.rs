use alloc::{rc::Rc, vec::Vec};
use core::cell::RefCell;

use crate::protocol::{Endpoint, Leaf};

#[cfg(feature = "interface")]
use crate::protocol::LeafMeta;

use super::super::{
    codec::decode_u32,
    constants::{
        ENDPOINT_RESPONDENT, LEAF_MERKLE_RESPONDENT, PROC_GET_BLOCK_STREAM, PROC_GET_CHILD_HASHES,
        PROC_GET_ROOT_HASH,
    },
    rpc::{block_chunk_frame, child_hash_frame, root_hash_frame},
    state::{RespondentReport, ResponseStream},
    tree::{BlockChunk, MerkleStore},
};

/// Respondent leaf that serves Merkle hash and block streams.
pub(crate) struct MerkleRespondentLeaf {
    remote: MerkleStore,
    active_stream: Option<ResponseStream>,
    report: Rc<RefCell<RespondentReport>>,
}

impl MerkleRespondentLeaf {
    /// Creates a respondent backed by the authoritative remote store.
    pub(crate) fn new(remote: MerkleStore, report: Rc<RefCell<RespondentReport>>) -> Self {
        Self {
            remote,
            active_stream: None,
            report,
        }
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
            authors: alloc::vec!["ASTATIN3"],
        }
    }

    fn update(&mut self, endpoint: &mut Endpoint) {
        self.open_stream_from_request(endpoint);
        self.send_one_response_frame(endpoint);
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
    fn frames_for_request(
        &self,
        procedure_id: u32,
        data: &[u8],
    ) -> Vec<super::super::rpc::OutgoingFrame> {
        match procedure_id {
            PROC_GET_ROOT_HASH => alloc::vec![root_hash_frame(self.remote.root_hash())],
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
