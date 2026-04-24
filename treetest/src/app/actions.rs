//! User-triggered TUI actions.

use super::{App, AppError, NodeId, Selection};

impl App {
    pub(super) fn perform_introspection(&mut self) -> Result<(), AppError> {
        match self.selected().clone() {
            Selection::Node(node_id) => {
                let result = self.simulation.call_endpoint_introspection(node_id)?;
                let steps = self.simulation.drain()?;
                self.refresh_selections(Some(node_id));
                self.status = format!("{} ({steps} steps)", result.label);
            }
            Selection::Leaf { node_id, leaf_name } => {
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

    pub(super) fn perform_invalid_fault_demo(&mut self) -> Result<(), AppError> {
        if let Some(hook_id) = self.simulation.hook_ids().last().copied() {
            let root_id = NodeId(0);
            if self.simulation.tree.nodes.len() > 1 {
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
