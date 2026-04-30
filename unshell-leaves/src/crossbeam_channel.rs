//! Crossbeam-channel-backed router leaf for in-process protocol simulations.
//!
//! This leaf owns parent/child transport links backed by `crossbeam_channel`, so
//! tests and examples can exercise full packet routing without opening real
//! sockets.

use std::collections::BTreeMap;

use crossbeam_channel::Sender;
use rkyv::{Archive, Deserialize, Serialize};
use unshell_protocol::FrameBytes;
use unshell_protocol::tree::{
    CallLeaf, ChildRoute, Endpoint, Ingress, ProtocolEndpoint, RouterLeaf,
};

use crate::{leaf, procedures};

/// One inbound frame delivered across a simulated channel hop.
///
/// What it is: the transport envelope sent between in-process nodes when this
/// leaf forwards protocol traffic over `crossbeam_channel`.
///
/// Why it exists: routing needs both the encoded frame bytes and the ingress side
/// that the receiver should apply when validating source paths.
///
/// # Example
/// ```rust
/// use unshell_leaves::crossbeam_channel::CrossbeamEnvelope;
/// use unshell_leaves::protocol::{FrameBytes, tree::Ingress};
/// let envelope = CrossbeamEnvelope {
///     ingress: Ingress::Parent,
///     frame: FrameBytes::new(),
/// };
/// assert!(matches!(envelope.ingress, Ingress::Parent));
/// ```
#[derive(Debug, Clone)]
pub struct CrossbeamEnvelope {
    /// Which side of the tree the receiving endpoint should treat this frame as coming from.
    pub ingress: Ingress,
    /// Encoded protocol frame bytes.
    pub frame: FrameBytes,
}

/// Request payload for promoting or pruning one simulated connection.
///
/// What it is: the protocol payload shared by the `add_connection` and
/// `remove_connection` procedures.
///
/// Why it exists: the leaf only needs the peer endpoint path to decide whether the
/// connection is a direct parent edge or a direct child edge.
///
/// # Example
/// ```rust
/// use unshell_leaves::crossbeam_channel::ConnectionRequest;
/// let request = ConnectionRequest {
///     peer_path: vec!["agent".into(), "child".into()],
/// };
/// assert_eq!(request.peer_path.len(), 2);
/// ```
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ConnectionRequest {
    /// Absolute endpoint path of the peer connection being managed.
    pub peer_path: Vec<String>,
}

/// Machine-readable snapshot of the leaf's active simulated connections.
///
/// What it is: the reply payload returned by `get_connections`, `add_connection`,
/// and `remove_connection`.
///
/// Why it exists: connection-management procedures should return the resulting
/// topology immediately so tests and tooling can confirm what changed.
///
/// # Example
/// ```rust
/// use unshell_leaves::crossbeam_channel::ConnectionSnapshot;
/// let snapshot = ConnectionSnapshot {
///     parent: Some(vec!["agent".into()]),
///     children: vec![vec!["agent".into(), "child".into()]],
/// };
/// assert_eq!(snapshot.children.len(), 1);
/// ```
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ConnectionSnapshot {
    /// The direct parent path, if this endpoint currently has one.
    pub parent: Option<Vec<String>>,
    /// The currently active direct child paths.
    pub children: Vec<Vec<String>>,
}

/// Errors surfaced by the channel-backed router leaf.
///
/// What it is: the small, deterministic error set used by both the management
/// procedures and the transport forwarding hooks.
///
/// Why it exists: tests and examples need structured failures when a staged link is
/// missing, a path is not a direct neighbor, or a channel is already closed.
///
/// # Example
/// ```rust
/// use unshell_leaves::crossbeam_channel::CrossbeamChannelError;
/// let error = CrossbeamChannelError::MissingStagedConnection;
/// assert_eq!(error.to_string(), "missing staged connection");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrossbeamChannelError {
    /// The requested peer path does not have a staged sender ready to activate.
    MissingStagedConnection,
    /// The requested peer path is neither the direct parent nor a direct child.
    InvalidPeerPath,
    /// No active parent link exists for upstream forwarding.
    MissingParentConnection,
    /// No active child link exists for the requested child path.
    MissingChildConnection,
    /// The receiving side of the channel is already disconnected.
    ChannelClosed,
}

