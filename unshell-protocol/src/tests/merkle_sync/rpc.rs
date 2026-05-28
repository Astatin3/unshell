use alloc::{vec, vec::Vec};

use crate::Packet;

use super::{
    codec::{encode_block_chunk, encode_child_summary, encode_u32},
    constants::{
        ENDPOINT_CALLER, ENDPOINT_RESPONDENT, PROC_BLOCK_CHUNK, PROC_CHILD_HASH_ENTRY,
        PROC_GET_BLOCK_STREAM, PROC_GET_CHILD_HASHES, PROC_GET_ROOT_HASH, PROC_ROOT_HASH,
    },
    tree::{BlockChunk, ChildSummary},
};

/// One outbound response frame before it is wrapped in endpoint routing fields.
///
/// A response stream owns a list of these frames and asks each frame to become a
/// packet only when the loop is ready to send it. That keeps retry behavior simple:
/// a failed send does not consume the frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OutgoingFrame {
    procedure_id: u32,
    data: Vec<u8>,
}

impl OutgoingFrame {
    /// Wraps the frame in an upward packet for `hook_id`.
    pub(super) fn to_packet(&self, hook_id: u16, end_hook: bool) -> Packet {
        Packet {
            hook_id,
            end_hook,
            path: vec![ENDPOINT_CALLER],
            procedure_id: self.procedure_id,
            data: self.data.clone(),
        }
    }
}

/// Builds the initial root-hash request.
pub(super) fn root_hash_request(hook_id: u16) -> Packet {
    request_packet(PROC_GET_ROOT_HASH, hook_id, Vec::new())
}

/// Builds a request for one branch node's child hashes.
pub(super) fn child_hashes_request(hook_id: u16, node_id: u32) -> Packet {
    request_packet(PROC_GET_CHILD_HASHES, hook_id, encode_u32(node_id))
}

/// Builds a request for one mismatched block's data stream.
pub(super) fn block_stream_request(hook_id: u16, block_id: u32) -> Packet {
    request_packet(PROC_GET_BLOCK_STREAM, hook_id, encode_u32(block_id))
}

/// Builds a single root-hash response frame.
pub(super) fn root_hash_frame(root_hash: u32) -> OutgoingFrame {
    OutgoingFrame {
        procedure_id: PROC_ROOT_HASH,
        data: encode_u32(root_hash),
    }
}

/// Builds one streamed child hash entry response frame.
pub(super) fn child_hash_frame(summary: ChildSummary) -> OutgoingFrame {
    OutgoingFrame {
        procedure_id: PROC_CHILD_HASH_ENTRY,
        data: encode_child_summary(summary),
    }
}

/// Builds one streamed block chunk response frame.
pub(super) fn block_chunk_frame(chunk: BlockChunk) -> OutgoingFrame {
    OutgoingFrame {
        procedure_id: PROC_BLOCK_CHUNK,
        data: encode_block_chunk(&chunk),
    }
}

/// Builds a downward request packet.
fn request_packet(procedure_id: u32, hook_id: u16, data: Vec<u8>) -> Packet {
    Packet {
        hook_id,
        end_hook: true,
        path: vec![ENDPOINT_CALLER, ENDPOINT_RESPONDENT],
        procedure_id,
        data,
    }
}
