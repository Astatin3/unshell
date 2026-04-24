//! Packet dispatch and routing glue.
//!
//! This layer sits directly above the protocol endpoint runtime. It converts one
//! local action into framed traffic, hands that traffic to the endpoint, and then
//! forwards the resulting frames across the simulated tree.

use unshell::protocol::FrameBytes;
use unshell::protocol::tree::{Endpoint, Ingress, RouteDecision};

use crate::model::{NodeId, format_leaf_ref, format_path};

use super::super::types::{Envelope, HookSnapshot, SimError, Simulation, TraceEvent};

impl Simulation {
    /// Builds a root-originated `Call` and feeds it into the runtime.
    pub(crate) fn dispatch_root_call(
        &mut self,
        dst_path: Vec<String>,
        dst_leaf: Option<String>,
        procedure_id: &str,
        data: Vec<u8>,
    ) -> Result<(), SimError> {
        // Hook allocation happens on the root host because the root is the hook
        // owner for every user-driven action in the demo.
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

        // Track enough metadata locally to render hooks and learn from returned
        // data later, even before the first response arrives.
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

    /// Delivers a frame into one endpoint as locally-originated traffic.
    pub(crate) fn process_local_frame(
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

    /// Applies one endpoint outcome to the simulated transport.
    pub(crate) fn process_outcome(
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

                    // Parent ingress needs the child path because the protocol
                    // runtime validates source-path claims against ingress side.
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

    /// Appends one entry to the rolling trace buffer.
    pub(crate) fn record_trace(&mut self, node_id: NodeId, summary: String) {
        let node_path = self.node(node_id).display_path();
        self.trace.push_back(TraceEvent {
            tick: self.next_tick,
            node_path,
            summary,
        });
        self.next_tick += 1;

        // Cap trace growth so the TUI remains responsive during long sessions.
        while self.trace.len() > 200 {
            self.trace.pop_front();
        }
    }
}
