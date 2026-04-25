//! Introspection response generation.

use alloc::string::String;
use rkyv::{rancor::Error as RkyvError, to_bytes};

use crate::protocol::{
    DataMessage, EndpointIntrospection, FrameError, LeafIntrospection, LeafIntrospectionSummary,
    PacketHeader, PacketType, ProtocolFault, encode_packet,
};

use super::super::HookKey;
use super::core::{EndpointError, EndpointOutcome, ProtocolEndpoint};

impl ProtocolEndpoint {
    pub(crate) fn handle_introspection(
        &mut self,
        header: &PacketHeader,
        key: Option<HookKey>,
    ) -> Result<EndpointOutcome, EndpointError> {
        let Some(key) = key else {
            return Ok(EndpointOutcome::dropped());
        };

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
                sub_endpoints: self
                    .children
                    .iter()
                    .filter(|child| child.state == super::core::ConnectionState::Registered)
                    .filter_map(|child| child.path.get(self.path.len()).cloned())
                    .collect(),
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

        if self.hooks.mark_local_end(&key) {
            self.hooks.remove_active(&key);
        }

        match self.decide_route(&key.return_path) {
            super::super::RouteDecision::Local => Ok(EndpointOutcome::event(
                super::core::LocalEvent::Data {
                    header: response_header,
                    message: response,
                },
            )),
            route => Ok(EndpointOutcome::forward(route, encode_packet(&response_header, &response)?)),
        }
    }
}
