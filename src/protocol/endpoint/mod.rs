mod hooks;
mod routing;

pub use hooks::HookID;

use alloc::{boxed::Box, vec::Vec};

use crate::{
    crypto::Counter,
    protocol::{ConnectionSet, HookMap, Leaf, Packet, Path, RouteMap},
};

pub struct Endpoint {
    // This endpoint's identifier
    pub id: u32,

    // A counter that creates unique hook IDs.
    pub(crate) last_hook: Counter,

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
            last_hook: Counter::new(),

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

    /// Drain inbound packets for `path` that match `predicate` and preserve the rest.
    ///
    /// Generated leaf dispatch uses this instead of [`Self::take_inbound_clear`] so
    /// one leaf can consume only its procedure or session packets without stealing
    /// traffic intended for another leaf. Matching packets are passed by value because
    /// most handlers need to move payload bytes into application state; unmatched
    /// packets are reinserted in their original FIFO order.
    pub fn take_inbound_matching<P, F>(&mut self, path: u32, mut predicate: P, mut f: F)
    where
        P: FnMut(&Packet) -> bool,
        F: FnMut(Packet),
    {
        let Some(mut queue) = self.inbound.remove(&path) else {
            return;
        };

        let mut unmatched = Vec::new();

        while let Some(packet) = queue.pop_front() {
            if predicate(&packet) {
                f(packet);
            } else {
                unmatched.push(packet);
            }
        }

        if !unmatched.is_empty() {
            self.inbound.entry(path).or_default().extend(unmatched);
        }
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

    pub fn iter_leaves<F>(&mut self) -> core::slice::IterMut<'_, Box<dyn Leaf + 'static>>
    where
        F: FnMut(&Packet),
    {
        self.leaves.iter_mut()
    }
}
