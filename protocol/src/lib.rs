//! The 'tree' protocol is a transport layer protocol
//! designed to be stacked on others, and to be built
//! on with emergent complexity and minimal overhead.
//!
//! Here are the core principles:
//! 1) The root node is the most trusted
//! 2) Packets can never be spontaneously sent from a lower level
//! 3) 'Streams' are freeform, they exist if a downwards packet
//!    doesn't have the 'close' bit set.
//! 4) There's very little intermediary structure required,
//!    error responses aren't handled, the handshake process
//!    isn't defined. Nodes are expected to handle their
//!    own security.

mod packet;
mod router;

pub use packet::PacketHeader;
pub use router::Router;

/// Node IDs are the identifiers of each node
/// They must be unique among the direct children
/// of any given node, but are not required to be unique
/// in any other circumstance.
///
/// Since the path is defined explicitly, this isn't a problem.
type NodeID = u32;

/// The Hook ID is the unique identifier of any given
/// bidirectional channel. It targets a specific procedure
/// on a child.
///
/// It must be unique among a pair of node ids,
/// and a procedure ID on the child
///
/// This can be used both as a bidirectional stream and as
/// a producer/subscriber channel, or anything else that
/// can be defined in this format.
type HookID = u32;

/// A procedure is an identifier to any given component
/// on a node. It can be a static remote proc method,
/// another protocol stacked on top of the tree protocol,
/// whatever.
type ProcedureID = u32;
