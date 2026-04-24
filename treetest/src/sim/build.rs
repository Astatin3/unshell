//! Construction and mode-management helpers for the simulator.
//!
//! These helpers are kept separate from runtime packet flow so scenario boot and
//! mode transitions remain easy to read and test in isolation.

use std::collections::{BTreeMap, VecDeque};

use crossbeam_channel::unbounded;
use unshell::protocol::tree::{ChildRoute, ConnectionState, LeafBehavior, ProtocolEndpoint};

use crate::model::{DemoTree, LeafKind, NodeId, ScenarioDefinition, Selection};

use super::knowledge::{InspectorMode, RootKnowledge};
use super::types::{ChatSession, SimError, SimNode, Simulation};

impl Simulation {
    /// Creates a fresh simulation from a scenario definition.
    ///
    /// # Example
    /// ```rust
    /// use treetest::{scenarios::built_in_scenarios, sim::Simulation};
    ///
    /// let scenario = built_in_scenarios().into_iter().next().unwrap();
    /// let simulation = Simulation::new(scenario).unwrap();
    /// assert_eq!(simulation.node(treetest::model::NodeId(0)).display_path(), "/");
    /// ```
    pub fn new(scenario: ScenarioDefinition) -> Result<Self, SimError> {
        // Flatten the recursive scenario description once so the rest of the
        // simulator can address nodes by stable ids.
        let tree = DemoTree::from_root(&scenario.root);
        let mut nodes = Vec::with_capacity(tree.nodes.len());

        for demo_node in &tree.nodes {
            // Each endpoint gets one mailbox pair. The simulator never opens a
            // real socket, so every hop is just channel delivery.
            let (tx, rx) = unbounded();

            // Materialize child routes up front so the protocol runtime can make
            // longest-prefix decisions without consulting the demo model again.
            let children = demo_node
                .children
                .iter()
                .map(|child_id| ChildRoute {
                    path: tree.node(*child_id).path.clone(),
                    state: ConnectionState::Registered,
                })
                .collect::<Vec<_>>();

            // Translate demo leaf metadata into protocol-runtime leaf specs.
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

            // Parents are stored by path because the protocol runtime reasons in
            // terms of endpoint paths rather than UI node ids.
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

            // Store the runtime endpoint alongside topology and mailbox state.
            nodes.push(SimNode {
                parent: demo_node.parent,
                children: demo_node.children.clone(),
                endpoint,
                tx,
                rx,
            });
        }

        // The root starts with only its own configuration plus direct-child
        // awareness, which realistic mode later uses as its initial knowledge.
        let root_knowledge = RootKnowledge::new(&tree);

        Ok(Self {
            scenario,
            tree,
            nodes,
            root_id: NodeId(0),
            // Tick counting starts at one so trace output reads naturally.
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
    ///
    /// Rationale: this mirrors a host that only retains locally configured and
    /// one-hop information until it learns more by introspection or traffic.
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
