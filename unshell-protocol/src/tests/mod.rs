use crate::{endpoint::EndpointRef, leaf::Leaf, packet::Packet};

use alloc::{
    collections::vec_deque::VecDeque,
    format,
    string::{String, ToString},
    vec::Vec,
};
use crossbeam_channel::{Receiver, Sender};

struct ControllerLeaf {
    responder_id: String,
    has_run: bool,
}
struct CommsLeaf {
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,

    remote_id: String,
    is_authority: bool,
    started: bool,
}
struct ResponderLeaf;

impl Leaf for ControllerLeaf {
    fn get_name(&self) -> &'static str {
        "ControllerLeaf"
    }

    fn update<'a>(&mut self, endpoint: &mut EndpointRef<'a>) {
        if !self.has_run {
            endpoint.add_outbound(
                self.responder_id.clone(),
                Packet {
                    hook_id: 0,
                    is_upwards_call: false,
                    end_hook: false,
                    path: format!("/{}", self.responder_id),
                    procedure_id: "echo".to_string(),
                    data: "ABC123".as_bytes().to_vec(),
                },
            );

            self.has_run = true;
        }
    }
}

impl Leaf for CommsLeaf {
    fn get_name(&self) -> &'static str {
        "CommsLeaf"
    }

    fn update<'a>(&mut self, endpoint: &mut EndpointRef<'a>) {
        if !self.started {
            endpoint
                .connections
                .insert((self.remote_id.clone(), self.is_authority));
        }

        while !self.rx.is_empty() {
            let packet = Packet::deserialize(&self.rx.recv().unwrap()).unwrap();

            endpoint.add_inbound(packet).unwrap();
        }

        endpoint.take_outbound(self.get_name(), |packet| {
            let data = packet.serialize().unwrap();
            self.tx.send(data).unwrap();
        });
    }
}

impl Leaf for ResponderLeaf {
    fn get_name(&self) -> &'static str {
        "ResponderLeaf"
    }

    fn update<'a>(&mut self, endpoint: &mut EndpointRef<'a>) {
        let packets = endpoint
            .inbound
            .get(self.get_name())
            .unwrap_or(&VecDeque::new())
            .iter()
            .map(|packet| {
                // let data = ;

                Packet {
                    hook_id: 0,
                    is_upwards_call: false,
                    end_hook: false,
                    path: String::new(),
                    // path: packet.path.clone(),
                    procedure_id: "echo".to_string(),
                    data: packet.data.clone(),
                }
            })
            .collect::<Vec<Packet>>();

        for packet in packets {
            endpoint.add_outbound(packet);
        }
    }
}

#[test]
fn test_comms() {}
