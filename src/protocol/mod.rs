mod endpoint;
mod error;
mod leaf;
mod leaf_meta;
mod leaf_template;
mod packet;
mod procedure;
mod runtime;
mod session;

pub use crate::unshell_leaf;
pub use endpoint::{Endpoint, HookID};
pub use error::*;
pub use leaf::Leaf;
pub use leaf_meta::LeafMeta;
pub use packet::Packet;
pub use procedure::*;
pub use runtime::*;
pub use session::*;

#[cfg(feature = "interface_ratatui")]
pub use ratatui;

// Various named types used for brevity
use alloc::{collections::vec_deque::VecDeque, vec::Vec};

type Path = Vec<u32>;
type EndpointName = u32;
type ConnectionSet = Vec<(EndpointName, bool)>;
type HookMap = Vec<(HookID, EndpointName)>;
pub type PacketQueue = VecDeque<Packet>;
type RouteMap = Vec<(EndpointName, PacketQueue)>;

#[cfg(test)]
mod tests;
