use alloc::vec::Vec;

use super::tree::{BlockChunk, ChildKind, ChildSummary};

/// Encodes one `u32` request or response payload.
pub(super) fn encode_u32(value: u32) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

/// Decodes one exact `u32` payload.
pub(super) fn decode_u32(data: &[u8]) -> Option<u32> {
    if data.len() == 4 {
        Some(read_u32(data, 0))
    } else {
        None
    }
}

/// Encodes one streamed child hash entry.
pub(super) fn encode_child_summary(summary: ChildSummary) -> Vec<u8> {
    let mut data = Vec::with_capacity(12);
    data.extend_from_slice(&summary.id.to_le_bytes());
    data.extend_from_slice(&summary.kind.discriminant().to_le_bytes());
    data.extend_from_slice(&summary.hash.to_le_bytes());
    data
}

/// Decodes one streamed child hash entry.
pub(super) fn decode_child_summary(data: &[u8]) -> Option<ChildSummary> {
    if data.len() != 12 {
        return None;
    }

    Some(ChildSummary {
        id: read_u32(data, 0),
        kind: ChildKind::from_discriminant(read_u32(data, 4))?,
        hash: read_u32(data, 8),
    })
}

/// Encodes one streamed block chunk.
pub(super) fn encode_block_chunk(chunk: &BlockChunk) -> Vec<u8> {
    let mut data = Vec::with_capacity(16 + chunk.data.len());
    data.extend_from_slice(&chunk.block_id.to_le_bytes());
    data.extend_from_slice(&chunk.index.to_le_bytes());
    data.extend_from_slice(&chunk.total.to_le_bytes());
    data.extend_from_slice(&(chunk.data.len() as u32).to_le_bytes());
    data.extend_from_slice(&chunk.data);
    data
}

/// Decodes one streamed block chunk.
pub(super) fn decode_block_chunk(data: &[u8]) -> Option<BlockChunk> {
    if data.len() < 16 {
        return None;
    }

    let len = read_u32(data, 12) as usize;
    if data.len() != 16 + len {
        return None;
    }

    Some(BlockChunk {
        block_id: read_u32(data, 0),
        index: read_u32(data, 4),
        total: read_u32(data, 8),
        data: data[16..].to_vec(),
    })
}

/// Reads a little-endian `u32` at a known-valid offset.
fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}
