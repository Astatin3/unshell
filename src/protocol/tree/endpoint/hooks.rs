//! Hook-state transitions and route helpers.
//!
//! These methods implement the hook lifecycle described in `PROTOCOL.md`:
//! pending contexts, active contexts, peer validation, and fault emission.

use alloc::string::String;

use crate::protocol::{
    DataMessage, FaultMessage, PacketHeader, PacketType, ProtocolFault, encode_packet,
};

use super::super::{HookKey, RouteDecision, route_destination};
use super::core::{EndpointError, EndpointOutcome, Ingress, LocalEvent, ProtocolEndpoint};

impl ProtocolEndpoint {
    /// Emits a protocol fault only when the original call declared a response hook.
    pub(crate) fn emit_fault_if_possible(
        &mut self,
        key: Option<HookKey>,
        fault: ProtocolFault,
    ) -> Result<EndpointOutcome, EndpointError> {
        let Some(key) = key else {
            return Ok(EndpointOutcome::dropped());
        };

        self.hooks.remove_pending(&key);
        self.hooks.remove_active(&key);

        let header = PacketHeader {
            packet_type: PacketType::Fault,
            src_path: self.path.clone(),
            dst_path: key.return_path.clone(),
            dst_leaf: None,
            hook_id: Some(key.hook_id),
        };
        let message = FaultMessage { fault };
        let route = self.decide_route(&key.return_path);

        match route {
            RouteDecision::Local => Ok(EndpointOutcome::event(LocalEvent::Fault { header, message })),
            _ => {
                let frame = encode_packet(&header, &message)?;
                Ok(EndpointOutcome::forward(route, frame))
            }
        }
    }

    /// Handles locally delivered hook `Data` packets.
    pub(crate) fn handle_local_data(
        &mut self,
        header: PacketHeader,
        message: DataMessage,
    ) -> Result<EndpointOutcome, EndpointError> {
        let hook_id = header.hook_id.expect("validated");
        // The hook host can address its hook directly with `self.path + hook_id`.
        // A non-host peer only knows the hook id it was given earlier, so it must
        // recover the host-scoped key from active state using its validated path.
        let key = self
            .hooks
            .active(&HookKey::new(self.path.clone(), hook_id))
            .map(|_| HookKey::new(self.path.clone(), hook_id))
            .or_else(|| {
                self.hooks
                    .find_active_key_by_peer(hook_id, &header.src_path)
            })
            .unwrap_or_else(|| HookKey::new(self.path.clone(), hook_id));

        if self.hooks.active(&key).is_none() {
            let matches = self.hooks.pending(&key).is_some_and(|pending| {
                pending.caller_src_path == header.src_path
                    && pending.procedure_id == message.procedure_id
            });
            if matches {
                self.hooks.activate_pending(&key, header.src_path.clone());
            }
        }

        let Some(active) = self.hooks.active(&key) else {
            return Ok(EndpointOutcome::dropped());
        };

        if active.peer_path != header.src_path {
            self.hooks.remove_active(&key);
            self.hooks.remove_pending(&key);
            return Ok(EndpointOutcome::event(LocalEvent::Fault {
                header: PacketHeader {
                    packet_type: PacketType::Fault,
                    src_path: header.src_path,
                    dst_path: self.path.clone(),
                    dst_leaf: None,
                    hook_id: Some(key.hook_id),
                },
                message: FaultMessage {
                    fault: ProtocolFault::INVALID_HOOK_PEER,
                },
            }));
        }

        if active.procedure_id != message.procedure_id {
            return Ok(EndpointOutcome::dropped());
        }

        if message.end_hook {
            self.hooks.remove_active(&key);
        }

        Ok(EndpointOutcome::event(LocalEvent::Data { header, message }))
    }

    /// Handles locally delivered hook `Fault` packets.
    pub(crate) fn handle_local_fault(
        &mut self,
        header: PacketHeader,
        message: FaultMessage,
    ) -> Result<EndpointOutcome, EndpointError> {
        let key = HookKey::new(self.path.clone(), header.hook_id.expect("validated"));
        let matches = self
            .hooks
            .active(&key)
            .is_some_and(|active| active.peer_path == header.src_path)
            || self
                .hooks
                .pending(&key)
                .is_some_and(|pending| pending.caller_src_path == header.src_path);

        if !matches {
            return Ok(EndpointOutcome::dropped());
        }

        self.hooks.remove_active(&key);
        self.hooks.remove_pending(&key);

        Ok(EndpointOutcome::event(LocalEvent::Fault { header, message }))
    }

    /// Chooses the next hop using the protocol's longest-prefix routing rule.
    pub(crate) fn decide_route(&self, dst_path: &[String]) -> RouteDecision {
        let child_paths = self
            .children
            .iter()
            .filter(|child| child.state == super::core::ConnectionState::Registered)
            .map(|child| &child.path);

        route_destination(
            &self.path,
            child_paths,
            self.parent_path.is_some(),
            dst_path,
        )
    }

    /// Validates whether a source path is attributable to the ingress side.
    ///
    /// Rationale: this looks backwards at first because parent ingress accepts
    /// non-local source paths. That is required for multi-hop routing, where a
    /// parent forwards traffic originating from ancestors or siblings.
    pub(crate) fn valid_source_for_ingress(&self, ingress: &Ingress, src_path: &[String]) -> bool {
        match ingress {
            Ingress::Parent => {
                if src_path.len() < self.path.len() {
                    return true;
                }
                if src_path.len() == self.path.len() {
                    return src_path == self.path;
                }
                !src_path.starts_with(&self.path)
            }
            Ingress::Child(child_path) => src_path.starts_with(child_path),
            Ingress::Local => src_path == self.path,
        }
    }
}
