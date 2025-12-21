use unshell_lib::{Announcement, Result};

use crate::network::Stream;

use crate::Manager;

impl Manager {
    pub fn add_connection(&mut self, connection: Box<dyn Stream<Announcement>>) {
        self.connections.push(connection);
    }

    pub fn prune_connections(&mut self) {
        self.connections.retain(|c| c.is_alive());
    }

    pub fn recv_connection_announcements(&mut self) {
        // Collect all incoming announcements
        let announcements = self
            .connections
            .iter_mut()
            .map(|c| c.try_read())
            .flat_map(|array| array)
            .collect::<Vec<Announcement>>();

        for announcement in announcements {
            self.recv_announcement(&announcement)
        }
    }

    pub fn broadcast(&mut self, announcement: Announcement) -> Result<()> {
        for connection in &mut self.connections {
            connection.write(announcement.clone())?;
        }
        Ok(())
    }
}
