use crate::protocol::{Endpoint, EndpointError, Packet, RouteDirection};

impl Endpoint {
    /// Register an inbound packet from legacy trusted code.
    ///
    /// Transports should prefer [`Self::add_inbound_from`] because peer-bound hook
    /// validation needs to know which adjacent endpoint supplied the bytes. This
    /// method keeps the old trusted in-process path small: it derives path direction,
    /// forwards or delivers the packet, and only checks that upward hooks exist.
    pub fn add_inbound(&mut self, packet: Packet) -> Result<(), EndpointError> {
        self.route_trusted_packet(packet)
    }

    /// Register an inbound packet received from `remote_id` and route it locally.
    ///
    /// Packets from a parent are downward traffic and pave return hooks when
    /// `end_hook` is false. Packets from a child are upward traffic and must match an
    /// already-paved hook for that exact child before they can move farther upward.
    pub fn add_inbound_from(
        &mut self,
        remote_id: u32,
        packet: Packet,
    ) -> Result<(), EndpointError> {
        self.ensure_path_is_set()?;

        let inbound_direction = self.inbound_direction_from_peer(remote_id)?;

        if packet.path == self.path {
            return match inbound_direction {
                RouteDirection::Downward => self.deliver_local_downward(packet, remote_id),
                RouteDirection::Upward => self.deliver_local_upward(packet, remote_id),
            };
        }

        if packet.path.starts_with(&self.path) {
            self.ensure_inbound_direction(remote_id, inbound_direction, RouteDirection::Downward)?;
            let next_hop = self.immediate_child_hop(&packet)?;
            return self.route_downward(packet, next_hop);
        }

        if self.path.starts_with(&packet.path) {
            self.ensure_inbound_direction(remote_id, inbound_direction, RouteDirection::Upward)?;
            let next_hop = self.parent_hop()?;
            return self.route_upward(packet, next_hop, Some(remote_id));
        }

        Err(EndpointError::DestinationOutsideLocalTree)
    }

    /// Register an outbound packet produced locally and route it to the next queue.
    pub fn add_outbound(&mut self, packet: Packet) -> Result<(), EndpointError> {
        self.ensure_path_is_set()?;

        if packet.path == self.path {
            return self.deliver_local(packet);
        }

        if packet.path.starts_with(&self.path) {
            let next_hop = self.immediate_child_hop(&packet)?;
            return self.route_downward(packet, next_hop);
        }

        if self.path.starts_with(&packet.path) {
            let next_hop = self.parent_hop()?;
            return self.route_upward(packet, next_hop, Some(next_hop));
        }

        Err(EndpointError::DestinationOutsideLocalTree)
    }

    /// Routes a trusted packet without transport-peer direction metadata.
    ///
    /// This intentionally does not create local hooks on local delivery because the
    /// endpoint cannot know whether the packet came from a parent or child. Transit
    /// routing still maintains hook state where path direction is unambiguous.
    fn route_trusted_packet(&mut self, packet: Packet) -> Result<(), EndpointError> {
        self.ensure_path_is_set()?;

        if packet.path == self.path {
            return self.deliver_local(packet);
        }

        if packet.path.starts_with(&self.path) {
            let next_hop = self.immediate_child_hop(&packet)?;
            return self.route_downward(packet, next_hop);
        }

        if self.path.starts_with(&packet.path) {
            let next_hop = self.parent_hop()?;
            return self.route_upward(packet, next_hop, None);
        }

        Err(EndpointError::DestinationOutsideLocalTree)
    }

    /// Delivers a packet to local leaves without changing hook state.
    fn deliver_local(&mut self, packet: Packet) -> Result<(), EndpointError> {
        let local_id = self.local_id()?;
        Self::route_push(local_id, packet, &mut self.inbound);
        Ok(())
    }

    /// Delivers parent-originated traffic locally and applies downward hook policy.
    fn deliver_local_downward(&mut self, packet: Packet, peer: u32) -> Result<(), EndpointError> {
        let hook_id = packet.hook_id;
        let end_hook = packet.end_hook;

        self.deliver_local(packet)?;
        self.apply_downward_hook_lifecycle(hook_id, end_hook, peer);
        Ok(())
    }

    /// Delivers child-originated traffic locally after validating its return hook.
    fn deliver_local_upward(&mut self, packet: Packet, peer: u32) -> Result<(), EndpointError> {
        let hook_id = packet.hook_id;
        let end_hook = packet.end_hook;

        self.ensure_hook_peer(hook_id, peer)?;
        self.deliver_local(packet)?;
        self.apply_upward_hook_lifecycle(hook_id, end_hook);
        Ok(())
    }

    /// Forwards a packet to a child and applies downward hook lifecycle rules.
    fn route_downward(&mut self, packet: Packet, next_hop: u32) -> Result<(), EndpointError> {
        let hook_id = packet.hook_id;
        let end_hook = packet.end_hook;

        self.ensure_registered_connection(next_hop, RouteDirection::Downward)?;
        Self::route_push(next_hop, packet, &mut self.outbound);
        self.apply_downward_hook_lifecycle(hook_id, end_hook, next_hop);
        Ok(())
    }

