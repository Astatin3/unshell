use alloc::vec::Vec;

use crossbeam_channel::{Receiver, Sender};

use crate::protocol::{Endpoint, Leaf, Packet};

#[cfg(feature = "interface")]
use crate::protocol::LeafMeta;

use super::super::constants::LEAF_MOCK_CONNECTION;

/// Leaf that simulates a serialized transport connection with crossbeam channels.
///
/// This is intentionally tiny and reusable. Both endpoints in the Merkle test have
/// exactly one of these leaves, giving the requested four-leaf topology: caller,
/// respondent, and two mock connections.
pub(crate) struct MockConnectionLeaf {
    pub(crate) tx: Sender<Vec<u8>>,
    pub(crate) rx: Receiver<Vec<u8>>,
    pub(crate) remote_id: u32,
    pub(crate) is_authority: bool,
    pub(crate) started: bool,
}

impl MockConnectionLeaf {
    /// Creates one side of a mock connection.
    pub(crate) fn new(
        tx: Sender<Vec<u8>>,
        rx: Receiver<Vec<u8>>,
        remote_id: u32,
        is_authority: bool,
    ) -> Self {
        Self {
            tx,
            rx,
            remote_id,
            is_authority,
            started: false,
        }
    }
}

impl Leaf for MockConnectionLeaf {
    fn get_id(&self) -> u32 {
        LEAF_MOCK_CONNECTION
    }

    #[cfg(feature = "interface")]
    fn get_meta(&self) -> LeafMeta {
        LeafMeta {
            name: "Merke Connection Leaf",
            identifier: "dev.unshell.test.merkle.connection",
            version: "v0",
            authors: alloc::vec!["ASTATIN3"],
        }
    }

    fn update(&mut self, endpoint: &mut Endpoint) {
        if !self.started {
            endpoint.add_connection(self.remote_id, self.is_authority);
            self.started = true;
        }

        while !self.rx.is_empty() {
            let data = self.rx.recv().unwrap();

            // Mock transports move untrusted bytes. Malformed frames are dropped so
            // the sync state machine is tested only after packet parsing succeeds.
            if let Ok(packet) = Packet::deserialize(&data) {
                let _ = endpoint.add_inbound_from(self.remote_id, packet);
            }
        }

        endpoint.take_outbound_clear(self.remote_id, |packet| {
            let data = packet.serialize().unwrap();
            let _ = self.tx.send(data);
        });
    }
}
