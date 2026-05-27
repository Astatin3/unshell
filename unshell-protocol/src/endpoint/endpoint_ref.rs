use alloc::{format, string::ToString};

use crate::{
    endpoint::error::EndpointError,
    packet::Packet,
    types::{ConnectionSet, HookMap, Path, RouteMap},
};

#[derive(Debug)]
pub struct EndpointRef<'a> {
    pub name: &'static str,
    pub path: &'a Path,

    pub hooks: &'a mut HookMap,

    pub connections: &'a mut ConnectionSet,

    pub inbound: &'a mut RouteMap,
    pub outbound: &'a mut RouteMap,
}

impl<'a> EndpointRef<'a> {
    pub fn add_inbound(&mut self, packet: Packet) -> Result<(), EndpointError> {
        // If the packet is routed towards this endpoint
        if packet.path.ends_with(self.name) {
            if packet.is_upwards_call {
                self.hooks.insert(packet.hook_id, packet.path.clone());
            }

            self.outbound
                .entry(packet.path.clone())
                .or_default()
                .push_back(packet);

            Ok(())
        } else {
            // If the absolute path of this endpoint hasn't been set yet
            if self.path.is_empty() {
                return Err(EndpointError::NoAbsoultePathYet);
            }

            if *self.path == packet.path {
                return Err(EndpointError::IncorrectAbsolutePath);
            }

            // For routing
            let connection = if packet.is_upwards_call && self.path.starts_with(&packet.path) {
                (
                    packet
                        .path
                        .rsplit_once('/')
                        .map_or(packet.path.clone(), |(_, after)| after.to_string()),
                    true,
                )
            } else if packet
                .path
                .starts_with(&format!("{}/{}", self.path, self.name))
            {
                let concat_len = self.path.len() + self.name.len();

                let after_self = &packet.path[concat_len..];

                (
                    after_self
                        .split_once('/')
                        .map_or(after_self.to_string(), |(before, _)| before.to_string()),
                    false,
                )
            } else {
                return Err(EndpointError::IncorrectAbsolutePath);
            };

            if !self.connections.contains(&connection) {
                return Err(EndpointError::RouteNotExist);
            }

            self.add_outbound(packet);

            Ok(())
        }
    }

    pub fn add_outbound_upwards(&mut self, packet: Packet) -> Result<(), EndpointError> {
        let next_hop = self
            .hooks
            .get(&packet.hook_id)
            .ok_or(EndpointError::RouteNotExist)?
            .clone();

        if packet.end_hook {
            let _ = self.hooks.remove(&packet.hook_id);
        }

        self.outbound
            .entry(next_hop.clone())
            .or_default()
            .push_back(packet);

        Ok(())
    }

    pub fn add_outbound_downwards(&mut self, packet: Packet) -> Result<(), EndpointError> {
        let next_hop = self
            .hooks
            .get(&packet.hook_id)
            .ok_or(EndpointError::RouteNotExist)?
            .clone();

        if packet.end_hook {
            let _ = self.hooks.remove(&packet.hook_id);
        }

        self.outbound
            .entry(next_hop.clone())
            .or_default()
            .push_back(packet);

        Ok(())
    }

    pub fn take_intbound<F>(&mut self, path: &str, f: F)
    where
        F: FnMut(&Packet),
    {
        if let Some(queue) = self.inbound.get_mut(path) {
            let _ = queue.iter().map(f);

            queue.clear();
        }
    }

    pub fn take_outbound<F>(&mut self, path: &str, f: F)
    where
        F: FnMut(&Packet),
    {
        if let Some(queue) = self.inbound.get_mut(path) {
            let _ = queue.iter().map(f);

            queue.clear();
        }
    }
}

// fn get_last_term_in_path(path: &Path) -> &str {}
