mod hooks;
mod routing;

pub use hooks::HookID;

use alloc::vec::Vec;

use crate::{
    crypto::Counter,
    protocol::{ConnectionSet, EndpointName, HookMap, Packet, PacketQueue, Path, RouteMap},
};

pub struct Endpoint {
    // This endpoint's identifier
    pub id: u32,

    // A counter that creates unique hook IDs.
    pub(crate) last_hook: Counter,

    // Absolute path for this node. Must be set by some leaf
    pub path: Path,

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
    /// Creates endpoint routing state for one protocol node.
    ///
    /// Leaves are intentionally owned by the caller instead of stored behind
    /// endpoint-local trait objects. That keeps minimized binaries from pulling in
    /// dynamic dispatch and allocation paths when a firmware-style application uses a
    /// fixed set of concrete leaves.
    pub fn new(id: u32) -> Self {
        Self {
            id,
            // Init the hook at 0, which will increment
            last_hook: Counter::new(),

            // Set the current path as an empty vec
            path: Vec::new(),
            hooks: Vec::new(),
            connections: Vec::new(),
            inbound: Vec::new(),
            outbound: Vec::new(),
        }
    }

    /// Registers an adjacent endpoint and returns whether this is a new edge.
    ///
    /// Endpoint routing tables are intentionally tiny in the minimized firmware
    /// profile. A linear vector keeps that profile from linking tree-map machinery
    /// while preserving the old set semantics: duplicate connection registrations do
    /// not create duplicate route entries.
    pub fn add_connection(&mut self, remote_id: EndpointName, is_authority: bool) -> bool {
        let connection = (remote_id, is_authority);

        if self.connection_contains(remote_id, is_authority) {
            false
        } else {
            self.connections.push(connection);
            true
        }
    }

    /// Removes an adjacent endpoint registration and reports whether it existed.
    pub fn remove_connection(&mut self, remote_id: EndpointName, is_authority: bool) -> bool {
        let Some(index) = self
            .connections
            .iter()
            .position(|connection| *connection == (remote_id, is_authority))
        else {
            return false;
        };

        self.connections.remove(index);
        true
    }

    /// Returns whether an adjacent endpoint is registered in the requested direction.
    pub fn connection_contains(&self, remote_id: EndpointName, is_authority: bool) -> bool {
        self.connections.contains(&(remote_id, is_authority))
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
        let Some(mut queue) = Self::route_remove(path, &mut self.inbound) else {
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
            Self::route_queue_mut(path, &mut self.inbound).extend(unmatched);
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
        if let Some(queue) = Self::route_queue_mut_existing(path, queue) {
            for packet in queue.iter() {
                f(packet);
            }

            queue.clear();
        }
    }

    /// Appends a packet to the route queue for `endpoint`.
    pub(crate) fn route_push(endpoint: EndpointName, packet: Packet, routes: &mut RouteMap) {
        Self::route_queue_mut(endpoint, routes).push_back(packet);
    }

    /// Returns the route queue for `endpoint` if one exists.
    #[cfg(test)]
    pub(crate) fn route_get(endpoint: EndpointName, routes: &RouteMap) -> Option<&PacketQueue> {
        routes
            .iter()
            .find(|(queued_endpoint, _)| *queued_endpoint == endpoint)
            .map(|(_, queue)| queue)
    }

    /// Removes and returns the queue for `endpoint`.
    pub(crate) fn route_remove(
        endpoint: EndpointName,
        routes: &mut RouteMap,
    ) -> Option<PacketQueue> {
        let index = routes
            .iter()
            .position(|(queued_endpoint, _)| *queued_endpoint == endpoint)?;

        Some(routes.remove(index).1)
    }

    /// Returns whether a route queue exists for `endpoint`.
    #[cfg(test)]
    pub(crate) fn route_contains(endpoint: EndpointName, routes: &RouteMap) -> bool {
        Self::route_get(endpoint, routes).is_some()
    }

    /// Returns whether no route queues are present.
    #[cfg(test)]
    pub(crate) fn routes_is_empty(routes: &RouteMap) -> bool {
        routes.is_empty()
    }

    /// Returns the route queue for `endpoint`, creating it on first use.
    fn route_queue_mut(endpoint: EndpointName, routes: &mut RouteMap) -> &mut PacketQueue {
        if let Some(index) = routes
            .iter()
            .position(|(queued_endpoint, _)| *queued_endpoint == endpoint)
        {
            &mut routes[index].1
        } else {
            routes.push((endpoint, PacketQueue::new()));
            &mut routes.last_mut().unwrap().1
        }
    }

    /// Returns the existing route queue for `endpoint` without allocating a new one.
    fn route_queue_mut_existing(
        endpoint: EndpointName,
        routes: &mut RouteMap,
    ) -> Option<&mut PacketQueue> {
        routes
            .iter_mut()
            .find(|(queued_endpoint, _)| *queued_endpoint == endpoint)
            .map(|(_, queue)| queue)
    }

    /// Inserts or updates a hook and returns the previously associated peer.
    pub(crate) fn hook_insert(
        &mut self,
        hook_id: HookID,
        peer: EndpointName,
    ) -> Option<EndpointName> {
        if let Some((_, existing_peer)) = self
            .hooks
            .iter_mut()
            .find(|(existing_hook, _)| *existing_hook == hook_id)
        {
            let previous = *existing_peer;
            *existing_peer = peer;
            Some(previous)
        } else {
            self.hooks.push((hook_id, peer));
            None
        }
    }

    /// Removes a hook and returns the peer it pointed at.
    pub(crate) fn hook_remove(&mut self, hook_id: HookID) -> Option<EndpointName> {
        let index = self
            .hooks
            .iter()
            .position(|(existing_hook, _)| *existing_hook == hook_id)?;

        Some(self.hooks.remove(index).1)
    }
}
