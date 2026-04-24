//! Internal packet routing and local event handling.
//!
//! This module is where the simulated transport meets the real protocol
//! endpoint runtime. It keeps forwarding logic, local delivery, and root
//! knowledge learning separate from the user-facing action helpers.

use unshell::protocol::tree::{Endpoint, Ingress, LocalEvent, RouteDecision};
use unshell::protocol::{
    CallMessage, DataMessage, FrameBytes, PacketHeader, deserialize_archived_bytes,
};

use crate::model::{
    EndpointProcedureKind, EndpointProcedureSpec, NodeId, format_hook_ref, format_leaf_ref,
    format_path,
};

use super::types::{Envelope, HookSnapshot, RecordedEvent, SimError, Simulation};

impl Simulation {
    pub(super) fn dispatch_root_call(
        &mut self,
        dst_path: Vec<String>,
        dst_leaf: Option<String>,
        procedure_id: &str,
        data: Vec<u8>,
    ) -> Result<(), SimError> {
        let hook_id = self.nodes[self.root_id.0].endpoint.allocate_hook_id();
        let frame = self.nodes[self.root_id.0]
            .endpoint
            .make_call(
                dst_path.clone(),
                dst_leaf.clone(),
                procedure_id.to_owned(),
                Some(hook_id),
                data,
            )
            .map_err(|error| SimError::Protocol(error.to_string()))?;
        self.hooks.insert(
            hook_id,
            HookSnapshot {
                hook_id,
                host_path: Vec::new(),
                peer_path: dst_path.clone(),
                procedure_id: procedure_id.to_owned(),
                target_leaf: dst_leaf.clone(),
                closed: false,
                last_message: format!("created for {}", format_path(&dst_path)),
            },
        );
        self.record_trace(
            self.root_id,
            format!(
                "root queued Call {} toward {}{}",
                if procedure_id.is_empty() {
                    "<introspection>"
                } else {
                    procedure_id
                },
                format_path(&dst_path),
                dst_leaf
                    .as_ref()
                    .map(|leaf| format!(" {}", format_leaf_ref(&dst_path, leaf)))
                    .unwrap_or_default()
            ),
        );
        self.process_local_frame(self.root_id, frame)
    }

    pub(super) fn process_local_frame(
        &mut self,
        node_id: NodeId,
        frame: FrameBytes,
    ) -> Result<(), SimError> {
        let outcome = self.nodes[node_id.0]
            .endpoint
            .receive(&Ingress::Local, frame)
            .map_err(|error| SimError::Protocol(error.to_string()))?;
        self.process_outcome(node_id, outcome)
    }

    pub(super) fn process_outcome(
        &mut self,
        node_id: NodeId,
        outcome: unshell::protocol::tree::EndpointOutcome,
    ) -> Result<(), SimError> {
        if outcome.dropped {
            self.record_trace(node_id, "packet dropped".to_owned());
        }

        for (route, frame) in outcome.forwards {
            match route {
                RouteDecision::Child(index) => {
                    let child_id = self.nodes[node_id.0]
                        .children
                        .get(index)
                        .copied()
                        .ok_or_else(|| {
                            SimError::Protocol(format!("missing child index {index}"))
                        })?;
                    self.record_trace(
                        node_id,
                        format!(
                            "forwarded frame to child {}",
                            self.node(child_id).display_path()
                        ),
                    );
                    self.nodes[child_id.0]
                        .tx
                        .send(Envelope {
                            ingress: Ingress::Parent,
                            frame,
                        })
                        .map_err(|error| SimError::Protocol(error.to_string()))?;
                }
                RouteDecision::Parent => {
                    let parent_id = self.nodes[node_id.0]
                        .parent
                        .ok_or_else(|| SimError::Protocol("missing parent route".to_owned()))?;
                    let child_path = self.node(node_id).path.clone();
                    self.record_trace(
                        node_id,
                        format!(
                            "forwarded frame to parent {}",
                            self.node(parent_id).display_path()
                        ),
                    );
                    self.nodes[parent_id.0]
                        .tx
                        .send(Envelope {
                            ingress: Ingress::Child(child_path),
                            frame,
                        })
                        .map_err(|error| SimError::Protocol(error.to_string()))?;
                }
                RouteDecision::Local => {
                    return Err(SimError::Protocol(
                        "local route leaked into forward list".to_owned(),
                    ));
                }
                RouteDecision::Drop => {
                    self.record_trace(node_id, "route decision dropped frame".to_owned());
                }
            }
        }

        for event in outcome.events {
            self.handle_local_event(node_id, event)?;
        }

        Ok(())
    }

