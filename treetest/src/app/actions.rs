//! User-triggered TUI actions.
//!
//! These handlers intentionally stay thin: each one maps one keypress to one
//! simulator operation, then updates UI-local state such as the selected row and
//! status message. Keeping them small makes it easier to audit which user action
//! changed which part of the app state.

use super::{App, AppError, NodeId, Selection};

impl App {
    /// Performs protocol introspection for the current selection.
    ///
    /// Rationale: node and leaf introspection share one key because the protocol
    /// also shares one reserved procedure id for both operations.
    pub(super) fn perform_introspection(&mut self) -> Result<(), AppError> {
        match self.selected().clone() {
            Selection::Node(node_id) => {
                // Route the blank procedure to endpoint-wide introspection.
                let result = self.simulation.call_endpoint_introspection(node_id)?;
                // Drain immediately so the inspector reflects the learned state.
                let steps = self.simulation.drain()?;
                self.refresh_selections(Some(node_id));
                self.status = format!("{} ({steps} steps)", result.label);
            }
            Selection::Leaf { node_id, leaf_name } => {
                // Route the blank procedure to one specific leaf.
                let result = self
                    .simulation
                    .call_leaf_introspection(node_id, &leaf_name)?;
                let steps = self.simulation.drain()?;
                self.refresh_selections(Some(node_id));
                self.status = format!("{} ({steps} steps)", result.label);
            }
        }
        Ok(())
    }

    /// Calls the currently selected echo leaf.
    ///
    /// Rationale: the payload is fixed so the demo highlights packet flow rather
    /// than turning the TUI into a line editor.
    pub(super) fn perform_echo(&mut self) -> Result<(), AppError> {
        if let Selection::Leaf { node_id, leaf_name } = self.selected().clone() {
            let result =
                self.simulation
                    .call_echo_leaf(node_id, &leaf_name, "demo echo from root")?;
            let steps = self.simulation.drain()?;
            self.refresh_selections(Some(node_id));
            self.status = format!("{} ({steps} steps)", result.label);
        } else {
            self.status = "Select a leaf first, then press e.".to_owned();
        }
        Ok(())
    }

    /// Calls the first endpoint-level procedure on the selected node.
    pub(super) fn perform_ping(&mut self) -> Result<(), AppError> {
        if let Selection::Node(node_id) = self.selected().clone() {
            if let Some(procedure_id) = self
                .simulation
                .node(node_id)
                .endpoint_procedures
                .first()
                .map(|procedure| procedure.procedure_id.clone())
            {
                let result = self.simulation.call_endpoint_procedure(
                    node_id,
                    &procedure_id,
                    b"ping".to_vec(),
                )?;
                let steps = self.simulation.drain()?;
                self.refresh_selections(Some(node_id));
                self.status = format!("{} ({steps} steps)", result.label);
            } else {
                self.status = "Selected node has no endpoint procedures.".to_owned();
            }
        } else {
            self.status = "Select a node first, then press p.".to_owned();
        }
        Ok(())
    }

    /// Calls the chunked-response procedure on the selected node.
    pub(super) fn perform_chunked(&mut self) -> Result<(), AppError> {
        if let Selection::Node(node_id) = self.selected().clone() {
            if let Some(procedure_id) = self
                .simulation
                .node(node_id)
                .endpoint_procedures
                .iter()
                .find(|procedure| {
                    procedure.description.contains("chunk")
                        || procedure.procedure_id.contains("chunked")
                })
                .map(|procedure| procedure.procedure_id.clone())
            {
                let result = self.simulation.call_endpoint_procedure(
                    node_id,
                    &procedure_id,
                    b"chunk please".to_vec(),
                )?;
                let steps = self.simulation.drain()?;
                self.refresh_selections(Some(node_id));
                self.status = format!("{} ({steps} steps)", result.label);
            } else {
                self.status = "Selected node has no chunked procedure.".to_owned();
            }
        } else {
            self.status = "Select a node first, then press c.".to_owned();
        }
        Ok(())
    }

    /// Opens a long-lived chat hook on the selected node.
    pub(super) fn perform_chat_call(&mut self) -> Result<(), AppError> {
        if let Selection::Node(node_id) = self.selected().clone() {
            if let Some(procedure_id) = self
                .simulation
                .node(node_id)
                .endpoint_procedures
                .iter()
                .find(|procedure| procedure.procedure_id.contains("chat"))
                .map(|procedure| procedure.procedure_id.clone())
            {
                let result = self.simulation.call_endpoint_procedure(
                    node_id,
                    &procedure_id,
                    b"open chat".to_vec(),
                )?;
                let steps = self.simulation.drain()?;
                self.refresh_selections(Some(node_id));
                self.status = format!("{} ({steps} steps)", result.label);
            } else {
                self.status = "Selected node has no chat procedure.".to_owned();
            }
        } else {
            self.status = "Select a node first, then press h.".to_owned();
        }
        Ok(())
    }

    /// Sends follow-up data on the newest known hook.
    ///
    /// Rationale: using the latest hook keeps the demo simple while still
    /// exposing bidirectional hook behavior.
    pub(super) fn perform_chat_data(&mut self) -> Result<(), AppError> {
        if let Some(hook_id) = self.simulation.hook_ids().last().copied() {
            let result =
                self.simulation
                    .send_root_hook_data(hook_id, "hello from the root", false)?;
            let steps = self.simulation.drain()?;
            self.refresh_selections(None);
            self.status = format!("{} ({steps} steps)", result.label);
        } else {
            self.status = "No known hook yet. Press h to open chat first.".to_owned();
        }
        Ok(())
    }

    /// Ends the newest known chat hook from the root side.
    pub(super) fn perform_chat_bye(&mut self) -> Result<(), AppError> {
        if let Some(hook_id) = self.simulation.hook_ids().last().copied() {
            let result = self.simulation.send_root_hook_data(hook_id, "bye", true)?;
            let steps = self.simulation.drain()?;
            self.refresh_selections(None);
            self.status = format!("{} ({steps} steps)", result.label);
        } else {
            self.status = "No known hook yet. Press h to open chat first.".to_owned();
        }
        Ok(())
    }

    /// Injects intentionally invalid hook data to exercise fault handling.
    pub(super) fn perform_invalid_fault_demo(&mut self) -> Result<(), AppError> {
        if let Some(hook_id) = self.simulation.hook_ids().last().copied() {
            // The root is always node zero in every built-in scenario.
            let root_id = NodeId(0);
            if self.simulation.tree.nodes.len() > 1 {
                // The first child is enough to spoof a wrong peer path.
                let attacker = NodeId(1);
                let result = self.simulation.inject_invalid_peer_data(
                    attacker,
                    root_id,
                    hook_id,
                    "demo.endpoint.v1.chat.session",
                    "spoofed data",
                )?;
                let steps = self.simulation.drain()?;
                self.refresh_selections(None);
                self.status = format!("{} ({steps} steps)", result.label);
            } else {
                self.status =
                    "This scenario has no second node for invalid-peer traffic.".to_owned();
            }
        } else {
            self.status = "Open a hook first before injecting invalid traffic.".to_owned();
        }
        Ok(())
    }
}
