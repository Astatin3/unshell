//! # Protocol Wire Types
//!
//! All structs and enums that appear on the wire.
//!
//! ## Serialisation
//!
//! Every type here derives rkyv's `Archive`, `Serialize`, and `Deserialize`.
//! This means they can be serialised to a byte slice and deserialised back
//! with zero copying — the deserialised view (`Archived<T>`) reads directly
//! from the byte slice without allocating.
//!
//! ## Wire Frame Format
//!
//! Every packet on the wire uses a two-part frame:
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────────────┐
//! │  Part 1: Header                          │  Part 2: Payload          │
//! │  [u32 big-endian length]                 │  [u32 big-endian length]  │
//! │  [rkyv-serialised PacketHeader bytes]    │  [rkyv payload bytes]     │
//! └──────────────────────────────────────────┴───────────────────────────┘
//! ```
//!
//! The router reads only Part 1 to determine where to route the packet.
//! Part 2 is forwarded opaque (the router does not deserialise it).

use alloc::string::String;
use alloc::vec::Vec;
use rkyv::{Archive, Deserialize, Serialize};

// ---------------------------------------------------------------------------
// PacketHeader
// ---------------------------------------------------------------------------

/// The header prefixed to every packet on the wire.
///
/// The router reads ONLY this field to determine routing.
/// The payload body is opaque to the router.
///
/// # Example
///
/// ```rust
/// use unshell::protocol::{PacketHeader, PacketType};
///
/// let header = PacketHeader {
///     dst_path: "/agents/abc123/shell/exec".into(),
///     src_path: "/operator/sess1".into(),
///     packet_type: PacketType::Request,
/// };
/// ```
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct PacketHeader {
    /// Destination path in the global tree.
    ///
    /// The router does a longest-prefix match against registered node paths.
    /// Example: `"/agents/abc123/shell/exec"`.
    pub dst_path: String,

    /// Source path of the sending node.
    ///
    /// Used by the destination to route the response back.
    /// Example: `"/operator/sess1"`.
    pub src_path: String,

    /// Discriminates between handshake messages and protocol messages.
    pub packet_type: PacketType,
}

/// Discriminates the payload type.
///
/// The receiver uses this to know which type to deserialise the payload as.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[rkyv(derive(Debug, PartialEq))]
pub enum PacketType {
    /// Sent by a newly-connected node to register with the router.
    Handshake,
    /// Sent by the router acknowledging (or rejecting) a handshake.
    HandshakeAck,
    /// An application-level request (the primary protocol message).
    Request,
    /// An application-level response.
    Response,
}

// ---------------------------------------------------------------------------
// Handshake
// ---------------------------------------------------------------------------

/// Sent by a node immediately after connecting to the router.
///
/// The router reads this to register the node in its routing table.
///
/// # Wire format
///
/// This struct is the payload part of a frame whose header has
/// `packet_type = PacketType::Handshake`. The `dst_path` in the header is
/// `"/router"` (the router's own registration endpoint).
///
/// # Example
///
/// ```rust
/// use unshell::protocol::{HandshakeMessage, NodeType};
///
/// let msg = HandshakeMessage {
///     node_id: "abc123".into(),
///     node_type: NodeType::Payload,
///     registered_paths: vec!["/agents/abc123".into()],
///     platform: "linux-x86_64".into(),
/// };
/// ```
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct HandshakeMessage {
    /// Node identifier.
    ///
    /// For payloads: a base62 string baked at compile time.
    /// For operator sessions: a random string generated on startup.
    pub node_id: String,

    /// Whether this node is a payload or an operator shell.
    pub node_type: NodeType,

    /// The path prefixes this node claims ownership of.
    ///
    /// All sub-paths under these prefixes are owned by this node.
    /// The router uses these for longest-prefix route matching.
    ///
    /// Example: `["/agents/abc123"]`
    pub registered_paths: Vec<String>,

    /// Human-readable platform identifier for operator visibility.
    ///
    /// Example: `"linux-x86_64"`, `"windows-x86_64"`, `"operator"`.
    pub platform: String,
}

