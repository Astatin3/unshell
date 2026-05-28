pub mod error;
mod routing;

use alloc::{boxed::Box, vec::Vec};

use crate::{
    leaf::Leaf,
    packet::Packet,
    types::{ConnectionSet, HookMap, Path, RouteMap},
};

pub struct Endpoint {
    // This endpoint's identifier
    pub id: u32,

    // A counter that creates unique hook IDs.
    // TODO: Actually check if the hook ID collides with any existing hooks.
    // TODO: Randomize the hooks for more obfuscation
    last_hook: u16,

    // Absolute path for this node. Must be set by some leaf
    pub path: Path,
    pub leaves: Vec<Box<dyn Leaf>>,

    // Map of connections so that we can know what is connected
    // and which endpoints are authorities
    pub connections: ConnectionSet,

    // Local list of hooks.
    pub(crate) hooks: HookMap,

    // Map of endpoints to packet queues
    pub(crate) inbound: RouteMap,
    pub(crate) outbound: RouteMap,
}

impl Endpoint {
    pub fn new(id: u32, leaves: Vec<Box<dyn Leaf>>) -> Self {
        Self {
            id,
            // Init the hook at 0, which will increment
            last_hook: 0,

            // Set the current path as an empty vec
            path: Vec::new(),
            leaves,
            hooks: HookMap::new(),
            connections: ConnectionSet::new(),
            inbound: RouteMap::new(),
            outbound: RouteMap::new(),
        }
    }

    /// Pass the endpoint state into all of the leaves
    pub fn update(&mut self) {
        // Grab the leaf vec temporarily so that we can iter over self
        // Apparently this only swaps out pointers
        let mut leaves = core::mem::take(&mut self.leaves);

        for leaf in leaves.iter_mut() {
            leaf.update(self);
        }

        self.leaves = leaves;
    }

    /// Run a function over all inbound packets with some ID then clear it.
    pub fn take_inbound_clear<F>(&mut self, path: u32, f: F)
    where
        F: FnMut(&Packet),
    {
        Self::take_clear(path, f, &mut self.inbound);
    }

    /// Run a function over all outbound packets with some ID then clear it.
    pub fn take_outbound_clear<F>(&mut self, path: u32, f: F)
    where
        F: FnMut(&Packet),
    {
        Self::take_clear(path, f, &mut self.outbound);
    }

    fn take_clear<F>(path: u32, mut f: F, queue: &mut RouteMap)
    where
        F: FnMut(&Packet),
    {
        if let Some(queue) = queue.get_mut(&path) {
            for packet in queue.iter() {
                f(packet);
            }

            queue.clear();
        }
    }

    pub fn get_hook_id(&mut self) -> u16 {
        self.last_hook = self.last_hook.wrapping_add(1);
        self.last_hook - 1
    }
}
