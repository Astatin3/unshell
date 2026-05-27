use alloc::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet, vec_deque::VecDeque},
    string::String,
};

use crate::packet::Packet;

pub type Path = String;
pub type EndpointName = String;
pub type HookID = u16;
pub type ConnectionSet = BTreeSet<(EndpointName, bool)>;
pub type HookMap = BTreeMap<HookID, EndpointName>;
pub type PacketQueue = VecDeque<Packet>;
pub type RouteMap = BTreeMap<EndpointName, PacketQueue>;
