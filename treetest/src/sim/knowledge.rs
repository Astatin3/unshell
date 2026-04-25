//! Root-host knowledge tracking.
//!
//! The root inspector can either show full scenario truth or the smaller set of
//! facts a real host would have learned from direct configuration, introspection,
//! and observed traffic.

use std::collections::BTreeMap;

use unshell::protocol::{EndpointIntrospection, LeafIntrospection};

use crate::model::EndpointProcedureSpec;

/// Root inspector mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorMode {
    GroundTruth,
    Realistic,
}

/// Learned procedure metadata stored by the root host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LearnedProcedure {
    pub procedure_id: String,
    pub description: Option<String>,
}

/// Learned leaf metadata stored by the root host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LearnedLeaf {
    pub leaf_name: String,
    pub description: Option<String>,
    pub procedures: Vec<LearnedProcedure>,
}

/// Learned endpoint metadata stored by the root host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LearnedNode {
    pub path: Vec<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub direct_child: bool,
    pub endpoint_procedures: Vec<LearnedProcedure>,
    pub leaves: Vec<LearnedLeaf>,
    pub endpoint_introspected: bool,
}

/// Root-host knowledge accumulated from local configuration and observed traffic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootKnowledge {
    pub nodes: BTreeMap<Vec<String>, LearnedNode>,
}

impl RootKnowledge {
    /// Builds the initial root knowledge from static scenario truth.
    pub(super) fn new(tree: &crate::model::DemoTree) -> Self {
        let mut knowledge = Self {
            nodes: BTreeMap::new(),
        };
        for node in &tree.nodes {
            if node.path.is_empty() || node.path.len() == 1 {
                // Realistic mode intentionally starts with root plus direct children,
                // not the full transitive tree.
                let direct_child = node.path.len() == 1;
                let mut learned = LearnedNode {
                    path: node.path.clone(),
                    title: Some(node.title.clone()),
                    description: Some(node.description.clone()),
                    direct_child,
                    endpoint_procedures: Vec::new(),
                    leaves: Vec::new(),
                    endpoint_introspected: node.path.is_empty(),
                };

                if node.path.is_empty() {
                    // The root always knows its own procedures and leaves because
                    // those are locally configured, not discovered remotely.
                    learned.endpoint_procedures = node
                        .endpoint_procedures
                        .iter()
                        .map(|procedure| LearnedProcedure {
                            procedure_id: procedure.procedure_id.clone(),
                            description: Some(procedure.description.clone()),
                        })
                        .collect();
                    learned.leaves = node
                        .leaves
                        .iter()
                        .map(|leaf| LearnedLeaf {
                            leaf_name: leaf.name.clone(),
                            description: Some(leaf.description.clone()),
                            procedures: leaf
                                .procedures
                                .iter()
                                .map(|procedure_id| LearnedProcedure {
                                    procedure_id: procedure_id.clone(),
                                    description: Some(leaf.description.clone()),
                                })
                                .collect(),
                        })
                        .collect();
                }

                knowledge.nodes.insert(node.path.clone(), learned);
            }
        }
        knowledge
    }

    /// Returns an existing learned node or creates a new placeholder record.
    pub(super) fn ensure_node(&mut self, demo_node: &crate::model::DemoNode) -> &mut LearnedNode {
        let direct_child = demo_node.path.len() == 1;
        self.nodes
            .entry(demo_node.path.clone())
            .or_insert_with(|| LearnedNode {
                path: demo_node.path.clone(),
                title: Some(demo_node.title.clone()),
                description: Some(demo_node.description.clone()),
                direct_child,
                endpoint_procedures: Vec::new(),
                leaves: Vec::new(),
                endpoint_introspected: false,
            })
    }

    pub(super) fn remember_endpoint_procedure(
        &mut self,
        demo_node: &crate::model::DemoNode,
        procedure: &EndpointProcedureSpec,
    ) {
        // Procedures are keyed by full `procedure_id`, so repeated observation
        // simply enriches one existing record instead of duplicating it.
        let learned_node = self.ensure_node(demo_node);
        push_procedure(
            &mut learned_node.endpoint_procedures,
            procedure.procedure_id.clone(),
            Some(procedure.description.clone()),
        );
    }

