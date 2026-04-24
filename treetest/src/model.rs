//! Static tree and scenario metadata used by the simulator and UI.
//!
//! The protocol runtime already owns routing and hook validation state. This
//! module adds a second, UI-friendly model so the demo can keep titles,
//! descriptions, selection ids, and behavior metadata without polluting the core
//! protocol implementation.

use std::collections::BTreeMap;

/// Stable identifier for a node in a demo tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub usize);

/// Supported demo leaf kinds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeafKind {
    /// Uses the built-in echo leaf behavior from `unshell`.
    Echo,
}

/// Static leaf declaration used to build a protocol endpoint and describe it in the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeafSpec {
    pub name: String,
    pub description: String,
    pub kind: LeafKind,
    pub procedures: Vec<String>,
}

/// Demo-only endpoint procedure behaviors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndpointProcedureKind {
    /// Single response that completes the hook immediately.
    Ping,
    /// Multi-packet response used to demonstrate chunking and finalization.
    ChunkedGreeting,
    /// Bidirectional hook that remains active until one side sends `bye`.
    Chat,
}

/// Static endpoint procedure definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointProcedureSpec {
    pub procedure_id: String,
    pub description: String,
    pub kind: EndpointProcedureKind,
}

/// Recursive scenario node specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeSpec {
    /// Empty for the root endpoint.
    pub segment: String,
    pub title: String,
    pub description: String,
    pub leaves: Vec<LeafSpec>,
    pub endpoint_procedures: Vec<EndpointProcedureSpec>,
    pub children: Vec<NodeSpec>,
}

/// Concrete node metadata used after scenario construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoNode {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub path: Vec<String>,
    pub title: String,
    pub description: String,
    pub leaves: Vec<LeafSpec>,
    pub endpoint_procedures: Vec<EndpointProcedureSpec>,
}

impl DemoNode {
    /// Returns a display path that keeps the root easy to recognize in the UI.
    pub fn display_path(&self) -> String {
        format_path(&self.path)
    }
}

/// Fully flattened tree metadata used by the simulator and UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoTree {
    pub root: NodeId,
    pub nodes: Vec<DemoNode>,
    path_index: BTreeMap<Vec<String>, NodeId>,
}

impl DemoTree {
    /// Builds a flattened tree from a recursive specification.
    pub fn from_root(spec: &NodeSpec) -> Self {
        let mut nodes = Vec::new();
        let mut path_index = BTreeMap::new();
        let root = Self::push_node(spec, None, &[], &mut nodes, &mut path_index);
        Self {
            root,
            nodes,
            path_index,
        }
    }

    fn push_node(
        spec: &NodeSpec,
        parent: Option<NodeId>,
        base_path: &[String],
        nodes: &mut Vec<DemoNode>,
        path_index: &mut BTreeMap<Vec<String>, NodeId>,
    ) -> NodeId {
        let id = NodeId(nodes.len());
        let path = if spec.segment.is_empty() {
            base_path.to_vec()
        } else {
            let mut next = base_path.to_vec();
            next.push(spec.segment.clone());
            next
        };

        nodes.push(DemoNode {
            id,
            parent,
            children: Vec::new(),
            path: path.clone(),
            title: spec.title.clone(),
            description: spec.description.clone(),
            leaves: spec.leaves.clone(),
            endpoint_procedures: spec.endpoint_procedures.clone(),
        });
        path_index.insert(path.clone(), id);

        let child_ids = spec
            .children
            .iter()
            .map(|child| Self::push_node(child, Some(id), &path, nodes, path_index))
            .collect::<Vec<_>>();
        nodes[id.0].children = child_ids;
        id
    }

    /// Returns the node with the given id.
    pub fn node(&self, id: NodeId) -> &DemoNode {
        &self.nodes[id.0]
    }

    /// Resolves an absolute path to a node id.
    pub fn find_by_path(&self, path: &[String]) -> Option<NodeId> {
        self.path_index.get(path).copied()
    }
}

/// Root-focused interaction target shown in the inspector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Node(NodeId),
    Leaf { node_id: NodeId, leaf_name: String },
}

impl Selection {
    /// Returns the owning node of this selection.
    pub fn node_id(&self) -> NodeId {
        match self {
            Self::Node(node_id) => *node_id,
            Self::Leaf { node_id, .. } => *node_id,
        }
    }
}

/// User-facing scenario definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioDefinition {
    pub name: String,
    pub description: String,
    pub highlights: Vec<String>,
    pub root: NodeSpec,
    pub initial_selection: Selection,
}

/// Formats a path the same way throughout the UI and tests.
pub fn format_path(path: &[String]) -> String {
    if path.is_empty() {
        "/".to_owned()
    } else {
        format!("/{}", path.join("/"))
    }
}
