//! Shared ids for the Merkle sync protocol test.
//!
//! Keeping ids in one file makes the manually managed leaf state easier to audit
//! and mirrors the table a future leaf-state macro would generate from annotated
//! RPC definitions.

pub(super) const ENDPOINT_CALLER: u32 = 0;
pub(super) const ENDPOINT_RESPONDENT: u32 = 1;

pub(super) const LEAF_MERKLE_CALLER: u32 = 300;
pub(super) const LEAF_MERKLE_RESPONDENT: u32 = 301;
pub(super) const LEAF_MOCK_CONNECTION: u32 = 302;

pub(super) const PROC_GET_ROOT_HASH: u32 = 10;
pub(super) const PROC_GET_CHILD_HASHES: u32 = 11;
pub(super) const PROC_GET_BLOCK_STREAM: u32 = 12;
pub(super) const PROC_ROOT_HASH: u32 = 20;
pub(super) const PROC_CHILD_HASH_ENTRY: u32 = 21;
pub(super) const PROC_BLOCK_CHUNK: u32 = 22;

pub(super) const ROOT_NODE: u32 = 0;
pub(super) const BRANCH_LEFT: u32 = 1;
pub(super) const BRANCH_RIGHT: u32 = 2;
pub(super) const BLOCK_ALPHA: u32 = 10;
pub(super) const BLOCK_BRAVO: u32 = 11;
pub(super) const BLOCK_CHARLIE: u32 = 20;
pub(super) const BLOCK_DELTA: u32 = 21;