    /// Forwards a packet toward the parent after validating hook state.
    ///
    /// `actual_peer` is `None` only for legacy trusted inbound routing where the
    /// transport source is unknown; in that mode the endpoint can check that a hook
    /// exists but cannot enforce peer ownership.
    fn route_upward(
        &mut self,
        packet: Packet,
        next_hop: u32,
        actual_peer: Option<u32>,
    ) -> Result<(), EndpointError> {
        let hook_id = packet.hook_id;
        let end_hook = packet.end_hook;

        self.ensure_upward_hook_peer(hook_id, actual_peer)?;
        self.ensure_registered_connection(next_hop, RouteDirection::Upward)?;
        Self::route_push(next_hop, packet, &mut self.outbound);
        self.apply_upward_hook_lifecycle(hook_id, end_hook);
        Ok(())
    }

    /// Returns this endpoint's final path segment for local queueing.
    fn local_id(&self) -> Result<u32, EndpointError> {
        self.path
            .last()
            .copied()
            .ok_or(EndpointError::EndpointPathUnset)
    }

    /// Returns the child that should receive a downward packet next.
    fn immediate_child_hop(&self, packet: &Packet) -> Result<u32, EndpointError> {
        packet
            .path
            .get(self.path.len())
            .copied()
            .ok_or(EndpointError::DestinationOutsideLocalTree)
    }

    /// Returns the direct parent next hop for upward routing.
    fn parent_hop(&self) -> Result<u32, EndpointError> {
        let parent_index = self
            .path
            .len()
            .checked_sub(2)
            .ok_or(EndpointError::MissingParentRoute)?;

        Ok(self.path[parent_index])
    }

    /// Reject routing before path-relative decisions when no absolute path is known.
    ///
    /// This preserves the current runtime sentinel where an empty path means the
    /// endpoint has not been attached to the tree yet.
    fn ensure_path_is_set(&self) -> Result<(), EndpointError> {
        if self.path.is_empty() {
            Err(EndpointError::EndpointPathUnset)
        } else {
            Ok(())
        }
    }

    /// Derives packet direction from a registered inbound adjacent peer.
    fn inbound_direction_from_peer(&self, remote_id: u32) -> Result<RouteDirection, EndpointError> {
        let is_upstream = self.connection_contains(remote_id, true);
        let is_downstream = self.connection_contains(remote_id, false);

        match (is_upstream, is_downstream) {
            (true, false) => Ok(RouteDirection::Downward),
            (false, true) => Ok(RouteDirection::Upward),
            (false, false) => Err(EndpointError::UnknownConnection { remote_id }),
            (true, true) => Err(EndpointError::AmbiguousConnection { remote_id }),
        }
    }

    /// Rejects inbound packets whose path-derived direction contradicts the connection.
    fn ensure_inbound_direction(
        &self,
        remote_id: u32,
        expected: RouteDirection,
        actual: RouteDirection,
    ) -> Result<(), EndpointError> {
        if expected == actual {
            Ok(())
        } else {
            Err(EndpointError::InboundDirectionMismatch {
                remote_id,
                expected,
                actual,
            })
        }
    }

    /// Verify that the derived adjacent endpoint is registered in this direction.
    ///
    /// The current connection table stores direction as a boolean. Keeping the bool
    /// conversion here confines that legacy representation to one place in routing.
    fn ensure_registered_connection(
        &self,
        next_hop: u32,
        direction: RouteDirection,
    ) -> Result<(), EndpointError> {
        let is_upward = matches!(direction, RouteDirection::Upward);

        if self.connection_contains(next_hop, is_upward) {
            Ok(())
        } else {
            Err(EndpointError::MissingConnection {
                next_hop,
                direction,
            })
        }
    }

    /// Validates hook state for upward routing.
    fn ensure_upward_hook_peer(
        &self,
        hook_id: u16,
        actual_peer: Option<u32>,
    ) -> Result<(), EndpointError> {
        if let Some(actual_peer) = actual_peer {
            self.ensure_hook_peer(hook_id, actual_peer)
        } else if self.has_hook(hook_id) {
            Ok(())
        } else {
            Err(EndpointError::UnknownHook { hook_id })
        }
    }

    /// Applies hook state for successfully routed downward packets.
    fn apply_downward_hook_lifecycle(&mut self, hook_id: u16, end_hook: bool, peer: u32) {
        if end_hook {
            self.close_hook(hook_id);
        } else {
            self.open_hook(hook_id, peer);
        }
    }

    /// Applies hook cleanup for successfully routed upward final packets.
    fn apply_upward_hook_lifecycle(&mut self, hook_id: u16, end_hook: bool) {
        if end_hook {
            self.close_hook(hook_id);
        }
    }
}
