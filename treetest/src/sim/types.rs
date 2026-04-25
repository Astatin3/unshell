//! Core simulator data types.
//!
//! This module intentionally contains only durable state and event structures.
//! Behavior lives in sibling modules so readers can scan data layout without
//! jumping through packet-processing logic.

use std::collections::{BTreeMap, VecDeque};

use crossbeam_channel::{Receiver, Sender};
use thiserror::Error;
use unshell::protocol::tree::{Ingress, ProtocolEndpoint};
use unshell::protocol::{CallMessage, DataMessage, FaultMessage, FrameBytes, PacketHeader};

use crate::model::{DemoTree, NodeId, ScenarioDefinition};

use super::knowledge::{InspectorMode, RootKnowledge};

/// User-facing outcome of a root-originated action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionResult {
    /// Human-readable summary shown in the footer after one action completes.
    pub label: String,
    /// Hook id allocated for the action, if the action opened or used one.
    pub hook_id: Option<u64>,
}

/// Snapshot of a hook interaction observed by the demo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookSnapshot {
    /// Hook identifier scoped to the root host.
    pub hook_id: u64,
    /// Host path for the hook, usually the root in this demo.
    pub host_path: Vec<String>,
    /// Peer endpoint currently associated with the hook.
    pub peer_path: Vec<String>,
    /// Procedure contract that established the hook.
    pub procedure_id: String,
    /// Optional target leaf when the originating call addressed one leaf.
    pub target_leaf: Option<String>,
    /// Whether the hook has finished normally or faulted.
    pub closed: bool,
    /// Most recent human-readable payload summary for the UI.
    pub last_message: String,
}

/// Trace entry shown in the UI and asserted in tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEvent {
    /// Monotonic event number assigned by the simulator.
    pub tick: u64,
    /// Display path of the node that emitted the trace line.
    pub node_path: String,
    /// Human-readable event summary.
    pub summary: String,
}

/// Summary of one local protocol event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordedEvent {
    /// Local hook data event.
    Data {
        node_path: String,
        header: PacketHeader,
        message: DataMessage,
    },
    /// Local protocol fault event.
    Fault {
        node_path: String,
        header: PacketHeader,
        message: FaultMessage,
    },
    /// Local call-delivery event.
    Call {
        node_path: String,
        header: PacketHeader,
        message: CallMessage,
    },
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
    /// Active scenario definition the simulation was built from.
    pub scenario: ScenarioDefinition,
    /// Flattened tree model used by both simulator and UI.
    pub tree: DemoTree,
    pub(super) nodes: Vec<SimNode>,
    pub(super) root_id: NodeId,
    pub(super) next_tick: u64,
    /// Rolling trace buffer shown in the UI.
    pub trace: VecDeque<TraceEvent>,
    /// Exact local events emitted by the protocol runtime.
    pub recorded_events: Vec<RecordedEvent>,
    /// Live and historical hook snapshots for display.
    pub hooks: BTreeMap<u64, HookSnapshot>,
    /// Which knowledge view the inspector currently renders.
    pub inspector_mode: InspectorMode,
    /// Root-host knowledge accumulated from direct config and observed traffic.
    pub root_knowledge: RootKnowledge,
    pub(super) chat_sessions: BTreeMap<u64, ChatSession>,
}

/// Per-node runtime wiring used by the simulator.
#[derive(Debug)]
pub(super) struct SimNode {
    /// Optional parent node in the explicit tree.
    pub(super) parent: Option<NodeId>,
    /// Child node ids in display order.
    pub(super) children: Vec<NodeId>,
    /// Backing protocol runtime for this endpoint.
    pub(super) endpoint: ProtocolEndpoint,
    /// Mailbox sender used by other nodes when forwarding frames here.
    pub(super) tx: Sender<Envelope>,
    /// Mailbox receiver consumed by `Simulation::step`.
    pub(super) rx: Receiver<Envelope>,
}

/// Internal packet delivery envelope.
#[derive(Debug, Clone)]
pub(super) struct Envelope {
    /// Ingress side seen by the receiving protocol runtime.
    pub(super) ingress: Ingress,
    /// Fully framed packet bytes.
    pub(super) frame: FrameBytes,
}

/// Application-level chat state layered on top of hook traffic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ChatSession {
    /// Node hosting the application-level chat behavior.
    pub(super) node_id: NodeId,
    /// Hook id that the chat session is bound to.
    pub(super) hook_id: u64,
    /// Path of the hook host to which replies must be routed.
    pub(super) host_path: Vec<String>,
    /// Procedure contract associated with the chat stream.
    pub(super) procedure_id: String,
}
