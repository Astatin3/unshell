use alloc::{collections::BTreeMap, vec, vec::Vec};

use super::constants::{
    BLOCK_ALPHA, BLOCK_BRAVO, BLOCK_CHARLIE, BLOCK_DELTA, BRANCH_LEFT, BRANCH_RIGHT, ROOT_NODE,
};

/// Type of child referenced by a Merkle node summary.
///
/// The sync caller uses this to decide whether a mismatched child should recurse
/// with `GET_CHILD_HASHES` or transfer data with `GET_BLOCK_STREAM`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ChildKind {
    Branch,
    Block,
}

/// One child entry in a streamed Merkle summary response.
///
/// A respondent streams these one per loop. The caller compares each `hash` with
/// its local store and queues either another node walk or a block transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ChildSummary {
    pub(super) id: u32,
    pub(super) kind: ChildKind,
    pub(super) hash: u32,
}

/// One chunk in a streamed block response.
///
/// Chunks carry their total so the caller can replace the local block only after
/// the final stream packet arrives. This keeps partially received data out of the
/// Merkle hash until the hook completes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BlockChunk {
    pub(super) block_id: u32,
    pub(super) index: u32,
    pub(super) total: u32,
    pub(super) data: Vec<u8>,
}

/// Static edge in the test Merkle tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TreeChild {
    id: u32,
    kind: ChildKind,
}

/// In-memory Merkle store used by the caller and respondent leaves.
///
/// This is deliberately small but extensible: adding wider trees, extra branches,
/// or different block chunking only changes this store, not the endpoint routing
/// harness. The hash is not cryptographic; it is deterministic test content used to
/// exercise the protocol state machine.
#[derive(Debug, Clone)]
pub(super) struct MerkleStore {
    root_id: u32,
    children: BTreeMap<u32, Vec<TreeChild>>,
    blocks: BTreeMap<u32, Vec<Vec<u8>>>,
}

impl MerkleStore {
    /// Creates an empty store with the standard root id.
    fn new() -> Self {
        Self {
            root_id: ROOT_NODE,
            children: BTreeMap::new(),
            blocks: BTreeMap::new(),
        }
    }

    /// Returns the deterministic root hash for the current tree contents.
    pub(super) fn root_hash(&self) -> u32 {
        self.node_hash(self.root_id)
    }

    /// Returns child summaries for `node_id` in stable order.
    pub(super) fn child_summaries(&self, node_id: u32) -> Vec<ChildSummary> {
        self.children
            .get(&node_id)
            .map(|children| {
                children
                    .iter()
                    .map(|child| ChildSummary {
                        id: child.id,
                        kind: child.kind,
                        hash: self.hash_for(child.kind, child.id),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Returns the local hash for a branch or block child.
    pub(super) fn hash_for(&self, kind: ChildKind, id: u32) -> u32 {
        match kind {
            ChildKind::Branch => self.node_hash(id),
            ChildKind::Block => self.block_hash(id),
        }
    }

    /// Returns the stored chunks for a block, preserving stream order.
    pub(super) fn block_chunks(&self, block_id: u32) -> Vec<Vec<u8>> {
        self.blocks.get(&block_id).cloned().unwrap_or_default()
    }

    /// Replaces one local block after a complete block stream arrives.
    pub(super) fn replace_block(&mut self, block_id: u32, chunks: Vec<Vec<u8>>) {
        self.blocks.insert(block_id, chunks);
    }

    /// Computes a deterministic hash for a branch node.
    fn node_hash(&self, node_id: u32) -> u32 {
        let mut hash = mix_u32(0x4E4F_4445, node_id);

        if let Some(children) = self.children.get(&node_id) {
            for child in children {
                hash = mix_u32(hash, child.id);
                hash = mix_u32(hash, child.kind.discriminant());
                hash = mix_u32(hash, self.hash_for(child.kind, child.id));
            }
        }

        hash
    }

    /// Computes a deterministic hash for a data block.
    fn block_hash(&self, block_id: u32) -> u32 {
        let mut hash = mix_u32(0x424C_4F43, block_id);

        if let Some(chunks) = self.blocks.get(&block_id) {
            for chunk in chunks {
                hash = mix_u32(hash, chunk.len() as u32);
                hash = hash_bytes(hash, chunk);
            }
        }

        hash
    }
}

impl ChildKind {
    /// Stable wire discriminant for streamed child summaries.
    pub(super) fn discriminant(self) -> u32 {
        match self {
            ChildKind::Branch => 0,
            ChildKind::Block => 1,
        }
    }

    /// Decodes a stable wire discriminant.
    pub(super) fn from_discriminant(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Branch),
            1 => Some(Self::Block),
            _ => None,
        }
    }
}

/// Remote store containing the authoritative content.
pub(super) fn remote_fixture() -> MerkleStore {
    let mut store = base_tree();
    store
        .blocks
        .insert(BLOCK_ALPHA, chunks(&["alpha-", "same"]));
    store
        .blocks
        .insert(BLOCK_BRAVO, chunks(&["bravo-", "remote-", "v2"]));
    store
        .blocks
        .insert(BLOCK_CHARLIE, chunks(&["charlie-", "remote"]));
    store.blocks.insert(BLOCK_DELTA, chunks(&["delta-same"]));
    store
}

/// Local store with two stale blocks and two already matching blocks.
pub(super) fn local_fixture() -> MerkleStore {
    let mut store = base_tree();
    store
        .blocks
        .insert(BLOCK_ALPHA, chunks(&["alpha-", "same"]));
    store
        .blocks
        .insert(BLOCK_BRAVO, chunks(&["bravo-", "local-", "v1"]));
    store
        .blocks
        .insert(BLOCK_CHARLIE, chunks(&["charlie-", "local"]));
    store.blocks.insert(BLOCK_DELTA, chunks(&["delta-same"]));
    store
}

/// Tree topology shared by the local and remote fixtures.
fn base_tree() -> MerkleStore {
    let mut store = MerkleStore::new();
    store.children.insert(
        ROOT_NODE,
        vec![
            TreeChild {
                id: BRANCH_LEFT,
                kind: ChildKind::Branch,
            },
            TreeChild {
                id: BRANCH_RIGHT,
                kind: ChildKind::Branch,
            },
        ],
    );
    store.children.insert(
        BRANCH_LEFT,
        vec![
            TreeChild {
                id: BLOCK_ALPHA,
                kind: ChildKind::Block,
            },
            TreeChild {
                id: BLOCK_BRAVO,
                kind: ChildKind::Block,
            },
        ],
    );
    store.children.insert(
        BRANCH_RIGHT,
        vec![
            TreeChild {
                id: BLOCK_CHARLIE,
                kind: ChildKind::Block,
            },
            TreeChild {
                id: BLOCK_DELTA,
                kind: ChildKind::Block,
            },
        ],
    );
    store
}

/// Converts string slices into owned byte chunks.
fn chunks(parts: &[&str]) -> Vec<Vec<u8>> {
    parts.iter().map(|part| part.as_bytes().to_vec()).collect()
}

/// FNV-like byte mixing used only for deterministic test hashes.
fn hash_bytes(mut hash: u32, bytes: &[u8]) -> u32 {
    for byte in bytes {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(16_777_619);
    }

    hash
}

/// Mixes one little-endian integer into the deterministic test hash.
fn mix_u32(hash: u32, value: u32) -> u32 {
    hash_bytes(hash, &value.to_le_bytes())
}
