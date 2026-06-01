use alloc::vec::Vec;

use crate::protocol::{Endpoint, EndpointName, Packet, PacketQueue, RouteMap};

impl Endpoint {
    /// Runs a function over all inbound packets for `path`, then clears that queue.
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

    /// Runs a function over all outbound packets for `path`, then clears that queue.
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
}
