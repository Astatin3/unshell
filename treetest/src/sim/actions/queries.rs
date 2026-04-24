//! Read-only simulator queries used by tests and UI widgets.

use crate::model::Selection;

use unshell::protocol::FaultMessage;

use super::super::types::{RecordedEvent, Simulation};

impl Simulation {
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
                crate::model::format_leaf_ref(&self.node(*node_id).path, leaf_name)
            }
        }
    }
}
