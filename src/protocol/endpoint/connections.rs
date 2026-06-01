use crate::protocol::{Endpoint, EndpointName};

impl Endpoint {
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
}
