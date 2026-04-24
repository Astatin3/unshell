//! Root-side knowledge learning from returned data.

use unshell::protocol::{
    DataMessage, EndpointIntrospection, LeafIntrospection, deserialize_archived_bytes,
};

use super::super::types::Simulation;

impl Simulation {
    pub(crate) fn learn_from_root_data(&mut self, hook_id: u64, message: &DataMessage) {
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
                    LeafIntrospection,
                >(&message.data)
                {
                    self.root_knowledge
                        .remember_leaf_introspection(&demo_node, &introspection);
                }
            } else if let Ok(introspection) = deserialize_archived_bytes::<
                unshell::protocol::introspection::ArchivedEndpointIntrospection,
                EndpointIntrospection,
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
