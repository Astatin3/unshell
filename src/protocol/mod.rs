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
use alloc::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet, vec_deque::VecDeque},
    vec::Vec,
};

type Path = Vec<u32>;
type EndpointName = u32;
type ConnectionSet = BTreeSet<(EndpointName, bool)>;
type HookMap = BTreeMap<HookID, EndpointName>;
pub type PacketQueue = VecDeque<Packet>;
type RouteMap = BTreeMap<EndpointName, PacketQueue>;

#[cfg(test)]
mod tests {
    mod merkle_sync;
    mod oneshot;
    mod packet;
}
