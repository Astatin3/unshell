use alloc::vec::Vec;

use crossbeam_channel::{Receiver, Sender};

use crate::protocol::{Endpoint, Leaf, Packet};

#[cfg(feature = "interface")]
use crate::protocol::LeafMeta;

const LEAF_COMMS: u32 = 101;

/// Mock transport leaf that serializes outbound packets through a channel pair.
///
/// This is intentionally shared by protocol integration tests: it is the boundary
/// where structured packets become untrusted bytes and malformed frames get dropped
/// before reaching endpoint routing.
pub(crate) struct CommsLeaf {
    pub(crate) tx: Sender<Vec<u8>>,
    pub(crate) rx: Receiver<Vec<u8>>,

    pub(crate) remote_id: u32,
    pub(crate) is_authority: bool,
    pub(crate) started: bool,
}

impl Leaf for CommsLeaf {
    fn get_id(&self) -> u32 {
        LEAF_COMMS
    }

    #[cfg(feature = "interface")]
    fn get_meta(&self) -> LeafMeta {
        LeafMeta {
            name: "Comms Leaf",
            identifier: "dev.unshell.test.comms_leaf",
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

            // Transport bytes are untrusted. Dropping malformed frames here keeps
            // integration harnesses faithful to a router boundary: invalid wire data
            // must not panic or poison later valid packets on the same connection.
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