    pub(super) fn remember_leaf_from_spec(
        &mut self,
        demo_node: &crate::model::DemoNode,
        leaf_spec: &crate::model::LeafSpec,
    ) {
        // Direct user targeting is enough for the root to remember a leaf exists,
        // even before remote introspection returns richer confirmation.
        let learned_node = self.ensure_node(demo_node);
        let leaf = ensure_leaf(
            &mut learned_node.leaves,
            leaf_spec.name.clone(),
            Some(leaf_spec.description.clone()),
        );
        for procedure_id in &leaf_spec.procedures {
            push_procedure(
                &mut leaf.procedures,
                procedure_id.clone(),
                Some(leaf_spec.description.clone()),
            );
        }
    }

    pub(super) fn remember_endpoint_introspection(
        &mut self,
        demo_node: &crate::model::DemoNode,
        introspection: &EndpointIntrospection,
    ) {
        // Endpoint introspection is the moment a node becomes explicitly known to
        // have been queried rather than merely inferred by path.
        let learned_node = self.ensure_node(demo_node);
        learned_node.endpoint_introspected = true;
        for summary in &introspection.leaves {
            let description = demo_node
                .leaves
                .iter()
                .find(|leaf| leaf.name == summary.leaf_name)
                .map(|leaf| leaf.description.clone());
            let leaf = ensure_leaf(
                &mut learned_node.leaves,
                summary.leaf_name.clone(),
                description,
            );
            for procedure_id in &summary.procedures {
                push_procedure(&mut leaf.procedures, procedure_id.clone(), None);
            }
        }
    }

    pub(super) fn remember_leaf_introspection(
        &mut self,
        demo_node: &crate::model::DemoNode,
        introspection: &LeafIntrospection,
    ) {
        let learned_node = self.ensure_node(demo_node);
        let description = demo_node
            .leaves
            .iter()
            .find(|leaf| leaf.name == introspection.leaf_name)
            .map(|leaf| leaf.description.clone());
        let leaf = ensure_leaf(
            &mut learned_node.leaves,
            introspection.leaf_name.clone(),
            description,
        );
        for procedure_id in &introspection.procedures {
            push_procedure(&mut leaf.procedures, procedure_id.clone(), None);
        }
    }

    pub(super) fn clear_deeper_than_one_hop(&mut self) {
        // This powers the realistic-mode reset, which forgets transitive state
        // and keeps only root-local plus direct-child knowledge.
        self.nodes.retain(|path, _| path.len() <= 1);
    }

    /// Returns one learned node by absolute path.
    pub fn node(&self, path: &[String]) -> Option<&LearnedNode> {
        self.nodes.get(path)
    }

    /// Returns every path currently known to the root host.
    pub fn known_paths(&self) -> Vec<Vec<String>> {
        self.nodes.keys().cloned().collect()
    }
}

/// Returns one learned leaf entry, creating it if necessary.
fn ensure_leaf<'a>(
    leaves: &'a mut Vec<LearnedLeaf>,
    leaf_name: String,
    description: Option<String>,
) -> &'a mut LearnedLeaf {
    if let Some(index) = leaves.iter().position(|leaf| leaf.leaf_name == leaf_name) {
        if leaves[index].description.is_none() {
            leaves[index].description = description;
        }
        return &mut leaves[index];
    }

    leaves.push(LearnedLeaf {
        leaf_name,
        description,
        procedures: Vec::new(),
    });
    leaves.last_mut().expect("just pushed")
}

/// Inserts or enriches one learned procedure entry.
fn push_procedure(
    procedures: &mut Vec<LearnedProcedure>,
    procedure_id: String,
    description: Option<String>,
) {
    if let Some(existing) = procedures
        .iter_mut()
        .find(|procedure| procedure.procedure_id == procedure_id)
    {
        // Preserve the first available description, then upgrade missing details
        // later if richer information is learned from introspection or config.
        if existing.description.is_none() {
            existing.description = description;
        }
        return;
    }
    procedures.push(LearnedProcedure {
        procedure_id,
        description,
    });
}
