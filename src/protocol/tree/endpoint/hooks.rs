//! Hook-state transitions and route helpers.

use alloc::string::String;

use crate::protocol::{
    DataMessage, FaultMessage, PacketHeader, PacketType, ProtocolFault, encode_packet,
};

use super::super::{HookKey, RouteDecision};
use super::core::{EndpointError, EndpointOutcome, Ingress, LocalEvent, ProtocolEndpoint};

impl ProtocolEndpoint {
    pub(crate) fn emit_fault_if_possible(
        &mut self,
        key: Option<HookKey>,
        fault: ProtocolFault,
    ) -> Result<EndpointOutcome, EndpointError> {
        let Some(key) = key else {
            return Ok(EndpointOutcome::Dropped);
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

        match self.decide_route(&key.return_path) {
            RouteDecision::Local => Ok(EndpointOutcome::Local(LocalEvent::Fault {
                header,
                message,
                hook_key: key,
            })),
            route => Ok(EndpointOutcome::Forward {
                route,
                frame: encode_packet(&header, &message)?,
            }),
        }
    }

    pub(crate) fn handle_local_data(
        &mut self,
        header: PacketHeader,
        message: DataMessage,
    ) -> Result<EndpointOutcome, EndpointError> {
        let hook_id = header.hook_id.expect("validated");
        let key = if let Some(key) =
            self.hooks
                .resolve_active_key(&self.path, hook_id, &header.src_path)
        {
            key
        } else {
            let pending_key = HookKey::new(self.path.clone(), hook_id);
            if self.hooks.pending(&pending_key).is_some_and(|pending| {
                pending.caller_src_path == header.src_path
                    && pending.procedure_id == message.procedure_id
            }) {
                self.hooks.activate_pending(&pending_key);
                pending_key
            } else {
                return Ok(EndpointOutcome::Dropped);
            }
        };

        let Some(active) = self.hooks.active(&key) else {
            return Ok(EndpointOutcome::Dropped);
        };

        if active.peer_path != header.src_path {
            // A reused hook id from the wrong peer is treated as terminal for this hook,
            // because the endpoint can no longer trust future traffic on it.
            self.hooks.remove_active(&key);
            return self.emit_fault_if_possible(Some(key), ProtocolFault::INVALID_HOOK_PEER);
        }

        if active.procedure_id != message.procedure_id {
            // Data frames stay bound to the procedure chosen by the original call.
            return Ok(EndpointOutcome::Dropped);
        }

        if message.end_hook && self.hooks.mark_peer_end(&key) {
            self.hooks.remove_active(&key);
        }

        Ok(EndpointOutcome::Local(LocalEvent::Data {
            header,
            message,
            hook_key: key,
        }))
    }

    pub(crate) fn handle_local_fault(
        &mut self,
        header: PacketHeader,
        message: FaultMessage,
    ) -> Result<EndpointOutcome, EndpointError> {
        let hook_id = header.hook_id.expect("validated");
        if let Some(key) = self
            .hooks
            .resolve_active_key(&self.path, hook_id, &header.src_path)
        {
            self.hooks.remove_active(&key);
            return Ok(EndpointOutcome::Local(LocalEvent::Fault {
                header,
                message,
                hook_key: key,
            }));
        }

        let pending_key = HookKey::new(self.path.clone(), hook_id);
        if self
            .hooks
            .pending(&pending_key)
            .is_some_and(|pending| pending.caller_src_path == header.src_path)
        {
            self.hooks.remove_pending(&pending_key);
            return Ok(EndpointOutcome::Local(LocalEvent::Fault {
                header,
                message,
                hook_key: pending_key,
            }));
        }

        Ok(EndpointOutcome::Dropped)
    }

    pub(crate) fn decide_route(&self, dst_path: &[String]) -> RouteDecision {
        self.routing.route(dst_path)
    }

    pub(crate) fn valid_source_for_ingress(&self, ingress: &Ingress, src_path: &[String]) -> bool {
        match ingress {
            Ingress::Parent => {
                // Parent ingress may carry packets from ancestors, siblings, or the endpoint
                // itself, but not from descendants pretending to be upstream.
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
