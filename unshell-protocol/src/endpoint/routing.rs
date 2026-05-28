use crate::{
    endpoint::{Endpoint, error::EndpointError},
    packet::Packet,
};

impl Endpoint {
    /// Register an inbound packet and route it
    pub fn add_inbound(&mut self, packet: Packet) -> Result<(), EndpointError> {
        // In case some leaf hasn't assigned the endpoint a path yet.
        if self.path.is_empty() {
            return Err(EndpointError::NoAbsoultePathYet);
        }

        // If the packet is routed towards this endpoint
        if packet.path == *self.path {
            // Get the last segment of the path
            let local_id = self
                .path
                .last()
                .cloned()
                .ok_or(EndpointError::IncorrectAbsolutePath)?;

            self.inbound.entry(local_id).or_default().push_back(packet);

            Ok(())
        } else {
            let (next_hop, is_upward) = self.next_hop_for(&packet)?;

            if !self.connections.contains(&(next_hop, is_upward)) {
                return Err(EndpointError::RouteNotExist);
            }

            self.queue_outbound(packet, next_hop)
        }
    }

    pub fn add_outbound(&mut self, packet: Packet) -> Result<(), EndpointError> {
        // In case some leaf hasn't assigned the endpoint a path yet.
        if self.path.is_empty() {
            return Err(EndpointError::NoAbsoultePathYet);
        }

        // If this packet is routed towards this node
        if packet.path == *self.path {
            // Grab the last endpoint ID
            let local_id = self
                .path
                .last()
                .cloned()
                .ok_or(EndpointError::IncorrectAbsolutePath)?;

            // Add it to the inbound queue
            self.inbound.entry(local_id).or_default().push_back(packet);

            return Ok(());
        }

        let (next_hop, is_upward) = self.next_hop_for(&packet)?;

        if !self.connections.contains(&(next_hop, is_upward)) {
            return Err(EndpointError::RouteNotExist);
        }

        self.queue_outbound(packet, next_hop)
    }

    fn queue_outbound(&mut self, packet: Packet, next_hop: u32) -> Result<(), EndpointError> {
        if packet.end_hook {
            self.hooks.remove(&packet.hook_id);
        }

        self.outbound.entry(next_hop).or_default().push_back(packet);

        Ok(())
    }

    fn next_hop_for(&self, packet: &Packet) -> Result<(u32, bool), EndpointError> {
        // Direction is derived from the local path. The packet never gets to declare
        // whether it is moving upward, because that would make the trust boundary spoofable.
        if packet.path.starts_with(&self.path) {
            let next_hop = packet
                .path
                .get(self.path.len())
                .cloned()
                .ok_or(EndpointError::IncorrectAbsolutePath)?;

            Ok((next_hop, false))
        } else if self.path.starts_with(&packet.path) {
            // SECURITY: All upward-routed packets must be checked against local hook state.
            if !self.hooks.contains_key(&packet.hook_id) {
                return Err(EndpointError::HookNotExist);
            }

            let parent_index = self
                .path
                .len()
                .checked_sub(2)
                .ok_or(EndpointError::RouteNotExist)?;

            Ok((self.path[parent_index], true))
        } else {
            Err(EndpointError::IncorrectAbsolutePath)
        }
    }
}