impl core::fmt::Display for CrossbeamChannelError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingStagedConnection => f.write_str("missing staged connection"),
            Self::InvalidPeerPath => f.write_str("peer path is not a direct parent or child"),
            Self::MissingParentConnection => f.write_str("missing parent connection"),
            Self::MissingChildConnection => f.write_str("missing child connection"),
            Self::ChannelClosed => f.write_str("channel receiver is disconnected"),
        }
    }
}

impl core::error::Error for CrossbeamChannelError {}

/// Shared compile-time declaration for the crossbeam-channel router leaf.
///
/// What it is: the public leaf declaration that owns the canonical leaf name and
/// exported management procedure ids for [`CrossbeamChannelLeaf`].
///
/// Why it exists: endpoint code, examples, and tests should all derive the same
/// protocol-facing metadata from one source of truth instead of hand-assembling
/// the leaf id and procedure inventory.
///
/// # Example
/// ```rust
/// use unshell_leaves::crossbeam_channel::CrossbeamChannel;
/// assert!(CrossbeamChannel::protocol_leaf_name().contains("crossbeam_channel"));
/// ```
#[leaf(
    id = "org.unshell.v1.crossbeam_channel",
    endpoint_struct = CrossbeamChannelLeaf,
    procedures = ["add_connection", "remove_connection", "get_connections"]
)]
pub struct CrossbeamChannel;

/// In-process router leaf backed by `crossbeam_channel` senders.
///
/// What it is: a leaf host that stores one optional parent sender, any number of
/// child senders, and a staging area for connections that should only become live
/// after an explicit procedure call.
///
/// Why it exists: protocol tests need a realistic forwarding surface with parent
/// and child links, but opening TCP sockets would make those tests slower and more
/// brittle than necessary.
///
/// # Example
/// ```rust
/// use crossbeam_channel::unbounded;
/// use unshell_leaves::crossbeam_channel::CrossbeamChannelLeaf;
/// let (tx, _rx) = unbounded();
/// let mut leaf = CrossbeamChannelLeaf::default();
/// let previous = leaf.stage_connection(vec!["agent".into()], tx);
/// assert!(previous.is_none());
/// ```
#[derive(Default)]
pub struct CrossbeamChannelLeaf {
    parent: Option<ChannelConnection>,
    children: BTreeMap<Vec<String>, Sender<CrossbeamEnvelope>>,
    child_routes: Vec<ChildRoute>,
    staged: BTreeMap<Vec<String>, Sender<CrossbeamEnvelope>>,
}

#[derive(Debug, Clone)]
struct ChannelConnection {
    path: Vec<String>,
    sender: Sender<CrossbeamEnvelope>,
}

impl CrossbeamChannelLeaf {
    /// Stages one channel sender so a later protocol procedure can activate it.
    ///
    /// What it is: a bootstrap helper that prepares the transport handle before the
    /// leaf promotes it into active routing state.
    ///
    /// Why it exists: the sender itself is not a serializable protocol payload, so
    /// tests and examples need a local way to install it before calling
    /// `add_connection`.
    pub fn stage_connection(
        &mut self,
        peer_path: Vec<String>,
        sender: Sender<CrossbeamEnvelope>,
    ) -> Option<Sender<CrossbeamEnvelope>> {
        self.staged.insert(peer_path, sender)
    }

