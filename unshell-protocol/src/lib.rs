#![no_std]

pub extern crate alloc;

mod endpoint;
mod error;
mod leaf;
mod packet;

pub use endpoint::{Endpoint, HookID};
pub use error::*;
pub use leaf::*;
pub use packet::Packet;

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