    fn handle_local_event(&mut self, node_id: NodeId, event: LocalEvent) -> Result<(), SimError> {
        let node_path = self.node(node_id).display_path();
        match event {
            LocalEvent::Data { header, message } => {
                let text = String::from_utf8_lossy(&message.data).to_string();
                self.record_trace(
                    node_id,
                    format!(
                        "local Data on {}: {text}",
                        format_hook_ref(
                            self.node(node_id).path.as_slice(),
                            header.hook_id.unwrap_or(0)
                        )
                    ),
                );
                if let Some(hook_id) = header.hook_id {
                    if let Some(snapshot) = self.hooks.get_mut(&hook_id) {
                        snapshot.last_message = if text.is_empty() {
                            format!("binary payload ({} bytes)", message.data.len())
                        } else {
                            text.clone()
                        };
                        if message.end_hook {
                            snapshot.closed = true;
                        }
                    }

                    if node_id == self.root_id {
                        self.learn_from_root_data(hook_id, &message);
                    }
                }

                if let Some(session) = self
                    .chat_sessions
                    .get(&header.hook_id.unwrap_or(0))
                    .cloned()
                    .filter(|session| session.node_id == node_id)
                {
                    // Rationale: chat responses are implemented here instead of in the
                    // core endpoint so the protocol crate stays generic. The simulator
                    // acts as the application layer sitting above validated hook traffic.
                    let reply = if text.eq_ignore_ascii_case("bye") {
                        Some(("chat session closed".to_owned(), true))
                    } else if !text.is_empty() {
                        Some((format!("chat ack: {}", text.to_uppercase()), false))
                    } else {
                        None
                    };

                    if let Some((reply, end_hook)) = reply {
                        let frame = self.nodes[session.node_id.0]
                            .endpoint
                            .make_data(
                                session.host_path.clone(),
                                session.hook_id,
                                session.procedure_id.clone(),
                                reply.clone().into_bytes(),
                                end_hook,
                            )
                            .map_err(|error| SimError::Protocol(error.to_string()))?;
                        self.record_trace(session.node_id, format!("chat handler sent: {reply}"));
                        self.process_local_frame(session.node_id, frame)?;
                        if end_hook {
                            self.chat_sessions.remove(&session.hook_id);
                        }
                    }
                }

                self.recorded_events.push(RecordedEvent::Data {
                    node_path,
                    header,
                    message,
                });
            }
            LocalEvent::Fault { header, message } => {
                self.record_trace(
                    node_id,
                    format!(
                        "local Fault on {}: 0x{:02X}",
                        format_hook_ref(
                            self.node(node_id).path.as_slice(),
                            header.hook_id.unwrap_or(0)
                        ),
                        message.fault.0
                    ),
                );
                if let Some(hook_id) = header.hook_id {
                    if let Some(snapshot) = self.hooks.get_mut(&hook_id) {
                        snapshot.closed = true;
                        snapshot.last_message = format!("fault 0x{:02X}", message.fault.0);
                    }
                    self.chat_sessions.remove(&hook_id);
                }
                self.recorded_events.push(RecordedEvent::Fault {
                    node_path,
                    header,
                    message,
                });
            }
            LocalEvent::Call { header, message } => {
                self.record_trace(
                    node_id,
                    format!(
                        "local Call {} on {}",
                        message.procedure_id,
                        header
                            .dst_leaf
                            .as_ref()
                            .map(|leaf| format_leaf_ref(&header.dst_path, leaf))
                            .unwrap_or_else(|| "endpoint".to_owned())
                    ),
                );
                self.handle_application_call(node_id, &header, &message)?;
                self.recorded_events.push(RecordedEvent::Call {
                    node_path,
                    header,
                    message,
                });
            }
        }
        Ok(())
    }

    fn handle_application_call(
        &mut self,
        node_id: NodeId,
        _header: &PacketHeader,
        message: &CallMessage,
    ) -> Result<(), SimError> {
        let Some(hook) = &message.response_hook else {
            return Ok(());
        };

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
                self.chat_sessions.insert(
                    hook.hook_id,
                    super::types::ChatSession {
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

    fn lookup_endpoint_procedure(
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

    pub(super) fn require_leaf(
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

    pub(super) fn require_endpoint_procedure(
        &self,
        node_id: NodeId,
        procedure_id: &str,
    ) -> Result<(), SimError> {
        self.lookup_endpoint_procedure(node_id, procedure_id)
            .map(|_| ())
    }

    pub(super) fn record_trace(&mut self, node_id: NodeId, summary: String) {
        let node_path = self.node(node_id).display_path();
        self.trace.push_back(super::types::TraceEvent {
            tick: self.next_tick,
            node_path,
            summary,
        });
        self.next_tick += 1;
        while self.trace.len() > 200 {
            self.trace.pop_front();
        }
    }

    fn learn_from_root_data(&mut self, hook_id: u64, message: &DataMessage) {
        let Some(snapshot) = self.hooks.get(&hook_id).cloned() else {
            return;
        };
        let Some(node_id) = self.tree.find_by_path(&snapshot.peer_path) else {
            return;
        };
        let demo_node = self.node(node_id).clone();

        if snapshot.procedure_id.is_empty() {
            if snapshot.target_leaf.is_some() {
                if let Ok(introspection) = deserialize_archived_bytes::<
                    unshell::protocol::introspection::ArchivedLeafIntrospection,
                    unshell::protocol::LeafIntrospection,
                >(&message.data)
                {
                    self.root_knowledge
                        .remember_leaf_introspection(&demo_node, &introspection);
                }
            } else if let Ok(introspection) = deserialize_archived_bytes::<
                unshell::protocol::introspection::ArchivedEndpointIntrospection,
                unshell::protocol::EndpointIntrospection,
            >(&message.data)
            {
                self.root_knowledge
                    .remember_endpoint_introspection(&demo_node, &introspection);
            }
            return;
        }

        if let Some(procedure) = demo_node
            .endpoint_procedures
            .iter()
            .find(|procedure| procedure.procedure_id == snapshot.procedure_id)
        {
            self.root_knowledge
                .remember_endpoint_procedure(&demo_node, procedure);
        }

        if let Some(leaf_name) = &snapshot.target_leaf
            && let Some(leaf_spec) = demo_node.leaves.iter().find(|leaf| &leaf.name == leaf_name)
        {
            self.root_knowledge
                .remember_leaf_from_spec(&demo_node, leaf_spec);
        }
    }
}
