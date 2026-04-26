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
    /// Wire-level packet class, which determines which payload type follows.
    pub packet_type: PacketType,
    /// Absolute endpoint path that sent the packet.
    pub src_path: Vec<String>,
    /// Absolute endpoint path the packet is trying to reach.
    pub dst_path: Vec<String>,
    /// Optional leaf name inside `dst_path` that should receive a `Call` packet.
    ///
    /// `Data` and `Fault` packets must leave this unset.
    pub dst_leaf: Option<String>,
    /// Hook identifier scoped to the receiving endpoint.
    ///
    /// `Call` packets must leave this unset. `Data` and `Fault` packets must fill it in.
    pub hook_id: Option<u64>,
}

/// Hook declaration embedded inside a call.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct HookTarget {
    /// Hook identifier reserved by the caller for returned `Data` or `Fault` traffic.
    pub hook_id: u64,
    /// Absolute endpoint path that should receive the response stream.
    ///
    /// Protocol validation requires this to exactly match the enclosing call header's
    /// `src_path`.
    pub return_path: Vec<String>,
}

/// Downwards call payload.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CallMessage {
    /// Canonical procedure identifier chosen by the caller.
    pub procedure_id: String,
    /// Opaque application payload for the target procedure.
    pub data: Vec<u8>,
    /// Optional response hook reservation for returned hook traffic.
    pub response_hook: Option<HookTarget>,
}

/// Hook data payload.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DataMessage {
    /// Canonical procedure identifier that owns the hook stream.
    pub procedure_id: String,
    /// Opaque application payload for the hook message.
    pub data: Vec<u8>,
    /// Whether this packet closes the peer side of the hook stream.
    pub end_hook: bool,
}

/// Protocol fault payload.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct FaultMessage {
    /// Stable protocol-level reason code for the failure.
    pub fault: ProtocolFault,
}

/// Stable protocol fault code.
///
/// The raw numeric value is public so callers can persist, compare, or forward fault codes
/// without knowing every symbolic constant in advance. Unknown values are allowed so newer
/// peers can extend the set without breaking older runtimes.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolFault(pub u8);

impl ProtocolFault {
    /// The addressed leaf name does not exist at the destination endpoint.
    pub const UNKNOWN_LEAF: Self = Self(0x01);
    /// The destination exists, but it does not expose the requested procedure id.
    pub const UNKNOWN_PROCEDURE: Self = Self(0x02);
    /// The packet source path is not valid for the ingress side where it arrived.
    pub const INVALID_SOURCE_PATH: Self = Self(0x03);
    /// Hook traffic arrived from a peer that does not own the active hook relationship.
    pub const INVALID_HOOK_PEER: Self = Self(0x04);
    /// The runtime hit an internal protocol failure and could only surface a generic fault.
    pub const INTERNAL_ERROR: Self = Self(0x05);
}
