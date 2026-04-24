//! Application-procedure handling layered over protocol calls.
//!
//! The core `unshell` runtime validates routing, hooks, and introspection, but
//! it intentionally does not know what a demo procedure should do. This module
//! is the thin application layer that turns validated local calls into concrete
//! demo behavior.

use unshell::protocol::{CallMessage, PacketHeader};

use crate::model::{EndpointProcedureKind, EndpointProcedureSpec, NodeId};

use super::super::super::types::{SimError, Simulation};

impl Simulation {
    /// Handles an application-visible `Call` that the protocol runtime already
    /// accepted and delivered locally.
    pub(super) fn handle_application_call(
        &mut self,
        node_id: NodeId,
        _header: &PacketHeader,
        message: &CallMessage,
    ) -> Result<(), SimError> {
        let Some(hook) = &message.response_hook else {
            return Ok(());
        };

        // Clone the procedure spec once so later reply generation can borrow the
        // rest of the simulator state freely.
        let procedure = self
            .lookup_endpoint_procedure(node_id, &message.procedure_id)?
            .clone();
        match procedure.kind {
            EndpointProcedureKind::Ping => {
                let reply = format!("pong from {}", self.node(node_id).display_path());
                let frame = self.nodes[node_id.0]
                    .endpoint
                    .make_data(
                        hook.return_path.clone(),
                        hook.hook_id,
                        procedure.procedure_id.clone(),
                        reply.clone().into_bytes(),
                        true,
                    )
                    .map_err(|error| SimError::Protocol(error.to_string()))?;
                self.record_trace(node_id, format!("endpoint sent ping reply: {reply}"));
                self.process_local_frame(node_id, frame)?;
            }
            EndpointProcedureKind::ChunkedGreeting => {
                for (index, text) in [
                    "chunk 1: hello from the endpoint",
                    "chunk 2: routing stayed path-based",
                    "chunk 3: hook complete",
                ]
                .iter()
                .enumerate()
                {
                    let frame = self.nodes[node_id.0]
                        .endpoint
                        .make_data(
                            hook.return_path.clone(),
                            hook.hook_id,
                            procedure.procedure_id.clone(),
                            text.as_bytes().to_vec(),
                            index == 2,
                        )
                        .map_err(|error| SimError::Protocol(error.to_string()))?;
                    self.record_trace(node_id, format!("endpoint sent chunk {}", index + 1));
                    self.process_local_frame(node_id, frame)?;
                }
            }
            EndpointProcedureKind::Chat => {
                // Persist chat state outside the protocol runtime because the
                // protocol itself does not define chat semantics.
                self.chat_sessions.insert(
                    hook.hook_id,
                    super::super::super::types::ChatSession {
                        node_id,
                        hook_id: hook.hook_id,
                        host_path: hook.return_path.clone(),
                        procedure_id: procedure.procedure_id.clone(),
                    },
                );
                let frame = self.nodes[node_id.0]
                    .endpoint
                    .make_data(
                        hook.return_path.clone(),
                        hook.hook_id,
                        procedure.procedure_id.clone(),
                        b"chat ready".to_vec(),
                        false,
                    )
                    .map_err(|error| SimError::Protocol(error.to_string()))?;
                self.record_trace(node_id, "chat handler opened session".to_owned());
                self.process_local_frame(node_id, frame)?;
            }
        }
        Ok(())
    }

    /// Resolves one endpoint procedure from the ground-truth node metadata.
    pub(super) fn lookup_endpoint_procedure(
        &self,
        node_id: NodeId,
        procedure_id: &str,
    ) -> Result<&EndpointProcedureSpec, SimError> {
        self.node(node_id)
            .endpoint_procedures
            .iter()
            .find(|procedure| procedure.procedure_id == procedure_id)
            .ok_or_else(|| SimError::UnknownProcedure {
                node_path: self.node(node_id).display_path(),
                procedure_id: procedure_id.to_owned(),
            })
    }

    /// Ensures one named leaf exists on the target node.
    pub(crate) fn require_leaf(
        &self,
        node_id: NodeId,
        leaf_name: &str,
    ) -> Result<&crate::model::LeafSpec, SimError> {
        self.node(node_id)
            .leaves
            .iter()
            .find(|leaf| leaf.name == leaf_name)
            .ok_or_else(|| SimError::UnknownLeaf {
                node_path: self.node(node_id).display_path(),
                leaf_name: leaf_name.to_owned(),
            })
    }

    /// Ensures one endpoint procedure exists on the target node.
    pub(crate) fn require_endpoint_procedure(
        &self,
        node_id: NodeId,
        procedure_id: &str,
    ) -> Result<(), SimError> {
        self.lookup_endpoint_procedure(node_id, procedure_id)
            .map(|_| ())
    }
}
