use alloc::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet, vec_deque::VecDeque},
    vec::Vec,
};

use crate::packet::Packet;

pub type Path = Vec<u32>;
pub type EndpointName = u32;
pub type HookID = u16;
pub type ConnectionSet = BTreeSet<(EndpointName, bool)>;
pub type HookMap = BTreeMap<HookID, EndpointName>;
pub type PacketQueue = VecDeque<Packet>;
pub type RouteMap = BTreeMap<EndpointName, PacketQueue>;