    /// Promotes one staged connection into the active topology.
    ///
    /// This is the same operation used by the public `add_connection` procedure,
    /// but it is also useful for local bootstrap code that has not yet wired the
    /// control plane needed to issue that call remotely.
    pub fn connect_staged(
        &mut self,
        endpoint: &mut ProtocolEndpoint,
        peer_path: Vec<String>,
    ) -> Result<ConnectionSnapshot, CrossbeamChannelError> {
        if !is_direct_parent(endpoint.path(), &peer_path)
            && !is_direct_child(endpoint.path(), &peer_path)
        {
            return Err(CrossbeamChannelError::InvalidPeerPath);
        }

        let Some(sender) = self.staged.remove(&peer_path) else {
            return Err(CrossbeamChannelError::MissingStagedConnection);
        };

        if is_direct_parent(endpoint.path(), &peer_path) {
            self.parent = Some(ChannelConnection {
                path: peer_path.clone(),
                sender,
            });
            endpoint
                .set_parent_path(Some(peer_path))
                .map_err(|_| CrossbeamChannelError::InvalidPeerPath)?;
            return Ok(ConnectionSnapshot::from_endpoint(endpoint));
        }

        if is_direct_child(endpoint.path(), &peer_path) {
            self.children.insert(peer_path.clone(), sender);
            self.sync_child_routes();
            endpoint
                .upsert_child_route(ChildRoute::registered(peer_path))
                .map_err(|_| CrossbeamChannelError::InvalidPeerPath)?;
            return Ok(ConnectionSnapshot::from_endpoint(endpoint));
        }

        unreachable!("direct-neighbor validation returned early above")
    }

    /// Removes one active connection and returns it to the staged set.
    pub fn disconnect(
        &mut self,
        endpoint: &mut ProtocolEndpoint,
        peer_path: &[String],
    ) -> Result<ConnectionSnapshot, CrossbeamChannelError> {
        if !is_direct_parent(endpoint.path(), peer_path)
            && !is_direct_child(endpoint.path(), peer_path)
        {
            return Err(CrossbeamChannelError::InvalidPeerPath);
        }

        if self
            .parent
            .as_ref()
            .is_some_and(|parent| parent.path == peer_path)
        {
            let Some(parent) = self.parent.take() else {
                return Err(CrossbeamChannelError::MissingParentConnection);
            };
            self.staged.insert(parent.path, parent.sender);
            endpoint
                .set_parent_path(None)
                .map_err(|_| CrossbeamChannelError::InvalidPeerPath)?;
            return Ok(ConnectionSnapshot::from_endpoint(endpoint));
        }

        let Some(sender) = self.children.remove(peer_path) else {
            return Err(CrossbeamChannelError::MissingChildConnection);
        };
        self.staged.insert(peer_path.to_vec(), sender);
        self.sync_child_routes();
        endpoint.remove_child_route(peer_path);
        Ok(ConnectionSnapshot::from_endpoint(endpoint))
    }

    fn sync_child_routes(&mut self) {
        self.child_routes = self
            .children
            .keys()
            .cloned()
            .map(ChildRoute::registered)
            .collect();
    }
}

impl ConnectionSnapshot {
    fn from_endpoint(endpoint: &ProtocolEndpoint) -> Self {
        Self {
            parent: endpoint.parent_path().map(<[String]>::to_vec),
            children: endpoint
                .child_routes()
                .iter()
                .map(|child| child.path.clone())
                .collect(),
        }
    }
}

#[procedures(error = CrossbeamChannelError)]
impl CrossbeamChannelLeaf {
    #[call]
    fn add_connection(
        &mut self,
        endpoint: &mut ProtocolEndpoint,
        request: ConnectionRequest,
    ) -> Result<ConnectionSnapshot, CrossbeamChannelError> {
        self.connect_staged(endpoint, request.peer_path)
    }

    #[call]
    fn remove_connection(
        &mut self,
        endpoint: &mut ProtocolEndpoint,
        request: ConnectionRequest,
    ) -> Result<ConnectionSnapshot, CrossbeamChannelError> {
        self.disconnect(endpoint, &request.peer_path)
    }

    #[call]
    fn get_connections(&mut self, endpoint: &ProtocolEndpoint) -> ConnectionSnapshot {
        ConnectionSnapshot::from_endpoint(endpoint)
    }
}

