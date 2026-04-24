//! Construction and mode-management helpers for the simulator.

use std::collections::{BTreeMap, VecDeque};

use crossbeam_channel::unbounded;
use unshell::protocol::tree::{ChildRoute, ConnectionState, LeafBehavior, ProtocolEndpoint};

use crate::model::{DemoTree, LeafKind, NodeId, ScenarioDefinition, Selection};

use super::knowledge::{InspectorMode, RootKnowledge};
use super::types::{ChatSession, SimError, SimNode, Simulation};

impl Simulation {
    /// Creates a fresh simulation from a scenario definition.
    pub fn new(scenario: ScenarioDefinition) -> Result<Self, SimError> {
        let tree = DemoTree::from_root(&scenario.root);
        let mut nodes = Vec::with_capacity(tree.nodes.len());

        for demo_node in &tree.nodes {
            let (tx, rx) = unbounded();
            let children = demo_node
                .children
                .iter()
                .map(|child_id| ChildRoute {
                    path: tree.node(*child_id).path.clone(),
                    state: ConnectionState::Registered,
                })
                .collect::<Vec<_>>();
            let leaves = demo_node
                .leaves
                .iter()
                .map(|leaf| unshell::protocol::tree::LeafSpec {
                    name: leaf.name.clone(),
                    procedures: leaf.procedures.clone(),
                    behavior: match leaf.kind {
                        LeafKind::Echo => LeafBehavior::Echo,
                    },
                })
                .collect::<Vec<_>>();
            let parent_path = demo_node
                .parent
                .map(|parent_id| tree.node(parent_id).path.clone());

            let mut endpoint =
                ProtocolEndpoint::new(demo_node.path.clone(), parent_path, children, leaves);
            for procedure in &demo_node.endpoint_procedures {
                endpoint
                    .add_endpoint_procedure(procedure.procedure_id.clone())
                    .map_err(|error| SimError::Protocol(error.to_string()))?;
            }

            nodes.push(SimNode {
                parent: demo_node.parent,
                children: demo_node.children.clone(),
                endpoint,
                tx,
                rx,
            });
        }

        let root_knowledge = RootKnowledge::new(&tree);

        Ok(Self {
            scenario,
            tree,
            nodes,
            root_id: NodeId(0),
            next_tick: 1,
            trace: VecDeque::new(),
            recorded_events: Vec::new(),
            hooks: BTreeMap::new(),
            inspector_mode: InspectorMode::GroundTruth,
            root_knowledge,
            chat_sessions: BTreeMap::<u64, ChatSession>::new(),
        })
    }

    /// Returns the scenario's initial selection.
    pub fn initial_selection(&self) -> Selection {
        self.scenario.initial_selection.clone()
    }

    /// Returns a node by id.
    pub fn node(&self, id: NodeId) -> &crate::model::DemoNode {
        self.tree.node(id)
    }

    /// Clears deeper root memory and switches the inspector into realistic mode.
    pub fn enable_realistic_mode_with_memory_reset(&mut self) {
        self.root_knowledge.clear_deeper_than_one_hop();
        self.inspector_mode = InspectorMode::Realistic;
    }

    /// Toggles the inspector between learned state and ground truth.
    pub fn toggle_inspector_mode(&mut self) {
        self.inspector_mode = match self.inspector_mode {
            InspectorMode::GroundTruth => InspectorMode::Realistic,
            InspectorMode::Realistic => InspectorMode::GroundTruth,
        };
    }

    /// Returns whether the inspector is using learned state.
    pub fn is_realistic_mode(&self) -> bool {
        self.inspector_mode == InspectorMode::Realistic
    }
}
