#![no_std]

extern crate alloc;

mod endpoint;
mod error;
mod packet;

pub use endpoint::{Endpoint, HookID};
pub use error::*;
pub use packet::Packet;

pub trait Leaf {
    // Identifier for this leaf
    fn get_id(&self) -> u32;

    // Gets called every program loop
    fn update(&mut self, _: &mut Endpoint);
}

// Various named types used for brevity
use alloc::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet, vec_deque::VecDeque},
    vec::Vec,
};

type Path = Vec<u32>;
type EndpointName = u32;
type ConnectionSet = BTreeSet<(EndpointName, bool)>;
type HookMap = BTreeMap<HookID, EndpointName>;
type PacketQueue = VecDeque<Packet>;
type RouteMap = BTreeMap<EndpointName, PacketQueue>;

#[cfg(test)]
mod tests {
    mod merkle_sync;
    mod oneshot;
    mod packet;
}