/// Sent by the router in response to a `HandshakeMessage`.
///
/// # Example
///
/// ```rust
/// use unshell::protocol::HandshakeAck;
///
/// // Successful registration
/// let ack = HandshakeAck {
///     accepted: true,
///     assigned_base_path: "/agents/abc123".into(),
///     rejection_reason: None,
/// };
///
/// // Rejection (duplicate node ID)
/// let nack = HandshakeAck {
///     accepted: false,
///     assigned_base_path: String::new(),
///     rejection_reason: Some("duplicate_node_id".into()),
/// };
/// ```
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct HandshakeAck {
    /// Whether the router accepted the registration.
    pub accepted: bool,

    /// The canonical base path assigned by the router.
    ///
    /// Typically matches the first entry in `HandshakeMessage::registered_paths`.
    /// Empty string if `accepted == false`.
    pub assigned_base_path: String,

    /// Human-readable rejection reason when `accepted == false`.
    ///
    /// Known values: `"duplicate_node_id"`, `"invalid_path"`.
    pub rejection_reason: Option<String>,
}

/// The type of node connecting to the router.
///
/// The `Router` variant is reserved for future multi-hop/pivoting support
/// and is not used in v1.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[rkyv(derive(Debug, PartialEq))]
pub enum NodeType {
    /// An implant running on a target machine.
    Payload,
    /// An operator's interactive shell session.
    Operator,
    // Router variant will be added when multi-hop/pivoting is implemented.
    // Router,
}

// ---------------------------------------------------------------------------
// TreeRequest / TreeResponse
// ---------------------------------------------------------------------------

/// An application-level request sent from an operator to a payload module.
///
/// The request travels: operator → router → destination node.
///
/// # Example
///
/// ```rust
/// use unshell::protocol::{TreeRequest, RequestType, content};
///
/// // Ask a shell module to execute a command
/// let req = TreeRequest {
///     request_id: 42,
///     request_type: RequestType::CallProcedure,
///     content_type: content::UTF8_STRING.into(),
///     data: b"ls -la /tmp".to_vec(),
/// };
/// ```
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct TreeRequest {
    /// Unique request ID generated by the sender.
    ///
    /// The responder echoes this back in [`TreeResponse::request_id`].
    /// This allows the sender to match responses to outstanding requests,
    /// which matters when multiple requests are in-flight concurrently
    /// (e.g., background sessions in the operator CLI).
    pub request_id: u64,

    /// The operation type.
    pub request_type: RequestType,

    /// Content-type describing how to interpret [`data`](Self::data).
    ///
    /// Use the constants in [`content`](super::content) for the built-in types.
    /// Custom module types should use the module name as namespace:
    /// `"mymodule/MyType"`.
    pub content_type: String,

    /// Operation payload. Interpretation depends on `content_type`.
    pub data: Vec<u8>,
}

/// The type of operation being requested.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[rkyv(derive(Debug, PartialEq))]
pub enum RequestType {
    /// Read a value at the target path.
    Read = 0,
    /// List available sub-paths and callable procedures at the target path.
    GetProcedures = 1,
    /// Write a value to the target path.
    Write = 2,
    /// Invoke a named procedure at the target path.
    CallProcedure = 3,
}

/// An application-level response from a payload module back to the operator.
///
/// The response travels: payload → router → requesting operator.
///
/// # Example
///
/// ```rust
/// use unshell::protocol::{TreeResponse, ResponseStatus, content};
///
/// let resp = TreeResponse {
///     request_id: 42, // echoed from the corresponding TreeRequest
///     status: ResponseStatus::Ok,
///     content_type: content::UTF8_STRING.into(),
///     data: b"file1.txt\nfile2.txt\n".to_vec(),
/// };
/// ```
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct TreeResponse {
    /// Echoed from the corresponding [`TreeRequest::request_id`].
    pub request_id: u64,

    /// Whether the operation succeeded.
    pub status: ResponseStatus,

    /// Content-type of the response data.
    pub content_type: String,

    /// Response payload. Empty if `status` is an error variant.
    pub data: Vec<u8>,
}

/// Indicates the outcome of a [`TreeRequest`].
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[rkyv(derive(Debug, PartialEq))]
pub enum ResponseStatus {
    /// The operation completed successfully.
    Ok = 0,
    /// The requested path does not exist at the destination node.
    NoBranchError = 1,
    /// The requested operation is not supported at this path.
    UnsupportedOperation = 2,
    /// The destination node encountered an internal error.
    ExecutionError = 3,
    /// The request payload was malformed or could not be deserialised.
    ProtocolError = 4,
}

/// A descriptor for a callable procedure, returned by [`RequestType::GetProcedures`].
///
/// This is what fills the `data` field of a `TreeResponse` when the
/// request type is `GetProcedures` and `content_type` is `content::PROCEDURE_LIST`.
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct ProcedureDescriptor {
    /// The name of the procedure (the path component after the module path).
    ///
    /// Example: `"exec"` for the module at `/agents/abc123/shell/exec`.
    pub name: String,

    /// Human-readable description of what this procedure does.
    pub description: String,
}
