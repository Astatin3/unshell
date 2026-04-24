//! Protocol local-event handling.
//!
//! These handlers translate validated protocol events into trace entries,
//! learned root knowledge, and higher-level demo state such as the chat helper.

use unshell::protocol::tree::LocalEvent;

use crate::model::{NodeId, format_hook_ref, format_leaf_ref};

use super::super::super::types::{RecordedEvent, SimError, Simulation};

impl Simulation {
    /// Handles one local event emitted by the protocol runtime.
    pub(crate) fn handle_local_event(
        &mut self,
        node_id: NodeId,
        event: LocalEvent,
    ) -> Result<(), SimError> {
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
                        // Keep the most recent human-readable payload in the UI.
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
                    // Any protocol fault ends the application-level chat too.
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
}
