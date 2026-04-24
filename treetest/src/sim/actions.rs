//! Public action helpers exposed to the UI and tests.

use crossbeam_channel::TryRecvError;
use unshell::protocol::tree::Endpoint;
use unshell::protocol::{DataMessage, FaultMessage, PacketHeader, PacketType, decode_frame};

use crate::model::{NodeId, Selection, format_hook_ref, format_leaf_ref, format_path};

use super::types::{ActionResult, RecordedEvent, SimError, Simulation};

impl Simulation {
    /// Builds and routes an endpoint introspection call from the root.
    pub fn call_endpoint_introspection(
        &mut self,
        node_id: NodeId,
    ) -> Result<ActionResult, SimError> {
        let path = self.tree.node(node_id).path.clone();
        self.dispatch_root_call(path.clone(), None, "", Vec::new())?;
        Ok(ActionResult {
            label: format!("Inspect endpoint {}", format_path(&path)),
            hook_id: self.hooks.last_key_value().map(|(hook_id, _)| *hook_id),
        })
    }

    /// Builds and routes a leaf introspection call from the root.
    pub fn call_leaf_introspection(
        &mut self,
        node_id: NodeId,
        leaf_name: &str,
    ) -> Result<ActionResult, SimError> {
        let node_path = self.tree.node(node_id).path.clone();
        self.require_leaf(node_id, leaf_name)?;
        let node = self.tree.node(node_id).clone();
        if let Some(leaf_spec) = node.leaves.iter().find(|leaf| leaf.name == leaf_name) {
            self.root_knowledge
                .remember_leaf_from_spec(&node, leaf_spec);
        }
        self.dispatch_root_call(node_path, Some(leaf_name.to_owned()), "", Vec::new())?;
        Ok(ActionResult {
            label: format!(
                "Inspect {}",
                format_leaf_ref(&self.node(node_id).path, leaf_name)
            ),
            hook_id: self.hooks.last_key_value().map(|(hook_id, _)| *hook_id),
        })
    }

    /// Calls a leaf echo procedure using the selected payload.
    pub fn call_echo_leaf(
        &mut self,
        node_id: NodeId,
        leaf_name: &str,
        text: &str,
    ) -> Result<ActionResult, SimError> {
        let node_path = self.tree.node(node_id).path.clone();
        let node_display = self.tree.node(node_id).display_path();
        let node = self.tree.node(node_id).clone();
        let procedures = self.require_leaf(node_id, leaf_name)?.procedures.clone();
        if let Some(leaf_spec) = node
            .leaves
            .iter()
            .find(|known_leaf| known_leaf.name == leaf_name)
        {
            self.root_knowledge
                .remember_leaf_from_spec(&node, leaf_spec);
        }
        let procedure_id =
            procedures
                .first()
                .cloned()
                .ok_or_else(|| SimError::UnknownProcedure {
                    node_path: node_display.clone(),
                    procedure_id: "<missing>".to_owned(),
                })?;
        self.dispatch_root_call(
            node_path,
            Some(leaf_name.to_owned()),
            &procedure_id,
            text.as_bytes().to_vec(),
        )?;
        Ok(ActionResult {
            label: format!(
                "Echo via {}",
                format_leaf_ref(&self.node(node_id).path, leaf_name)
            ),
            hook_id: self.hooks.last_key_value().map(|(hook_id, _)| *hook_id),
        })
    }

    /// Calls an endpoint-level procedure.
    pub fn call_endpoint_procedure(
        &mut self,
        node_id: NodeId,
        procedure_id: &str,
        data: Vec<u8>,
    ) -> Result<ActionResult, SimError> {
        let node_path = self.tree.node(node_id).path.clone();
        let node_display = self.tree.node(node_id).display_path();
        self.require_endpoint_procedure(node_id, procedure_id)?;
        let node = self.tree.node(node_id).clone();
        if let Some(procedure) = node
            .endpoint_procedures
            .iter()
            .find(|known_procedure| known_procedure.procedure_id == procedure_id)
        {
            self.root_knowledge
                .remember_endpoint_procedure(&node, procedure);
        }
        self.dispatch_root_call(node_path, None, procedure_id, data)?;
        Ok(ActionResult {
            label: format!("Call {procedure_id} on {}", node_display),
            hook_id: self.hooks.last_key_value().map(|(hook_id, _)| *hook_id),
        })
    }

    /// Sends a raw call without demo-side validation so tests can exercise
    /// remote `UnknownLeaf` and `UnknownProcedure` fault behavior.
    pub fn call_unchecked(
        &mut self,
        node_id: NodeId,
        dst_leaf: Option<&str>,
        procedure_id: &str,
        data: Vec<u8>,
    ) -> Result<ActionResult, SimError> {
        let node_path = self.tree.node(node_id).path.clone();
        let node_display = self.tree.node(node_id).display_path();
        self.dispatch_root_call(node_path, dst_leaf.map(str::to_owned), procedure_id, data)?;
        Ok(ActionResult {
            label: format!(
                "Call {} on {}{}",
                if procedure_id.is_empty() {
                    "<introspection>"
                } else {
                    procedure_id
                },
                node_display,
                dst_leaf
                    .map(|leaf_name| format!(
                        " {}",
                        format_leaf_ref(&self.node(node_id).path, leaf_name)
                    ))
                    .unwrap_or_default()
            ),
            hook_id: self.hooks.last_key_value().map(|(hook_id, _)| *hook_id),
        })
    }

