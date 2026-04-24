//! Introspection response generation.
//!
//! This code implements the reserved empty-procedure behavior from the
//! introspection sections of `PROTOCOL.md`.

use alloc::{string::String, vec};
use rkyv::{rancor::Error as RkyvError, to_bytes};

use crate::protocol::{
    DataMessage, EndpointIntrospection, FrameError, LeafIntrospection, LeafIntrospectionSummary,
    PacketHeader, PacketType, ProtocolFault, encode_packet,
};

use super::super::HookKey;
use super::core::{EndpointError, EndpointOutcome, ProtocolEndpoint};

impl ProtocolEndpoint {
    /// Handles the reserved introspection procedure.
    pub(crate) fn handle_introspection(
        &mut self,
        header: &PacketHeader,
        key: Option<HookKey>,
    ) -> Result<EndpointOutcome, EndpointError> {
        let Some(key) = key else {
            return Ok(EndpointOutcome {
                dropped: true,
                ..EndpointOutcome::default()
            });
        };

        self.hooks.activate_pending(&key, header.src_path.clone());

        let payload = if let Some(leaf_name) = &header.dst_leaf {
            let Some(leaf) = self.leaves.get(leaf_name) else {
                return self.emit_fault_if_possible(Some(key), ProtocolFault::UNKNOWN_LEAF);
            };
            to_bytes::<RkyvError>(&LeafIntrospection {
                leaf_name: leaf_name.clone(),
                procedures: leaf.procedures.clone(),
            })
            .map_err(|error| EndpointError::Frame(FrameError::Serialize(error)))?
            .to_vec()
        } else {
            to_bytes::<RkyvError>(&EndpointIntrospection {
                leaves: self
                    .leaves
                    .values()
                    .map(|leaf| LeafIntrospectionSummary {
                        leaf_name: leaf.name.clone(),
                        procedures: leaf.procedures.clone(),
                    })
                    .collect(),
            })
            .map_err(|error| EndpointError::Frame(FrameError::Serialize(error)))?
            .to_vec()
        };

        let response_header = PacketHeader {
            packet_type: PacketType::Data,
            src_path: self.path.clone(),
            dst_path: key.return_path.clone(),
            dst_leaf: None,
            hook_id: Some(key.hook_id),
        };
        let response = DataMessage {
            procedure_id: String::new(),
            data: payload,
            end_hook: true,
        };
        self.hooks.remove_active(&key);
        let route = self.decide_route(&key.return_path);

        match route {
            super::super::RouteDecision::Local => Ok(EndpointOutcome {
                events: vec![super::core::LocalEvent::Data {
                    header: response_header,
                    message: response,
                }],
                ..EndpointOutcome::default()
            }),
            _ => {
                let frame = encode_packet(&response_header, &response)?;
                Ok(EndpointOutcome {
                    forwards: vec![(route, frame)],
                    ..EndpointOutcome::default()
                })
            }
        }
    }
}
