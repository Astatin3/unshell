use crate::{
    Announcement,
    module::Manager,
    network::{Connection, Stream},
};

impl Manager {
    pub fn add_connection(&mut self, connection: Connection) {
        self.connections.push(connection);
    }

    pub fn prune_connections(&mut self) {
        self.connections.retain(|c| c.is_alive());
    }

    pub fn recv_connection_announcements(&mut self) {
        // Collect all incoming announcements
        let announcements = self
            .connections
            .iter()
            .map(|c| c.read())
            .flat_map(|array| array)
            .collect::<Vec<Announcement>>();

        for announcement in announcements {
            self.recv_announcement(&announcement)
        }
    }
}