    /// Sends more hook data from the root side.
    pub fn send_root_hook_data(
        &mut self,
        hook_id: u64,
        text: &str,
        end_hook: bool,
    ) -> Result<ActionResult, SimError> {
        let snapshot = self
            .hooks
            .get(&hook_id)
            .cloned()
            .ok_or(SimError::UnknownHook(hook_id))?;
        let frame = self.nodes[self.root_id.0]
            .endpoint
            .make_data(
                snapshot.peer_path.clone(),
                hook_id,
                snapshot.procedure_id.clone(),
                text.as_bytes().to_vec(),
                end_hook,
            )
            .map_err(|error| SimError::Protocol(error.to_string()))?;
        self.record_trace(
            self.root_id,
            format!(
                "root queued hook data for {}: {text}",
                format_hook_ref(self.node(self.root_id).path.as_slice(), hook_id)
            ),
        );
        self.process_local_frame(self.root_id, frame)?;
        Ok(ActionResult {
            label: format!("Send hook data {hook_id}"),
            hook_id: Some(hook_id),
        })
    }

    /// Injects intentionally invalid traffic to demonstrate `InvalidHookPeer`.
    pub fn inject_invalid_peer_data(
        &mut self,
        from_node_id: NodeId,
        to_node_id: NodeId,
        hook_id: u64,
        procedure_id: &str,
        text: &str,
    ) -> Result<ActionResult, SimError> {
        let from_path = self.tree.node(from_node_id).path.clone();
        let to_path = self.tree.node(to_node_id).path.clone();
        let header = PacketHeader {
            packet_type: PacketType::Data,
            src_path: from_path.clone(),
            dst_path: to_path.clone(),
            dst_leaf: None,
            hook_id: Some(hook_id),
        };
        let message = DataMessage {
            procedure_id: procedure_id.to_owned(),
            data: text.as_bytes().to_vec(),
            end_hook: false,
        };
        let frame = unshell::protocol::encode_packet(&header, &message)
            .map_err(|error| SimError::Protocol(error.to_string()))?;

        self.record_trace(
            from_node_id,
            format!(
                "injected invalid peer data toward {} for {}",
                format_path(&to_path),
                format_hook_ref(self.node(to_node_id).path.as_slice(), hook_id)
            ),
        );
        self.process_local_frame(from_node_id, frame)?;
        Ok(ActionResult {
            label: format!(
                "Inject invalid peer data for {}",
                format_hook_ref(self.node(to_node_id).path.as_slice(), hook_id)
            ),
            hook_id: Some(hook_id),
        })
    }

    /// Processes one queued frame if available.
    pub fn step(&mut self) -> Result<bool, SimError> {
        for node_id in 0..self.nodes.len() {
            match self.nodes[node_id].rx.try_recv() {
                Ok(envelope) => {
                    self.record_trace(
                        NodeId(node_id),
                        format!("received frame via {:?}", envelope.ingress),
                    );
                    let outcome = self.nodes[node_id]
                        .endpoint
                        .receive(&envelope.ingress, envelope.frame)
                        .map_err(|error| SimError::Protocol(error.to_string()))?;
                    self.process_outcome(NodeId(node_id), outcome)?;
                    return Ok(true);
                }
                Err(TryRecvError::Disconnected) => {
                    return Err(SimError::Protocol("mailbox disconnected".to_owned()));
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        Ok(false)
    }

    /// Runs frames until the network becomes idle.
    pub fn drain(&mut self) -> Result<usize, SimError> {
        let mut steps = 0;
        while self.step()? {
            steps += 1;
        }
        Ok(steps)
    }

    /// Returns a compact description of a frame for debugging.
    pub fn describe_frame(frame: &[u8]) -> String {
        match decode_frame(frame) {
            Ok(parsed) => {
                let header = parsed.header();
                format!(
                    "{:?} {} -> {} hook {:?}",
                    header.packet_type,
                    format_path(&header.src_path),
                    format_path(&header.dst_path),
                    header.hook_id,
                )
            }
            Err(error) => format!("<invalid frame: {error}>"),
        }
    }

    /// Returns the latest fault observed at the root, if any.
    pub fn latest_root_fault(&self) -> Option<&FaultMessage> {
        self.recorded_events
            .iter()
            .rev()
            .find_map(|event| match event {
                RecordedEvent::Fault {
                    node_path, message, ..
                } if node_path == "/" => Some(message),
                _ => None,
            })
    }

    /// Returns the latest root data message as utf-8 for tests and status text.
    pub fn latest_root_data_text(&self) -> Option<String> {
        self.recorded_events
            .iter()
            .rev()
            .find_map(|event| match event {
                RecordedEvent::Data {
                    node_path, message, ..
                } if node_path == "/" => Some(String::from_utf8_lossy(&message.data).to_string()),
                _ => None,
            })
    }

    /// Returns all hook ids known to the demo in ascending order.
    pub fn hook_ids(&self) -> Vec<u64> {
        self.hooks.keys().copied().collect()
    }

    /// Builds a human-readable description of the current selection.
    pub fn selection_summary(&self, selection: &Selection) -> String {
        match selection {
            Selection::Node(node_id) => {
                let node = self.node(*node_id);
                format!("{}: {}", node.display_path(), node.title)
            }
            Selection::Leaf { node_id, leaf_name } => {
                format_leaf_ref(&self.node(*node_id).path, leaf_name)
            }
        }
    }
}
