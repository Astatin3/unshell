//! Root-issued calls and injected traffic.
//!
//! These helpers form the high-level API that both the TUI and the tests use.
//! Each helper prepares the right destination metadata, teaches the root what it
//! can know locally, and then hands one concrete call or data packet to the
//! runtime layer.

use crate::model::{NodeId, format_hook_ref, format_leaf_ref, format_path};
use unshell::protocol::{DataMessage, PacketHeader, PacketType};

use super::super::types::{ActionResult, SimError, Simulation};

impl Simulation {
    /// Builds and routes an endpoint introspection call from the root.
    ///
    /// # Example
    /// ```rust
    /// use treetest::{model::NodeId, scenarios::built_in_scenarios, sim::Simulation};
    ///
    /// let scenario = built_in_scenarios().into_iter().next().unwrap();
    /// let mut simulation = Simulation::new(scenario).unwrap();
    /// let result = simulation.call_endpoint_introspection(NodeId(0)).unwrap();
    /// assert!(result.label.contains("Inspect endpoint"));
    /// ```
    pub fn call_endpoint_introspection(
        &mut self,
        node_id: NodeId,
    ) -> Result<ActionResult, SimError> {
        // Snapshot the destination path now so the later packet build cannot be
        // confused by any further selection or scenario changes.
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

        // Fail fast if the selected leaf name is not valid in ground truth.
        self.require_leaf(node_id, leaf_name)?;

        let node = self.tree.node(node_id).clone();
        if let Some(leaf_spec) = node.leaves.iter().find(|leaf| leaf.name == leaf_name) {
            // The root already knows a leaf exists because the user targeted it
            // directly, even before the remote introspection result returns.
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

        // Clone the procedure list out before mutating learned state below.
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

        // Keep the public helper strict so ordinary UI actions cannot target a
        // non-existent endpoint procedure by mistake.
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

    /// Sends a raw call without demo-side validation.
    ///
    /// Rationale: tests need one escape hatch that can deliberately trigger
    /// remote `UnknownLeaf` and `UnknownProcedure` faults.
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
        // Fetch the peer path and procedure contract from the active hook model.
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

        // Build the packet by hand so the sender path can intentionally violate
        // the active hook's expected peer relationship.
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
}
