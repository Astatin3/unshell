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
    pub(super) nodes: Vec<SimNode>,
    pub(super) root_id: NodeId,
    pub(super) next_tick: u64,
    pub trace: VecDeque<TraceEvent>,
    pub recorded_events: Vec<RecordedEvent>,
    pub hooks: BTreeMap<u64, HookSnapshot>,
    pub inspector_mode: InspectorMode,
    pub root_knowledge: RootKnowledge,
    pub(super) chat_sessions: BTreeMap<u64, ChatSession>,
}

/// Per-node runtime wiring used by the simulator.
#[derive(Debug)]
pub(super) struct SimNode {
    pub(super) parent: Option<NodeId>,
    pub(super) children: Vec<NodeId>,
    pub(super) endpoint: ProtocolEndpoint,
    pub(super) tx: Sender<Envelope>,
    pub(super) rx: Receiver<Envelope>,
}

/// Internal packet delivery envelope.
#[derive(Debug, Clone)]
pub(super) struct Envelope {
    pub(super) ingress: Ingress,
    pub(super) frame: FrameBytes,
}

/// Application-level chat state layered on top of hook traffic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ChatSession {
    pub(super) node_id: NodeId,
    pub(super) hook_id: u64,
    pub(super) host_path: Vec<String>,
    pub(super) procedure_id: String,
}