impl CallLeaf for CrossbeamChannelLeaf {
    type Error = CrossbeamChannelError;
}

impl RouterLeaf for CrossbeamChannelLeaf {
    type RouteError = CrossbeamChannelError;

    fn parent_path(&self) -> Option<&[String]> {
        self.parent.as_ref().map(|parent| parent.path.as_slice())
    }

    fn child_routes(&self) -> &[ChildRoute] {
        &self.child_routes
    }

    fn route_to_parent(
        &mut self,
        local_path: &[String],
        frame: FrameBytes,
    ) -> Result<(), Self::RouteError> {
        let Some(parent) = &self.parent else {
            return Err(CrossbeamChannelError::MissingParentConnection);
        };
        parent
            .sender
            .send(CrossbeamEnvelope {
                ingress: Ingress::Child(local_path.to_vec()),
                frame,
            })
            .map_err(|_| CrossbeamChannelError::ChannelClosed)
    }

    fn route_to_child(
        &mut self,
        child_path: &[String],
        frame: FrameBytes,
    ) -> Result<(), Self::RouteError> {
        let Some(sender) = self.children.get(child_path) else {
            return Err(CrossbeamChannelError::MissingChildConnection);
        };
        sender
            .send(CrossbeamEnvelope {
                ingress: Ingress::Parent,
                frame,
            })
            .map_err(|_| CrossbeamChannelError::ChannelClosed)
    }
}

fn is_direct_parent(local_path: &[String], peer_path: &[String]) -> bool {
    local_path
        .split_last()
        .is_some_and(|(_, parent_path)| parent_path == peer_path)
}

fn is_direct_child(local_path: &[String], peer_path: &[String]) -> bool {
    peer_path.len() == local_path.len() + 1 && peer_path.starts_with(local_path)
}

#[cfg(test)]
mod tests {
    use crossbeam_channel::{Receiver, unbounded};
    use unshell_protocol::decode_frame;
    use unshell_protocol::tree::{
        Endpoint, EndpointOutcome, LeafRuntime, decode_call_input, encode_call_reply,
    };

    use super::*;

