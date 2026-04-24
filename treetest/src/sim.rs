//! Crossbeam-backed protocol simulation.
//!
//! The simulator never opens real sockets. Each endpoint gets a mailbox, and
//! forwarded frames are pushed into the next hop's queue. That makes routing and
//! hook behavior deterministic enough for tests while still feeling like traffic.

use std::collections::{BTreeMap, VecDeque};

use crossbeam_channel::{Receiver, Sender, TryRecvError, unbounded};
use thiserror::Error;
use unshell::protocol::tree::{
    ChildRoute, ConnectionState, Endpoint, Ingress, LeafBehavior, LocalEvent, ProtocolEndpoint,
};
use unshell::protocol::{
    CallMessage, DataMessage, EndpointIntrospection, FaultMessage, FrameBytes, LeafIntrospection,
    PacketHeader, PacketType, decode_frame, deserialize_archived_bytes,
};

use crate::model::{
    DemoTree, EndpointProcedureKind, EndpointProcedureSpec, LeafKind, NodeId, ScenarioDefinition,
    Selection, format_hook_ref, format_leaf_ref, format_path,
};

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

/// User-facing outcome of a root-originated action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionResult {
    pub label: String,
    pub hook_id: Option<u64>,
}

/// Snapshot of a hook interaction observed by the demo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookSnapshot {
    pub hook_id: u64,
    pub host_path: Vec<String>,
    pub peer_path: Vec<String>,
    pub procedure_id: String,
    pub target_leaf: Option<String>,
    pub closed: bool,
    pub last_message: String,
}

/// Trace entry shown in the UI and asserted in tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEvent {
    pub tick: u64,
    pub node_path: String,
    pub summary: String,
}

/// Summary of one local protocol event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordedEvent {
    Data {
        node_path: String,
        header: PacketHeader,
        message: DataMessage,
    },
    Fault {
        node_path: String,
        header: PacketHeader,
        message: FaultMessage,
    },
    Call {
        node_path: String,
        header: PacketHeader,
        message: CallMessage,
    },
}

#[derive(Debug)]
struct SimNode {
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    endpoint: ProtocolEndpoint,
    tx: Sender<Envelope>,
    rx: Receiver<Envelope>,
}

