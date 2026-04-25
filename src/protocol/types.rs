//! Canonical UnShell protocol message types.

use alloc::{string::String, vec::Vec};
use rkyv::{Archive, Deserialize, Serialize};

/// The three protocol packet types.
#[repr(u8)]
#[derive(Archive, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    /// Downwards procedure invocation.
    Call = 0x01,
    /// Returned or continuing hook traffic.
    Data = 0x02,
    /// Upstream protocol failure tied to a hook.
    Fault = 0xFF,
}

/// Header fields used for routing and hook attribution.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PacketHeader {
    pub packet_type: PacketType,
    pub src_path: Vec<String>,
    pub dst_path: Vec<String>,
    pub dst_leaf: Option<String>,
    pub hook_id: Option<u64>,
}

/// Hook declaration embedded inside a call.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct HookTarget {
    pub hook_id: u64,
    pub return_path: Vec<String>,
}

/// Downwards call payload.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CallMessage {
    pub procedure_id: String,
    pub data: Vec<u8>,
    pub response_hook: Option<HookTarget>,
}

/// Hook data payload.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DataMessage {
    pub procedure_id: String,
    pub data: Vec<u8>,
    pub end_hook: bool,
}

/// Protocol fault payload.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct FaultMessage {
    pub fault: ProtocolFault,
}

/// Stable protocol fault set.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolFault(pub u8);

impl ProtocolFault {
    pub const UNKNOWN_LEAF: Self = Self(0x01);
    pub const UNKNOWN_PROCEDURE: Self = Self(0x02);
    pub const INVALID_SOURCE_PATH: Self = Self(0x03);
    pub const INVALID_HOOK_PEER: Self = Self(0x04);
    pub const INTERNAL_ERROR: Self = Self(0x05);
}
