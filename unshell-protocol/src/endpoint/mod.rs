mod endpoint_ref;
pub mod error;

use alloc::{boxed::Box, string::String, vec::Vec};

use crate::{
    leaf::Leaf,
    types::{ConnectionSet, HookMap, Path, RouteMap},
};

pub use endpoint_ref::EndpointRef;

pub struct Endpoint {
    pub name: &'static str,

    // Absolute path for this node.
    pub path: Path,
    pub leaves: Vec<Box<dyn Leaf>>,

    pub connections: ConnectionSet,

    pub hooks: HookMap,
    pub inbound: RouteMap,
    pub outbound: RouteMap,
}

impl Endpoint {
    pub fn new(name: &'static str, leaves: Vec<Box<dyn Leaf>>) -> Self {
        Self {
            name,
            path: String::new(),
            leaves,
            hooks: HookMap::new(),
            connections: ConnectionSet::new(),
            inbound: RouteMap::new(),
            outbound: RouteMap::new(),
        }
    }

    pub fn update(&mut self) {
        let mut self_ref = EndpointRef {
            name: self.name,
            path: &mut self.path,
            hooks: &mut self.hooks,
            connections: &mut self.connections,
            inbound: &mut self.inbound,
            outbound: &mut self.outbound,
        };

        let _ = self.leaves.iter_mut().map(|leaf| {
            leaf.update(&mut self_ref);
        });
    }
}