    fn path(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_owned()).collect()
    }

    struct ChannelNode {
        runtime: LeafRuntime<CrossbeamChannelLeaf>,
        rx: Receiver<CrossbeamEnvelope>,
    }

    impl ChannelNode {
        fn new(path: Vec<String>) -> (Self, Sender<CrossbeamEnvelope>) {
            let (tx, rx) = unbounded();
            let endpoint = ProtocolEndpoint::new(
                path,
                None,
                Vec::new(),
                vec![CrossbeamChannelLeaf::protocol_leaf_spec()],
            );
            (
                Self {
                    runtime: LeafRuntime::new(endpoint, CrossbeamChannelLeaf::default()),
                    rx,
                },
                tx,
            )
        }

        fn drain(&mut self) -> usize {
            let mut processed = 0usize;
            while let Ok(envelope) = self.rx.try_recv() {
                let outcome = self
                    .runtime
                    .receive_routed(&envelope.ingress, envelope.frame)
                    .expect("node should process routed frame");
                self.runtime
                    .route_forwarded(outcome.forwarded)
                    .expect("router leaf should forward emitted frames");
                processed += 1;
            }
            processed
        }

        fn stage_connection(&mut self, peer_path: Vec<String>, sender: Sender<CrossbeamEnvelope>) {
            let _ = self.runtime.leaf_mut().stage_connection(peer_path, sender);
        }

        fn connect_staged(&mut self, peer_path: Vec<String>) {
            let snapshot = {
                let runtime = &mut self.runtime;
                let mut leaf = core::mem::take(runtime.leaf_mut());
                let result = leaf.connect_staged(runtime.endpoint_mut(), peer_path);
                *runtime.leaf_mut() = leaf;
                result
            };
            snapshot.expect("staged connection should activate");
        }
    }

    #[test]
    fn crossbeam_channel_leaf_routes_calls_and_replies_across_parent_and_child_links() {
        let (mut agent, root_to_agent) = ChannelNode::new(path(&["agent"]));
        let (mut child, agent_to_child) = ChannelNode::new(path(&["agent", "child"]));
        let (agent_to_root, root_rx) = unbounded();

        let mut root = ProtocolEndpoint::new(
            Vec::new(),
            None,
            vec![ChildRoute::registered(path(&["agent"]))],
            Vec::new(),
        );

        agent.stage_connection(Vec::new(), agent_to_root);
        agent.connect_staged(Vec::new());

        child.stage_connection(path(&["agent"]), root_to_agent.clone());
        child.connect_staged(path(&["agent"]));

        agent.stage_connection(path(&["agent", "child"]), agent_to_child);

        let hook_id = root.allocate_hook_id();
        let add_connection = root
            .send_call(
                path(&["agent"]),
                Some(CrossbeamChannelLeaf::protocol_leaf_name()),
                CrossbeamChannelLeaf::protocol_procedure_id("add_connection")
                    .expect("procedure should exist"),
                Some(hook_id),
                encode_call_reply(&ConnectionRequest {
                    peer_path: path(&["agent", "child"]),
                })
                .expect("request should encode"),
            )
            .expect("root should build add-connection call");
        let EndpointOutcome::Forward { frame, .. } = add_connection else {
            panic!("root should forward add-connection call");
        };
        root_to_agent
            .send(CrossbeamEnvelope {
                ingress: Ingress::Parent,
                frame,
            })
            .expect("root should deliver frame to agent");

        for _ in 0..8 {
            let mut progress = 0usize;
            progress += agent.drain();
            progress += child.drain();
            while let Ok(envelope) = root_rx.try_recv() {
                let outcome = root
                    .receive(&envelope.ingress, envelope.frame)
                    .expect("root should accept reply frame");
                if let EndpointOutcome::Local(local) = outcome {
                    match local {
                        unshell_protocol::tree::LocalEvent::Data { .. }
                        | unshell_protocol::tree::LocalEvent::Fault { .. } => {}
                        unshell_protocol::tree::LocalEvent::Call { .. } => {}
                    }
                }
                progress += 1;
            }
            if progress == 0 {
                break;
            }
        }

        assert_eq!(agent.runtime.endpoint().child_routes().len(), 1);

        let query_hook = root.allocate_hook_id();
        let query = root
            .send_call(
                path(&["agent", "child"]),
                Some(CrossbeamChannelLeaf::protocol_leaf_name()),
                CrossbeamChannelLeaf::protocol_procedure_id("get_connections")
                    .expect("procedure should exist"),
                Some(query_hook),
                encode_call_reply(&()).expect("unit request should encode"),
            )
            .expect("root should build query call");
        let EndpointOutcome::Forward { frame, .. } = query else {
            panic!("root should forward query call");
        };
        root_to_agent
            .send(CrossbeamEnvelope {
                ingress: Ingress::Parent,
                frame,
            })
            .expect("root should deliver query to agent");

        let mut reply = None;
        for _ in 0..12 {
            let mut progress = 0usize;
            progress += agent.drain();
            progress += child.drain();
            while let Ok(envelope) = root_rx.try_recv() {
                let outcome = root
                    .receive(&envelope.ingress, envelope.frame)
                    .expect("root should accept routed reply");
                if let EndpointOutcome::Local(unshell_protocol::tree::LocalEvent::Data {
                    message,
                    ..
                }) = outcome
                {
                    reply = Some(
                        decode_call_input::<ConnectionSnapshot>(message.data.as_slice())
                            .expect("reply payload should decode"),
                    );
                }
                progress += 1;
            }
            if reply.is_some() || progress == 0 {
                break;
            }
        }

        let reply = reply.expect("root should receive child connection snapshot");
        assert_eq!(reply.parent, Some(path(&["agent"])));
        assert!(reply.children.is_empty());

        let remove_hook = root.allocate_hook_id();
        let remove = root
            .send_call(
                path(&["agent"]),
                Some(CrossbeamChannelLeaf::protocol_leaf_name()),
                CrossbeamChannelLeaf::protocol_procedure_id("remove_connection")
                    .expect("procedure should exist"),
                Some(remove_hook),
                encode_call_reply(&ConnectionRequest {
                    peer_path: path(&["agent", "child"]),
                })
                .expect("request should encode"),
            )
            .expect("root should build remove-connection call");
        let EndpointOutcome::Forward { frame, .. } = remove else {
            panic!("root should forward remove-connection call");
        };
        root_to_agent
            .send(CrossbeamEnvelope {
                ingress: Ingress::Parent,
                frame,
            })
            .expect("root should deliver removal call to agent");

        for _ in 0..8 {
            let mut progress = 0usize;
            progress += agent.drain();
            progress += child.drain();
            while let Ok(envelope) = root_rx.try_recv() {
                let _ = root
                    .receive(&envelope.ingress, envelope.frame)
                    .expect("root should process removal reply");
                progress += 1;
            }
            if progress == 0 {
                break;
            }
        }

        assert!(agent.runtime.endpoint().child_routes().is_empty());
        let final_hook = root.allocate_hook_id();
        let dropped = root
            .send_call(
                path(&["agent", "child"]),
                Some(CrossbeamChannelLeaf::protocol_leaf_name()),
                CrossbeamChannelLeaf::protocol_procedure_id("get_connections")
                    .expect("procedure should exist"),
                Some(final_hook),
                encode_call_reply(&()).expect("unit request should encode"),
            )
            .expect("query call should encode after removal");
        assert!(matches!(dropped, EndpointOutcome::Forward { .. }));

        if let EndpointOutcome::Forward { frame, .. } = dropped {
            root_to_agent
                .send(CrossbeamEnvelope {
                    ingress: Ingress::Parent,
                    frame,
                })
                .expect("root should still reach the agent");
        }
        let mut saw_reply = false;
        for _ in 0..8 {
            let mut progress = 0usize;
            progress += agent.drain();
            progress += child.drain();
            while let Ok(envelope) = root_rx.try_recv() {
                progress += 1;
                if let EndpointOutcome::Local(unshell_protocol::tree::LocalEvent::Data {
                    message,
                    ..
                }) = root
                    .receive(&envelope.ingress, envelope.frame)
                    .expect("root should process any late reply")
                {
                    let _ = decode_frame(message.data.as_slice());
                    saw_reply = true;
                }
            }
            if progress == 0 {
                break;
            }
        }
        assert!(
            !saw_reply,
            "removed child route should stop forwarded replies"
        );
    }

    #[test]
    fn invalid_add_connection_keeps_staged_sender_available_for_retry() {
        let (tx, _rx) = unbounded();
        let mut leaf = CrossbeamChannelLeaf::default();
        let mut endpoint = ProtocolEndpoint::new(path(&["agent"]), None, Vec::new(), Vec::new());
        leaf.stage_connection(path(&["elsewhere"]), tx);

        let error = leaf
            .connect_staged(&mut endpoint, path(&["elsewhere"]))
            .expect_err("non-neighbor path should fail");
        assert_eq!(error, CrossbeamChannelError::InvalidPeerPath);
        assert!(leaf.staged.contains_key(&path(&["elsewhere"])));
    }

    #[test]
    fn invalid_remove_connection_reports_invalid_peer_path() {
        let mut leaf = CrossbeamChannelLeaf::default();
        let mut endpoint = ProtocolEndpoint::new(path(&["agent"]), None, Vec::new(), Vec::new());

        let error = leaf
            .disconnect(&mut endpoint, &path(&["not", "a", "neighbor"]))
            .expect_err("non-neighbor removal should fail");
        assert_eq!(error, CrossbeamChannelError::InvalidPeerPath);
    }
}