#[derive(Debug, Clone)]
struct Envelope {
    ingress: Ingress,
    frame: FrameBytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChatSession {
    node_id: NodeId,
    hook_id: u64,
    host_path: Vec<String>,
    procedure_id: String,
}

/// Errors raised by the demo simulator.
#[derive(Debug, Error)]
pub enum SimError {
    #[error("node {0} was not found")]
    UnknownNode(String),
    #[error("leaf {leaf_name} was not found on {node_path}")]
    UnknownLeaf {
        node_path: String,
        leaf_name: String,
    },
    #[error("procedure {procedure_id} was not found on {node_path}")]
    UnknownProcedure {
        node_path: String,
        procedure_id: String,
    },
    #[error("hook {0} was not found")]
    UnknownHook(u64),
    #[error("protocol runtime error: {0}")]
    Protocol(String),
}

/// Fully built simulation for one scenario.
#[derive(Debug)]
pub struct Simulation {
    pub scenario: ScenarioDefinition,
    pub tree: DemoTree,
    nodes: Vec<SimNode>,
    root_id: NodeId,
    next_tick: u64,
    pub trace: VecDeque<TraceEvent>,
    pub recorded_events: Vec<RecordedEvent>,
    pub hooks: BTreeMap<u64, HookSnapshot>,
    pub inspector_mode: InspectorMode,
    pub root_knowledge: RootKnowledge,
    chat_sessions: BTreeMap<u64, ChatSession>,
}

impl RootKnowledge {
    fn new(tree: &DemoTree) -> Self {
        let mut knowledge = Self {
            nodes: BTreeMap::new(),
        };
        for node in &tree.nodes {
            if node.path.is_empty() || node.path.len() == 1 {
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

    fn ensure_node(&mut self, demo_node: &crate::model::DemoNode) -> &mut LearnedNode {
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

    fn remember_endpoint_procedure(
        &mut self,
        demo_node: &crate::model::DemoNode,
        procedure: &EndpointProcedureSpec,
    ) {
        let learned_node = self.ensure_node(demo_node);
        push_procedure(
            &mut learned_node.endpoint_procedures,
            procedure.procedure_id.clone(),
            Some(procedure.description.clone()),
        );
    }

    fn remember_leaf_from_spec(
        &mut self,
        demo_node: &crate::model::DemoNode,
        leaf_spec: &crate::model::LeafSpec,
    ) {
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

    fn remember_endpoint_introspection(
        &mut self,
        demo_node: &crate::model::DemoNode,
        introspection: &EndpointIntrospection,
    ) {
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

    fn remember_leaf_introspection(
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

    fn clear_deeper_than_one_hop(&mut self) {
        self.nodes.retain(|path, _| path.len() <= 1);
    }

    pub fn node(&self, path: &[String]) -> Option<&LearnedNode> {
        self.nodes.get(path)
    }

    pub fn known_paths(&self) -> Vec<Vec<String>> {
        self.nodes.keys().cloned().collect()
    }
}

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

fn push_procedure(
    procedures: &mut Vec<LearnedProcedure>,
    procedure_id: String,
    description: Option<String>,
) {
    if let Some(existing) = procedures
        .iter_mut()
        .find(|procedure| procedure.procedure_id == procedure_id)
    {
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
            chat_sessions: BTreeMap::new(),
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
            label: format!("Send hook data #{hook_id}"),
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

    fn dispatch_root_call(
        &mut self,
        dst_path: Vec<String>,
        dst_leaf: Option<String>,
        procedure_id: &str,
        data: Vec<u8>,
    ) -> Result<(), SimError> {
        let hook_id = self.nodes[self.root_id.0].endpoint.allocate_hook_id();
        let frame = self.nodes[self.root_id.0]
            .endpoint
            .make_call(
                dst_path.clone(),
                dst_leaf.clone(),
                procedure_id.to_owned(),
                Some(hook_id),
                data,
            )
            .map_err(|error| SimError::Protocol(error.to_string()))?;
        self.hooks.insert(
            hook_id,
            HookSnapshot {
                hook_id,
                host_path: Vec::new(),
                peer_path: dst_path.clone(),
                procedure_id: procedure_id.to_owned(),
                target_leaf: dst_leaf.clone(),
                closed: false,
                last_message: format!("created for {}", format_path(&dst_path)),
            },
        );
        self.record_trace(
            self.root_id,
            format!(
                "root queued Call {} toward {}{}",
                if procedure_id.is_empty() {
                    "<introspection>"
                } else {
                    procedure_id
                },
                format_path(&dst_path),
                dst_leaf
                    .as_ref()
                    .map(|leaf| format!(" {}", format_leaf_ref(&dst_path, leaf)))
                    .unwrap_or_default()
            ),
        );
        self.process_local_frame(self.root_id, frame)
    }

    fn process_local_frame(&mut self, node_id: NodeId, frame: FrameBytes) -> Result<(), SimError> {
        let outcome = self.nodes[node_id.0]
            .endpoint
            .receive(&Ingress::Local, frame)
            .map_err(|error| SimError::Protocol(error.to_string()))?;
        self.process_outcome(node_id, outcome)
    }

    fn process_outcome(
        &mut self,
        node_id: NodeId,
        outcome: unshell::protocol::tree::EndpointOutcome,
    ) -> Result<(), SimError> {
        if outcome.dropped {
            self.record_trace(node_id, "packet dropped".to_owned());
        }

        for (route, frame) in outcome.forwards {
            match route {
                unshell::protocol::tree::RouteDecision::Child(index) => {
                    let child_id = self.nodes[node_id.0]
                        .children
                        .get(index)
                        .copied()
                        .ok_or_else(|| {
                            SimError::Protocol(format!("missing child index {index}"))
                        })?;
                    self.record_trace(
                        node_id,
                        format!(
                            "forwarded frame to child {}",
                            self.node(child_id).display_path()
                        ),
                    );
                    self.nodes[child_id.0]
                        .tx
                        .send(Envelope {
                            ingress: Ingress::Parent,
                            frame,
                        })
                        .map_err(|error| SimError::Protocol(error.to_string()))?;
                }
                unshell::protocol::tree::RouteDecision::Parent => {
                    let parent_id = self.nodes[node_id.0]
                        .parent
                        .ok_or_else(|| SimError::Protocol("missing parent route".to_owned()))?;
                    let child_path = self.node(node_id).path.clone();
                    self.record_trace(
                        node_id,
                        format!(
                            "forwarded frame to parent {}",
                            self.node(parent_id).display_path()
                        ),
                    );
                    self.nodes[parent_id.0]
                        .tx
                        .send(Envelope {
                            ingress: Ingress::Child(child_path),
                            frame,
                        })
                        .map_err(|error| SimError::Protocol(error.to_string()))?;
                }
                unshell::protocol::tree::RouteDecision::Local => {
                    return Err(SimError::Protocol(
                        "local route leaked into forward list".to_owned(),
                    ));
                }
                unshell::protocol::tree::RouteDecision::Drop => {
                    self.record_trace(node_id, "route decision dropped frame".to_owned());
                }
            }
        }

        for event in outcome.events {
            self.handle_local_event(node_id, event)?;
        }

        Ok(())
    }

    fn handle_local_event(&mut self, node_id: NodeId, event: LocalEvent) -> Result<(), SimError> {
        let node_path = self.node(node_id).display_path();
        match event {
            LocalEvent::Data { header, message } => {
                let text = String::from_utf8_lossy(&message.data).to_string();
                self.record_trace(
                    node_id,
                    format!(
                        "local Data on {}: {text}",
                        format_hook_ref(
                            self.node(node_id).path.as_slice(),
                            header.hook_id.unwrap_or(0)
                        )
                    ),
                );
                if let Some(hook_id) = header.hook_id {
                    if let Some(snapshot) = self.hooks.get_mut(&hook_id) {
                        snapshot.last_message = if text.is_empty() {
                            format!("binary payload ({} bytes)", message.data.len())
                        } else {
                            text.clone()
                        };
                        if message.end_hook {
                            snapshot.closed = true;
                        }
                    }

                    if node_id == self.root_id {
                        self.learn_from_root_data(hook_id, &message);
                    }
                }

                if let Some(session) = self
                    .chat_sessions
                    .get(&header.hook_id.unwrap_or(0))
                    .cloned()
                    .filter(|session| session.node_id == node_id)
                {
                    // Rationale: chat responses are implemented here instead of in the
                    // core endpoint so the protocol crate stays generic. The simulator
                    // acts as the application layer sitting above validated hook traffic.
                    let reply = if text.eq_ignore_ascii_case("bye") {
                        Some(("chat session closed".to_owned(), true))
                    } else if !text.is_empty() {
                        Some((format!("chat ack: {}", text.to_uppercase()), false))
                    } else {
                        None
                    };

                    if let Some((reply, end_hook)) = reply {
                        let frame = self.nodes[session.node_id.0]
                            .endpoint
                            .make_data(
                                session.host_path.clone(),
                                session.hook_id,
                                session.procedure_id.clone(),
                                reply.clone().into_bytes(),
                                end_hook,
                            )
                            .map_err(|error| SimError::Protocol(error.to_string()))?;
                        self.record_trace(session.node_id, format!("chat handler sent: {reply}"));
                        self.process_local_frame(session.node_id, frame)?;
                        if end_hook {
                            self.chat_sessions.remove(&session.hook_id);
                        }
                    }
                }

                self.recorded_events.push(RecordedEvent::Data {
                    node_path,
                    header,
                    message,
                });
            }
            LocalEvent::Fault { header, message } => {
                self.record_trace(
                    node_id,
                    format!(
                        "local Fault on {}: 0x{:02X}",
                        format_hook_ref(
                            self.node(node_id).path.as_slice(),
                            header.hook_id.unwrap_or(0)
                        ),
                        message.fault.0
                    ),
                );
                if let Some(hook_id) = header.hook_id {
                    if let Some(snapshot) = self.hooks.get_mut(&hook_id) {
                        snapshot.closed = true;
                        snapshot.last_message = format!("fault 0x{:02X}", message.fault.0);
                    }
                    self.chat_sessions.remove(&hook_id);
                }
                self.recorded_events.push(RecordedEvent::Fault {
                    node_path,
                    header,
                    message,
                });
            }
            LocalEvent::Call { header, message } => {
                self.record_trace(
                    node_id,
                    format!(
                        "local Call {} on {}",
                        message.procedure_id,
                        header
                            .dst_leaf
                            .as_ref()
                            .map(|leaf| format_leaf_ref(&header.dst_path, leaf))
                            .unwrap_or_else(|| "endpoint".to_owned())
                    ),
                );
                self.handle_application_call(node_id, &header, &message)?;
                self.recorded_events.push(RecordedEvent::Call {
                    node_path,
                    header,
                    message,
                });
            }
        }
        Ok(())
    }

    fn handle_application_call(
        &mut self,
        node_id: NodeId,
        _header: &PacketHeader,
        message: &CallMessage,
    ) -> Result<(), SimError> {
        let Some(hook) = &message.response_hook else {
            return Ok(());
        };

        let procedure = self
            .lookup_endpoint_procedure(node_id, &message.procedure_id)?
            .clone();
        match procedure.kind {
            EndpointProcedureKind::Ping => {
                let reply = format!("pong from {}", self.node(node_id).display_path());
                let frame = self.nodes[node_id.0]
                    .endpoint
                    .make_data(
                        hook.return_path.clone(),
                        hook.hook_id,
                        procedure.procedure_id.clone(),
                        reply.clone().into_bytes(),
                        true,
                    )
                    .map_err(|error| SimError::Protocol(error.to_string()))?;
                self.record_trace(node_id, format!("endpoint sent ping reply: {reply}"));
                self.process_local_frame(node_id, frame)?;
            }
            EndpointProcedureKind::ChunkedGreeting => {
                for (index, text) in [
                    "chunk 1: hello from the endpoint",
                    "chunk 2: routing stayed path-based",
                    "chunk 3: hook complete",
                ]
                .iter()
                .enumerate()
                {
                    let frame = self.nodes[node_id.0]
                        .endpoint
                        .make_data(
                            hook.return_path.clone(),
                            hook.hook_id,
                            procedure.procedure_id.clone(),
                            text.as_bytes().to_vec(),
                            index == 2,
                        )
                        .map_err(|error| SimError::Protocol(error.to_string()))?;
                    self.record_trace(node_id, format!("endpoint sent chunk {}", index + 1));
                    self.process_local_frame(node_id, frame)?;
                }
            }
            EndpointProcedureKind::Chat => {
                self.chat_sessions.insert(
                    hook.hook_id,
                    ChatSession {
                        node_id,
                        hook_id: hook.hook_id,
                        host_path: hook.return_path.clone(),
                        procedure_id: procedure.procedure_id.clone(),
                    },
                );
                let frame = self.nodes[node_id.0]
                    .endpoint
                    .make_data(
                        hook.return_path.clone(),
                        hook.hook_id,
                        procedure.procedure_id.clone(),
                        b"chat ready".to_vec(),
                        false,
                    )
                    .map_err(|error| SimError::Protocol(error.to_string()))?;
                self.record_trace(node_id, "chat handler opened session".to_owned());
                self.process_local_frame(node_id, frame)?;
            }
        }
        Ok(())
    }

    fn lookup_endpoint_procedure(
        &self,
        node_id: NodeId,
        procedure_id: &str,
    ) -> Result<&EndpointProcedureSpec, SimError> {
        self.node(node_id)
            .endpoint_procedures
            .iter()
            .find(|procedure| procedure.procedure_id == procedure_id)
            .ok_or_else(|| SimError::UnknownProcedure {
                node_path: self.node(node_id).display_path(),
                procedure_id: procedure_id.to_owned(),
            })
    }

    fn require_leaf(
        &self,
        node_id: NodeId,
        leaf_name: &str,
    ) -> Result<&crate::model::LeafSpec, SimError> {
        self.node(node_id)
            .leaves
            .iter()
            .find(|leaf| leaf.name == leaf_name)
            .ok_or_else(|| SimError::UnknownLeaf {
                node_path: self.node(node_id).display_path(),
                leaf_name: leaf_name.to_owned(),
            })
    }

    fn require_endpoint_procedure(
        &self,
        node_id: NodeId,
        procedure_id: &str,
    ) -> Result<(), SimError> {
        self.lookup_endpoint_procedure(node_id, procedure_id)
            .map(|_| ())
    }

    fn record_trace(&mut self, node_id: NodeId, summary: String) {
        let node_path = self.node(node_id).display_path();
        self.trace.push_back(TraceEvent {
            tick: self.next_tick,
            node_path,
            summary,
        });
        self.next_tick += 1;
        while self.trace.len() > 200 {
            self.trace.pop_front();
        }
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

    fn learn_from_root_data(&mut self, hook_id: u64, message: &DataMessage) {
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
