//! Single-threaded runtime shell around endpoint packet state.
//!
//! This first slice owns transport and connection metadata, derives ingress from
//! registered connections, delegates packet invariants to [`EndpointState`], and
//! queues concrete runtime effects. Leaf action reduction is intentionally
//! narrow and grows one action family at a time.

use crate::alloc::{string::String, vec::Vec};
use crate::connections::{
    Connection, ConnectionDirection, ConnectionGeneration, ConnectionId, ConnectionState,
    Connections, RegisteredConnection,
};
use crate::context::{LeafAction, LeafContext};
use crate::effects::{EffectQueue, RuntimeEffect};
use crate::leaf::{Leaf, LeafId, RegisteredLeaf};
use crate::transport::Transport;
use unshell_protocol::FrameBytes;
use unshell_protocol::tree::ChildRoute;
use unshell_protocol::tree::{
    Endpoint, EndpointError, EndpointOutcome, IncomingCall, IncomingData, IncomingFault, Ingress,
    LocalEvent, RouteDecision,
};

use super::{EndpointState, PacketProcessor};

/// Limits one runtime progress step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TickBudget {
    /// Maximum inbound frames to poll from the transport.
    pub max_inbound_frames: usize,
    /// Whether queued outbound frame effects should be flushed through transport.
    pub flush_outbound: bool,
}

impl Default for TickBudget {
    fn default() -> Self {
        Self {
            max_inbound_frames: 16,
            flush_outbound: true,
        }
    }
}

/// Summary returned after one runtime step.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TickOutcome {
    /// Number of inbound frames processed.
    pub inbound_frames: usize,
    /// Number of outbound frames sent.
    pub outbound_frames: usize,
    /// Number of frames intentionally dropped.
    pub dropped_frames: usize,
    /// Number of local endpoint events queued for later leaf dispatch.
    pub local_events: usize,
}

/// Error surfaced by [`NodeRuntime`].
#[derive(Debug)]
pub enum NodeRuntimeError<TransportError> {
    /// The connection is unknown or not registered for protocol routing.
    UnregisteredConnection(ConnectionId),
    /// The endpoint selected a route with no matching registered connection.
    MissingRouteConnection,
    /// Packet processing failed inside endpoint state.
    Endpoint(EndpointError),
    /// Transport send, receive, or flush failed.
    Transport(TransportError),
    /// A queued leaf action is not implemented by this runtime slice.
    UnsupportedLeafAction {
        /// Leaf id that requested the action.
        leaf_id: LeafId,
        /// Stable action name for diagnostics.
        action: &'static str,
    },
}

/// Error returned when a leaf callback rejects a local event.
#[derive(Debug)]
pub struct LeafDispatchError<LeafError> {
    /// Leaf id that received the event.
    pub leaf_id: LeafId,
    /// Callback-specific error returned by the leaf.
    pub source: LeafError,
}

impl<LeafError> core::fmt::Display for LeafDispatchError<LeafError>
where
    LeafError: core::fmt::Display,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "leaf {} failed during dispatch: {}",
            self.leaf_id.as_str(),
            self.source
        )
    }
}

impl<LeafError> core::error::Error for LeafDispatchError<LeafError> where
    LeafError: core::error::Error + 'static
{
}

impl<TransportError> core::fmt::Display for NodeRuntimeError<TransportError>
where
    TransportError: core::fmt::Display,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnregisteredConnection(connection) => {
                write!(f, "connection {} is not registered", connection.get())
            }
            Self::MissingRouteConnection => f.write_str("route has no registered connection"),
            Self::Endpoint(error) => write!(f, "{error}"),
            Self::Transport(error) => write!(f, "{error}"),
            Self::UnsupportedLeafAction { leaf_id, action } => {
                write!(
                    f,
                    "leaf {} requested unsupported action {action}",
                    leaf_id.as_str()
                )
            }
        }
    }
}

impl<TransportError> core::error::Error for NodeRuntimeError<TransportError> where
    TransportError: core::error::Error + 'static
{
}

/// Runtime owner for one endpoint, transport, and connection table.
#[derive(Debug)]
pub struct NodeRuntime<T, LeafError = core::convert::Infallible> {
    endpoint: EndpointState,
    connections: Connections,
    transport: T,
    effects: EffectQueue,
    leaves: Vec<RegisteredLeaf<LeafError>>,
    leaf_actions: Vec<(LeafId, LeafAction)>,
}

impl<T> NodeRuntime<T> {
    /// Creates a runtime from endpoint state, registered connection metadata, and
    /// one concrete transport.
    #[must_use]
    pub const fn new(endpoint: EndpointState, connections: Connections, transport: T) -> Self {
        Self {
            endpoint,
            connections,
            transport,
            effects: EffectQueue::new(),
            leaves: Vec::new(),
            leaf_actions: Vec::new(),
        }
    }
}

impl<T, LeafError> NodeRuntime<T, LeafError> {
    /// Creates a runtime with an explicit leaf callback error type.
    #[must_use]
    pub const fn new_with_leaf_error(
        endpoint: EndpointState,
        connections: Connections,
        transport: T,
    ) -> Self {
        Self {
            endpoint,
            connections,
            transport,
            effects: EffectQueue::new(),
            leaves: Vec::new(),
            leaf_actions: Vec::new(),
        }
    }

    /// Returns endpoint packet state.
    #[must_use]
    pub const fn endpoint(&self) -> &EndpointState {
        &self.endpoint
    }

    /// Returns mutable endpoint packet state.
    #[must_use]
    pub const fn endpoint_mut(&mut self) -> &mut EndpointState {
        &mut self.endpoint
    }

    /// Returns connection metadata.
    #[must_use]
    pub const fn connections(&self) -> &Connections {
        &self.connections
    }

    /// Returns mutable connection metadata.
    #[must_use]
    pub const fn connections_mut(&mut self) -> &mut Connections {
        &mut self.connections
    }

    /// Returns the transport.
    #[must_use]
    pub const fn transport(&self) -> &T {
        &self.transport
    }

    /// Returns the mutable transport.
    #[must_use]
    pub const fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// Registers or updates the parent connection and endpoint parent route together.
    ///
    /// Call this instead of mutating [`Connections`] and [`EndpointState`] separately.
    /// The endpoint validates that `parent_path` is the direct parent before the
    /// connection table is made routable.
    pub fn register_parent_connection(
        &mut self,
        connection: ConnectionId,
        parent_path: Vec<String>,
        generation: ConnectionGeneration,
    ) -> Result<(), EndpointError> {
        let previous = self.connections.registered(connection).cloned();
        self.endpoint
            .endpoint_mut()
            .set_parent_path(Some(parent_path.clone()))?;

        if let Some(previous) = previous
            && previous.direction() == ConnectionDirection::Child
        {
            self.endpoint
                .endpoint_mut()
                .remove_child_route(previous.peer_path());
        }

        self.upsert_registered_connection(
            connection,
            ConnectionDirection::Parent,
            parent_path.clone(),
            generation,
        );
        self.connections
            .demote_registered_direction_except(ConnectionDirection::Parent, connection);
        Ok(())
    }

    /// Registers or updates a child connection and endpoint child route together.
    ///
    /// Call this instead of mutating [`Connections`] and [`EndpointState`] separately.
    /// The endpoint validates that `child_path` is a direct child before the
    /// connection table is made routable.
    pub fn register_child_connection(
        &mut self,
        connection: ConnectionId,
        child_path: Vec<String>,
        generation: ConnectionGeneration,
    ) -> Result<(), EndpointError> {
        let previous = self.connections.registered(connection).cloned();
        self.endpoint
            .endpoint_mut()
            .upsert_child_route(ChildRoute::registered(child_path.clone()))?;

        if let Some(previous) = previous {
            match previous.direction() {
                ConnectionDirection::Parent => {
                    self.endpoint.endpoint_mut().set_parent_path(None)?;
                }
                ConnectionDirection::Child if previous.peer_path() != child_path.as_slice() => {
                    self.endpoint
                        .endpoint_mut()
                        .remove_child_route(previous.peer_path());
                }
                ConnectionDirection::Child => {}
            }
        }

        self.upsert_registered_connection(
            connection,
            ConnectionDirection::Child,
            child_path.clone(),
            generation,
        );
        self.connections.demote_registered_path_except(
            ConnectionDirection::Child,
            &child_path,
            connection,
        );
        Ok(())
    }

    fn upsert_registered_connection(
        &mut self,
        connection: ConnectionId,
        direction: ConnectionDirection,
        peer_path: Vec<String>,
        generation: ConnectionGeneration,
    ) {
        if let Some(existing) = self.connections.get_mut(connection) {
            let state = ConnectionState::Registered(RegisteredConnection::new(
                direction, peer_path, generation,
            ));
            existing.set_state(state);
        } else {
            self.connections.push(Connection::registered(
                connection, direction, peer_path, generation,
            ));
        }
    }

    /// Returns currently queued effects.
    #[must_use]
    pub fn effects(&self) -> &[RuntimeEffect] {
        self.effects.entries()
    }

    /// Drains queued local-dispatch effects in FIFO order.
    ///
    /// Outbound frame effects remain queued for runtime-owned transport flushing.
    pub fn drain_local_effects(&mut self) -> impl Iterator<Item = RuntimeEffect> {
        self.effects.drain_local()
    }

    /// Registers a leaf under its declared `leaf_name` dispatch id.
    ///
    /// If the id already exists, the new handler replaces the previous one. This
    /// keeps local dispatch deterministic without adding a broader registry API.
    pub fn register_leaf<L>(&mut self, leaf: L) -> LeafId
    where
        L: Leaf<Error = LeafError> + 'static,
    {
        let id = LeafId::new(leaf.capabilities().leaf_name.clone());
        self.register_leaf_as(id.clone(), leaf);
        id
    }

    /// Registers a leaf under an explicit dispatch id.
    ///
    /// This is useful when tests or adapters already hold the exact `dst_leaf`
    /// string from protocol metadata. Duplicate ids are replaced.
    pub fn register_leaf_as<L>(&mut self, id: LeafId, leaf: L)
    where
        L: Leaf<Error = LeafError> + 'static,
    {
        if let Some(existing) = self.leaves.iter_mut().find(|entry| entry.id() == &id) {
            *existing = RegisteredLeaf::new(id, leaf);
        } else {
            self.leaves.push(RegisteredLeaf::new(id, leaf));
        }
    }

    /// Returns registered leaf handlers.
    #[must_use]
    pub fn leaves(&self) -> &[RegisteredLeaf<LeafError>] {
        &self.leaves
    }

    /// Returns leaf actions queued by dispatched callbacks.
    #[must_use]
    pub fn leaf_actions(&self) -> &[(LeafId, LeafAction)] {
        &self.leaf_actions
    }

    /// Drains leaf actions queued by dispatched callbacks.
    pub fn drain_leaf_actions(&mut self) -> impl Iterator<Item = (LeafId, LeafAction)> {
        let actions = core::mem::take(&mut self.leaf_actions);
        actions.into_iter()
    }

    /// Dispatches currently queued local effects to matching leaf handlers.
    ///
    /// Local events are attempted in FIFO queue order. A matched event is removed
    /// only after the leaf callback succeeds. Unmatched local events, outbound
    /// sends, and drop notifications remain queued for future runtime work.
    pub fn dispatch_local_effects(&mut self) -> Result<usize, LeafDispatchError<LeafError>> {
        let mut retained = EffectQueue::new();
        let mut dispatched = 0usize;
        let mut pending = core::mem::take(&mut self.effects);
        let mut drained = pending.drain();

        while let Some(effect) = drained.next() {
            match effect {
                RuntimeEffect::Local(event) => {
                    let Some(leaf_index) = self.leaf_index_for_event(&event) else {
                        retained.push(RuntimeEffect::Local(event));
                        continue;
                    };

                    if let Err(error) = self.dispatch_event_to_leaf(leaf_index, &event) {
                        retained.push(RuntimeEffect::Local(event));
                        for remaining in drained {
                            retained.push(remaining);
                        }
                        self.effects = retained;
                        return Err(error);
                    }
                    dispatched += 1;
                }
                other => retained.push(other),
            }
        }

        self.effects = retained;
        Ok(dispatched)
    }

    fn leaf_index_for_event(&self, event: &LocalEvent) -> Option<usize> {
        let leaf_name = local_event_leaf_name(event)?;
        self.leaves
            .iter()
            .position(|entry| entry.id().as_str() == leaf_name)
    }

    fn dispatch_event_to_leaf(
        &mut self,
        leaf_index: usize,
        event: &LocalEvent,
    ) -> Result<(), LeafDispatchError<LeafError>> {
        let local_path = self.endpoint.endpoint().path();
        let (leaf_id, actions) = {
            let leaf = &mut self.leaves[leaf_index];
            let (leaf_id, capabilities, handler) = leaf.dispatch_parts_mut();
            let mut ctx = LeafContext::new(local_path, leaf_id, capabilities, &self.connections);

            match event {
                LocalEvent::Call { header, message } => handler
                    .on_call(
                        &mut ctx,
                        IncomingCall {
                            header: header.clone(),
                            message: message.clone(),
                        },
                    )
                    .map_err(|source| LeafDispatchError {
                        leaf_id: leaf_id.clone(),
                        source,
                    })?,
                LocalEvent::Data {
                    header,
                    message,
                    hook_key,
                } => handler
                    .on_data(
                        &mut ctx,
                        IncomingData {
                            header: header.clone(),
                            message: message.clone(),
                            hook_key: hook_key.clone(),
                        },
                    )
                    .map_err(|source| LeafDispatchError {
                        leaf_id: leaf_id.clone(),
                        source,
                    })?,
                LocalEvent::Fault {
                    header,
                    message,
                    hook_key,
                } => handler
                    .on_fault(
                        &mut ctx,
                        IncomingFault {
                            header: header.clone(),
                            fault: message.clone(),
                            hook_key: hook_key.clone(),
                        },
                    )
                    .map_err(|source| LeafDispatchError {
                        leaf_id: leaf_id.clone(),
                        source,
                    })?,
            }

            (leaf_id.clone(), ctx.into_actions())
        };

        self.leaf_actions
            .extend(actions.into_iter().map(|action| (leaf_id.clone(), action)));
        Ok(())
    }
}

impl<T, LeafError> NodeRuntime<T, LeafError>
where
    T: Transport,
{
    /// Processes one nonblocking runtime step.
    pub fn tick(&mut self, budget: TickBudget) -> Result<TickOutcome, NodeRuntimeError<T::Error>> {
        let mut outcome = TickOutcome::default();
        let effects_start = self.effects.entries().len();

        for _ in 0..budget.max_inbound_frames {
            let Some((connection, frame)) = self
                .transport
                .poll_recv()
                .map_err(NodeRuntimeError::Transport)?
            else {
                break;
            };
            self.receive_frame(connection, frame)?;
            outcome.inbound_frames += 1;
        }

        outcome.dropped_frames += self
            .effects
            .entries()
            .iter()
            .skip(effects_start)
            .filter(|effect| matches!(effect, RuntimeEffect::Dropped))
            .count();
        outcome.local_events += self
            .effects
            .entries()
            .iter()
            .skip(effects_start)
            .filter(|effect| matches!(effect, RuntimeEffect::Local(_)))
            .count();

        if budget.flush_outbound {
            outcome.outbound_frames = self.flush_outbound()?;
        }
        Ok(outcome)
    }

    /// Processes one frame from a known transport connection.
    pub fn receive_frame(
        &mut self,
        connection: ConnectionId,
        frame: FrameBytes,
    ) -> Result<(), NodeRuntimeError<T::Error>> {
        let registered = self
            .connections
            .registered(connection)
            .ok_or(NodeRuntimeError::UnregisteredConnection(connection))?;
        let ingress = ingress_for(registered);
        let outcome = self
            .endpoint
            .process_frame(&ingress, frame)
            .map_err(NodeRuntimeError::Endpoint)?;
        self.apply_outcome(outcome)
    }

    /// Reduces queued leaf actions through endpoint packet state.
    ///
    /// [`LeafAction::SendCall`], [`LeafAction::SendHookData`], and
    /// [`LeafAction::FailHook`] are implemented in this slice. Unsupported
    /// actions stop reduction and remain queued with all later actions so callers
    /// can retry after a future runtime gains support.
    pub fn reduce_leaf_actions(&mut self) -> Result<usize, NodeRuntimeError<T::Error>> {
        let mut reduced = 0usize;
        let mut retained = Vec::new();
        let mut pending = core::mem::take(&mut self.leaf_actions).into_iter();

        while let Some((leaf_id, action)) = pending.next() {
            match action {
                LeafAction::SendCall(call) => {
                    let original_action = LeafAction::SendCall(call.clone());
                    let route = self.endpoint.route_decision(&call.dst_path);
                    if route_requires_connection(route)
                        && self.connection_for_route(route).is_none()
                    {
                        retained.push((leaf_id, original_action));
                        retained.extend(pending);
                        self.leaf_actions = retained;
                        return Err(NodeRuntimeError::MissingRouteConnection);
                    }

                    if let Err(error) = self.endpoint.validate_call_request(
                        &call.dst_path,
                        call.dst_leaf.as_ref(),
                        &call.procedure_id,
                        &call.payload,
                        call.expects_response,
                    ) {
                        retained.push((leaf_id, original_action));
                        retained.extend(pending);
                        self.leaf_actions = retained;
                        return Err(NodeRuntimeError::Endpoint(error));
                    }

                    // Allocate only after transport availability is known. A
                    // failed preflight must leave the queued call retryable
                    // without consuming a hook id or reserving pending hook state.
                    let endpoint_checkpoint = self.endpoint.clone();
                    let response_hook_id = call
                        .expects_response
                        .then(|| self.endpoint.allocate_hook_id());
                    let outcome = match self.endpoint.send_call(
                        call.dst_path,
                        call.dst_leaf,
                        call.procedure_id,
                        response_hook_id,
                        call.payload,
                    ) {
                        Ok(outcome) => outcome,
                        Err(error) => {
                            self.endpoint = endpoint_checkpoint;
                            retained.push((leaf_id, original_action));
                            retained.extend(pending);
                            self.leaf_actions = retained;
                            return Err(NodeRuntimeError::Endpoint(error));
                        }
                    };

                    if let Err(error) = self.apply_outcome(outcome) {
                        self.endpoint = endpoint_checkpoint;
                        retained.push((leaf_id, original_action));
                        retained.extend(pending);
                        self.leaf_actions = retained;
                        return Err(error);
                    }
                    reduced += 1;
                }
                LeafAction::SendHookData(data) => {
                    let original_action = LeafAction::SendHookData(data.clone());
                    let route = self.endpoint.route_decision(&data.dst_path);
                    if route_requires_connection(route)
                        && self.connection_for_route(route).is_none()
                    {
                        retained.push((leaf_id, original_action));
                        retained.extend(pending);
                        self.leaf_actions = retained;
                        return Err(NodeRuntimeError::MissingRouteConnection);
                    }

                    let outcome = match self.endpoint.send_hook_data(
                        data.dst_path,
                        data.hook_id,
                        data.procedure_id,
                        data.payload,
                        data.end_hook,
                    ) {
                        Ok(outcome) => outcome,
                        Err(error) => {
                            retained.push((leaf_id, original_action));
                            retained.extend(pending);
                            self.leaf_actions = retained;
                            return Err(NodeRuntimeError::Endpoint(error));
                        }
                    };

                    if let Err(error) = self.apply_outcome(outcome) {
                        retained.push((leaf_id, original_action));
                        retained.extend(pending);
                        self.leaf_actions = retained;
                        return Err(error);
                    }
                    reduced += 1;
                }
                LeafAction::FailHook { hook_id, fault } => {
                    let original_action = LeafAction::FailHook { hook_id, fault };
                    if let Some(route) = self.endpoint.hook_fault_route(hook_id)
                        && (matches!(route, RouteDecision::Drop)
                            || (route_requires_connection(route)
                                && self.connection_for_route(route).is_none()))
                    {
                        retained.push((leaf_id, original_action));
                        retained.extend(pending);
                        self.leaf_actions = retained;
                        return Err(NodeRuntimeError::MissingRouteConnection);
                    }

                    let endpoint_checkpoint = self.endpoint.clone();
                    let outcome = match self.endpoint.fail_hook(hook_id, fault) {
                        Ok(outcome) => outcome,
                        Err(error) => {
                            self.endpoint = endpoint_checkpoint;
                            retained.push((leaf_id, original_action));
                            retained.extend(pending);
                            self.leaf_actions = retained;
                            return Err(NodeRuntimeError::Endpoint(error));
                        }
                    };

                    if let Err(error) = self.apply_outcome(outcome) {
                        self.endpoint = endpoint_checkpoint;
                        retained.push((leaf_id, original_action));
                        retained.extend(pending);
                        self.leaf_actions = retained;
                        return Err(error);
                    }
                    reduced += 1;
                }
                unsupported => {
                    let action_name = leaf_action_name(&unsupported);
                    retained.push((leaf_id.clone(), unsupported));
                    retained.extend(pending);
                    self.leaf_actions = retained;
                    return Err(NodeRuntimeError::UnsupportedLeafAction {
                        leaf_id,
                        action: action_name,
                    });
                }
            }
        }

        self.leaf_actions = retained;
        Ok(reduced)
    }

    fn connection_for_route(
        &self,
        route: RouteDecision,
    ) -> Option<(ConnectionId, ConnectionGeneration)> {
        match route {
            RouteDecision::Parent => self
                .connections
                .registered_by_direction(ConnectionDirection::Parent)
                .and_then(|connection| {
                    connection
                        .state()
                        .registered()
                        .map(|registered| (connection.id(), registered.generation()))
                }),
            RouteDecision::Child(index) => self
                .endpoint
                .endpoint()
                .child_routes()
                .iter()
                // RouteDecision indexes are compiled from registered children only.
                .filter(|child| child.registered)
                .nth(index)
                .and_then(|child| {
                    self.connections
                        .registered_by_path(ConnectionDirection::Child, &child.path)
                })
                .and_then(|connection| {
                    connection
                        .state()
                        .registered()
                        .map(|registered| (connection.id(), registered.generation()))
                }),
            RouteDecision::Local | RouteDecision::Drop => None,
        }
    }

    fn apply_outcome(
        &mut self,
        outcome: EndpointOutcome,
    ) -> Result<(), NodeRuntimeError<T::Error>> {
        match outcome {
            EndpointOutcome::Forward { route, frame } => self.queue_forward(route, frame),
            EndpointOutcome::Local(event) => {
                self.effects.push(RuntimeEffect::Local(event));
                Ok(())
            }
            EndpointOutcome::Dropped => {
                self.effects.push(RuntimeEffect::Dropped);
                Ok(())
            }
        }
    }

    fn queue_forward(
        &mut self,
        route: RouteDecision,
        frame: FrameBytes,
    ) -> Result<(), NodeRuntimeError<T::Error>> {
        let (connection, generation) = self
            .connection_for_route(route)
            .ok_or(NodeRuntimeError::MissingRouteConnection)?;

        self.effects.push(RuntimeEffect::SendFrame {
            connection,
            generation,
            frame,
        });
        Ok(())
    }

    fn flush_outbound(&mut self) -> Result<usize, NodeRuntimeError<T::Error>> {
        let mut retained = EffectQueue::new();
        let mut sent = 0usize;
        let mut pending = core::mem::take(&mut self.effects);
        let mut drained = pending.drain();
        while let Some(effect) = drained.next() {
            match effect {
                RuntimeEffect::SendFrame {
                    connection,
                    generation,
                    frame,
                } if self
                    .connections
                    .registered(connection)
                    .is_some_and(|registered| registered.generation() == generation) =>
                {
                    if let Err(error) = self.transport.send_frame(connection, &frame) {
                        retained.push(RuntimeEffect::SendFrame {
                            connection,
                            generation,
                            frame,
                        });
                        for remaining in drained {
                            retained.push(remaining);
                        }
                        self.effects = retained;
                        return Err(NodeRuntimeError::Transport(error));
                    }
                    sent += 1;
                }
                RuntimeEffect::SendFrame { .. } => {}
                other => retained.push(other),
            }
        }
        self.effects = retained;
        self.transport
            .flush()
            .map_err(NodeRuntimeError::Transport)?;
        Ok(sent)
    }
}

fn ingress_for(registered: &RegisteredConnection) -> Ingress {
    match registered.direction() {
        ConnectionDirection::Parent => Ingress::Parent,
        ConnectionDirection::Child => Ingress::Child(registered.peer_path().to_vec()),
    }
}

fn local_event_leaf_name(event: &LocalEvent) -> Option<&str> {
    match event {
        LocalEvent::Call { header, .. }
        | LocalEvent::Data { header, .. }
        | LocalEvent::Fault { header, .. } => header.dst_leaf.as_deref(),
    }
}

fn leaf_action_name(action: &LeafAction) -> &'static str {
    match action {
        LeafAction::SendCall(_) => "SendCall",
        LeafAction::SendHookData(_) => "SendHookData",
        LeafAction::FailHook { .. } => "FailHook",
        LeafAction::Connection(_) => "Connection",
    }
}

const fn route_requires_connection(route: RouteDecision) -> bool {
    matches!(route, RouteDecision::Parent | RouteDecision::Child(_))
}

#[cfg(test)]
mod tests {
    use core::cell::RefCell;
    use core::convert::Infallible;

    use crate::alloc::rc::Rc;
    use crate::alloc::string::String;
    use crate::alloc::vec;
    use crate::alloc::vec::Vec;
    use crate::connections::{
        Connection, ConnectionDirection, ConnectionGeneration, ConnectionId, ConnectionState,
        Connections,
    };
    use crate::context::{ConnectionAction, LeafAction, OutboundCall, OutboundHookData};
    use crate::effects::RuntimeEffect;
    use crate::leaf::{Leaf, LeafCapabilities, LeafPermissions};
    use crate::transport::Transport;
    use unshell_protocol::tree::{
        ChildRoute, EndpointError, IncomingCall, LeafSpec, LocalEvent, ProtocolEndpoint,
        RouteDecision,
    };
    use unshell_protocol::{
        CallMessage, FrameBytes, HookTarget, PacketHeader, PacketType, ProtocolFault, decode_frame,
        encode_packet,
    };

    use super::{EndpointState, NodeRuntime, NodeRuntimeError, TickBudget};

    #[derive(Debug, Default)]
    struct RecordingTransport {
        inbound: Option<(ConnectionId, FrameBytes)>,
        sent: Vec<(ConnectionId, FrameBytes)>,
        fail_send: bool,
    }

    #[derive(Debug, Clone, Copy, Eq, PartialEq)]
    struct SendError;

    impl Transport for RecordingTransport {
        type Error = SendError;

        fn poll_recv(&mut self) -> Result<Option<(ConnectionId, FrameBytes)>, Self::Error> {
            Ok(self.inbound.take())
        }

        fn send_frame(
            &mut self,
            connection: ConnectionId,
            frame: &FrameBytes,
        ) -> Result<(), Self::Error> {
            if self.fail_send {
                return Err(SendError);
            }
            self.sent.push((connection, frame.clone()));
            Ok(())
        }
    }

    struct RecordingLeaf {
        capabilities: LeafCapabilities,
        calls: Rc<RefCell<Vec<IncomingCall>>>,
    }

    impl RecordingLeaf {
        fn new(leaf_name: &str, calls: Rc<RefCell<Vec<IncomingCall>>>) -> Self {
            Self {
                capabilities: LeafCapabilities {
                    leaf_name: String::from(leaf_name),
                    procedures: vec![String::from("org.example.v1.echo.invoke")],
                    permissions: LeafPermissions::REPLY_ONLY,
                },
                calls,
            }
        }
    }

    impl Leaf for RecordingLeaf {
        type Error = Infallible;

        fn capabilities(&self) -> &LeafCapabilities {
            &self.capabilities
        }

        fn on_call(
            &mut self,
            ctx: &mut crate::LeafContext<'_>,
            call: IncomingCall,
        ) -> Result<(), Self::Error> {
            self.calls.borrow_mut().push(call.clone());
            ctx.hook_data(OutboundHookData {
                dst_path: call.header.src_path,
                hook_id: 7,
                procedure_id: call.message.procedure_id,
                payload: vec![1, 2, 3],
                end_hook: true,
            })
            .expect("reply-only leaf can queue hook data");
            Ok(())
        }
    }

    struct FailingLeaf {
        capabilities: LeafCapabilities,
    }

    impl FailingLeaf {
        fn new(leaf_name: &str) -> Self {
            Self {
                capabilities: LeafCapabilities {
                    leaf_name: String::from(leaf_name),
                    procedures: vec![String::from("org.example.v1.fail.invoke")],
                    permissions: LeafPermissions::REPLY_ONLY,
                },
            }
        }
    }

    impl Leaf for FailingLeaf {
        type Error = &'static str;

        fn capabilities(&self) -> &LeafCapabilities {
            &self.capabilities
        }

        fn on_call(
            &mut self,
            _ctx: &mut crate::LeafContext<'_>,
            _call: IncomingCall,
        ) -> Result<(), Self::Error> {
            Err("leaf failed")
        }
    }

    #[test]
    fn tick_derives_ingress_and_sends_forwarded_child_frame() {
        let parent = ConnectionId::new(1);
        let child = ConnectionId::new(2);
        let mut connections = Connections::new();
        connections.push(Connection::registered(
            parent,
            ConnectionDirection::Parent,
            vec![],
            ConnectionGeneration::INITIAL,
        ));
        connections.push(Connection::registered(
            child,
            ConnectionDirection::Child,
            vec![String::from("agent"), String::from("grand")],
            ConnectionGeneration::INITIAL,
        ));

        let endpoint = ProtocolEndpoint::new(
            vec![String::from("agent")],
            Some(vec![]),
            vec![ChildRoute::registered(vec![
                String::from("agent"),
                String::from("grand"),
            ])],
            vec![],
        );

        let frame = encode_packet(
            &PacketHeader {
                packet_type: PacketType::Call,
                src_path: vec![],
                dst_path: vec![String::from("agent"), String::from("grand")],
                dst_leaf: None,
                hook_id: None,
            },
            &CallMessage {
                procedure_id: String::from("org.example.v1.echo.invoke"),
                data: vec![],
                response_hook: None,
            },
        )
        .expect("frame encodes");

        let transport = RecordingTransport {
            inbound: Some((parent, frame)),
            sent: Vec::new(),
            fail_send: false,
        };
        let mut runtime = NodeRuntime::new(EndpointState::new(endpoint), connections, transport);

        let outcome = runtime.tick(TickBudget::default()).expect("tick succeeds");

        assert_eq!(outcome.inbound_frames, 1);
        assert_eq!(outcome.outbound_frames, 1);
        assert!(runtime.effects().is_empty());
        assert_eq!(runtime.transport().sent[0].0, child);
    }

    #[test]
    fn runtime_child_registration_updates_connection_and_route_topology() {
        let parent = ConnectionId::new(1);
        let child = ConnectionId::new(2);
        let mut connections = Connections::new();
        connections.push(Connection::connected(parent, ConnectionGeneration::INITIAL));
        connections.push(Connection::connected(child, ConnectionGeneration::INITIAL));

        let endpoint =
            ProtocolEndpoint::new(vec![String::from("agent")], None, Vec::new(), Vec::new());
        let frame = encode_packet(
            &PacketHeader {
                packet_type: PacketType::Call,
                src_path: vec![],
                dst_path: vec![String::from("agent"), String::from("grand")],
                dst_leaf: None,
                hook_id: None,
            },
            &CallMessage {
                procedure_id: String::from("org.example.v1.echo.invoke"),
                data: vec![],
                response_hook: None,
            },
        )
        .expect("frame encodes");
        let transport = RecordingTransport {
            inbound: Some((parent, frame)),
            sent: Vec::new(),
            fail_send: false,
        };
        let mut runtime = NodeRuntime::new(EndpointState::new(endpoint), connections, transport);

        runtime
            .register_parent_connection(parent, vec![], ConnectionGeneration::INITIAL)
            .expect("parent registers");
        runtime
            .register_child_connection(
                child,
                vec![String::from("agent"), String::from("grand")],
                ConnectionGeneration::INITIAL,
            )
            .expect("child registers");

        let outcome = runtime.tick(TickBudget::default()).expect("tick succeeds");

        assert_eq!(outcome.outbound_frames, 1);
        assert_eq!(runtime.transport().sent[0].0, child);
        assert_eq!(
            runtime.endpoint().endpoint().child_routes(),
            [ChildRoute::registered(vec![
                String::from("agent"),
                String::from("grand")
            ])]
        );
    }

    #[test]
    fn connected_child_without_runtime_registration_is_unroutable() {
        let parent = ConnectionId::new(1);
        let child = ConnectionId::new(2);
        let mut connections = Connections::new();
        connections.push(Connection::connected(parent, ConnectionGeneration::INITIAL));
        connections.push(Connection::connected(child, ConnectionGeneration::INITIAL));

        let endpoint = ProtocolEndpoint::new(
            vec![String::from("agent")],
            None,
            vec![ChildRoute::registered(vec![
                String::from("agent"),
                String::from("grand"),
            ])],
            Vec::new(),
        );
        let frame = encode_packet(
            &PacketHeader {
                packet_type: PacketType::Call,
                src_path: vec![],
                dst_path: vec![String::from("agent"), String::from("grand")],
                dst_leaf: None,
                hook_id: None,
            },
            &CallMessage {
                procedure_id: String::from("org.example.v1.echo.invoke"),
                data: vec![],
                response_hook: None,
            },
        )
        .expect("frame encodes");
        let transport = RecordingTransport {
            inbound: Some((parent, frame)),
            sent: Vec::new(),
            fail_send: false,
        };
        let mut runtime = NodeRuntime::new(EndpointState::new(endpoint), connections, transport);
        runtime
            .register_parent_connection(parent, vec![], ConnectionGeneration::INITIAL)
            .expect("parent registers");

        let error = runtime
            .tick(TickBudget::default())
            .expect_err("child is not routable");

        assert!(matches!(error, NodeRuntimeError::MissingRouteConnection));
        assert!(runtime.transport().sent.is_empty());
        assert!(runtime.connections().registered(child).is_none());
    }

    #[test]
    fn child_reregistration_removes_old_route() {
        let child = ConnectionId::new(1);
        let mut connections = Connections::new();
        connections.push(Connection::connected(child, ConnectionGeneration::INITIAL));

        let endpoint =
            ProtocolEndpoint::new(vec![String::from("agent")], None, Vec::new(), Vec::new());
        let transport = RecordingTransport {
            inbound: None,
            sent: Vec::new(),
            fail_send: false,
        };
        let mut runtime = NodeRuntime::new(EndpointState::new(endpoint), connections, transport);

        runtime
            .register_child_connection(
                child,
                vec![String::from("agent"), String::from("old")],
                ConnectionGeneration::INITIAL,
            )
            .expect("old child registers");
        runtime
            .register_child_connection(
                child,
                vec![String::from("agent"), String::from("new")],
                ConnectionGeneration::INITIAL,
            )
            .expect("new child registers");

        assert_eq!(
            runtime.endpoint().endpoint().child_routes(),
            [ChildRoute::registered(vec![
                String::from("agent"),
                String::from("new")
            ])]
        );
        assert!(
            runtime
                .connections()
                .registered_by_path(
                    ConnectionDirection::Child,
                    &[String::from("agent"), String::from("old")],
                )
                .is_none()
        );
    }

    #[test]
    fn replacement_child_registration_demotes_old_peer() {
        let parent = ConnectionId::new(1);
        let old_child = ConnectionId::new(2);
        let new_child = ConnectionId::new(3);
        let mut connections = Connections::new();
        connections.push(Connection::connected(parent, ConnectionGeneration::INITIAL));
        connections.push(Connection::connected(
            old_child,
            ConnectionGeneration::INITIAL,
        ));
        connections.push(Connection::connected(
            new_child,
            ConnectionGeneration::INITIAL,
        ));

        let endpoint =
            ProtocolEndpoint::new(vec![String::from("agent")], None, Vec::new(), Vec::new());
        let transport = RecordingTransport {
            inbound: None,
            sent: Vec::new(),
            fail_send: false,
        };
        let mut runtime = NodeRuntime::new(EndpointState::new(endpoint), connections, transport);

        runtime
            .register_parent_connection(parent, vec![], ConnectionGeneration::INITIAL)
            .expect("parent registers");
        runtime
            .register_child_connection(
                old_child,
                vec![String::from("agent"), String::from("grand")],
                ConnectionGeneration::INITIAL,
            )
            .expect("old child registers");
        runtime
            .register_child_connection(
                new_child,
                vec![String::from("agent"), String::from("grand")],
                ConnectionGeneration::INITIAL,
            )
            .expect("new child replaces old child");

        let frame = encode_packet(
            &PacketHeader {
                packet_type: PacketType::Call,
                src_path: vec![],
                dst_path: vec![String::from("agent"), String::from("grand")],
                dst_leaf: None,
                hook_id: None,
            },
            &CallMessage {
                procedure_id: String::from("org.example.v1.echo.invoke"),
                data: vec![],
                response_hook: None,
            },
        )
        .expect("frame encodes");
        runtime.transport_mut().inbound = Some((parent, frame));

        let outcome = runtime.tick(TickBudget::default()).expect("tick succeeds");

        assert_eq!(outcome.outbound_frames, 1);
        assert_eq!(runtime.transport().sent[0].0, new_child);
        assert!(runtime.connections().registered(old_child).is_none());
    }

    #[test]
    fn invalid_child_registration_leaves_connection_unregistered() {
        let child = ConnectionId::new(1);
        let mut connections = Connections::new();
        connections.push(Connection::connected(child, ConnectionGeneration::INITIAL));

        let endpoint =
            ProtocolEndpoint::new(vec![String::from("agent")], None, Vec::new(), Vec::new());
        let transport = RecordingTransport {
            inbound: None,
            sent: Vec::new(),
            fail_send: false,
        };
        let mut runtime = NodeRuntime::new(EndpointState::new(endpoint), connections, transport);

        let error = runtime
            .register_child_connection(
                child,
                vec![String::from("other"), String::from("kid")],
                ConnectionGeneration::INITIAL,
            )
            .expect_err("invalid child path is rejected");

        assert!(matches!(error, EndpointError::Validation(_)));
        assert!(runtime.connections().registered(child).is_none());
        assert!(runtime.endpoint().endpoint().child_routes().is_empty());
    }

    #[test]
    fn invalid_child_reregistration_preserves_existing_registration() {
        let child = ConnectionId::new(1);
        let mut connections = Connections::new();
        connections.push(Connection::connected(child, ConnectionGeneration::INITIAL));

        let endpoint =
            ProtocolEndpoint::new(vec![String::from("agent")], None, Vec::new(), Vec::new());
        let transport = RecordingTransport {
            inbound: None,
            sent: Vec::new(),
            fail_send: false,
        };
        let mut runtime = NodeRuntime::new(EndpointState::new(endpoint), connections, transport);
        let valid_path = vec![String::from("agent"), String::from("kid")];

        runtime
            .register_child_connection(child, valid_path.clone(), ConnectionGeneration::INITIAL)
            .expect("initial child registers");

        let error = runtime
            .register_child_connection(
                child,
                vec![String::from("other"), String::from("kid")],
                ConnectionGeneration::INITIAL.next(),
            )
            .expect_err("invalid replacement path is rejected");

        assert!(matches!(error, EndpointError::Validation(_)));
        let registered = runtime
            .connections()
            .registered(child)
            .expect("original child remains registered");
        assert_eq!(registered.peer_path(), valid_path);
        assert_eq!(
            runtime.endpoint().endpoint().child_routes(),
            [ChildRoute::registered(valid_path)]
        );
    }

    #[test]
    fn child_route_decision_uses_registered_child_order() {
        let parent = ConnectionId::new(1);
        let unregistered_child = ConnectionId::new(2);
        let registered_child = ConnectionId::new(3);
        let mut connections = Connections::new();
        connections.push(Connection::registered(
            parent,
            ConnectionDirection::Parent,
            vec![],
            ConnectionGeneration::INITIAL,
        ));
        connections.push(Connection::registered(
            unregistered_child,
            ConnectionDirection::Child,
            vec![String::from("agent"), String::from("spare")],
            ConnectionGeneration::INITIAL,
        ));
        connections.push(Connection::registered(
            registered_child,
            ConnectionDirection::Child,
            vec![String::from("agent"), String::from("grand")],
            ConnectionGeneration::INITIAL,
        ));

        let endpoint = ProtocolEndpoint::new(
            vec![String::from("agent")],
            Some(vec![]),
            vec![
                ChildRoute {
                    path: vec![String::from("agent"), String::from("spare")],
                    registered: false,
                },
                ChildRoute::registered(vec![String::from("agent"), String::from("grand")]),
            ],
            vec![],
        );

        let frame = encode_packet(
            &PacketHeader {
                packet_type: PacketType::Call,
                src_path: vec![],
                dst_path: vec![String::from("agent"), String::from("grand")],
                dst_leaf: None,
                hook_id: None,
            },
            &CallMessage {
                procedure_id: String::from("org.example.v1.echo.invoke"),
                data: vec![],
                response_hook: None,
            },
        )
        .expect("frame encodes");

        let transport = RecordingTransport {
            inbound: Some((parent, frame)),
            sent: Vec::new(),
            fail_send: false,
        };
        let mut runtime = NodeRuntime::new(EndpointState::new(endpoint), connections, transport);

        let outcome = runtime.tick(TickBudget::default()).expect("tick succeeds");

        assert_eq!(outcome.outbound_frames, 1);
        assert_eq!(runtime.transport().sent[0].0, registered_child);
    }

    #[test]
    fn receive_keeps_local_events_queued_for_leaf_dispatch() {
        let parent = ConnectionId::new(1);
        let mut connections = Connections::new();
        connections.push(Connection::registered(
            parent,
            ConnectionDirection::Parent,
            vec![],
            ConnectionGeneration::INITIAL,
        ));

        let mut endpoint =
            ProtocolEndpoint::new(vec![String::from("agent")], Some(vec![]), vec![], vec![]);
        endpoint
            .add_endpoint_procedure("org.example.v1.echo.invoke")
            .expect("procedure registers");
        let frame = encode_packet(
            &PacketHeader {
                packet_type: PacketType::Call,
                src_path: vec![],
                dst_path: vec![String::from("agent")],
                dst_leaf: None,
                hook_id: None,
            },
            &CallMessage {
                procedure_id: String::from("org.example.v1.echo.invoke"),
                data: vec![],
                response_hook: None,
            },
        )
        .expect("frame encodes");

        let mut runtime = NodeRuntime::new(
            EndpointState::new(endpoint),
            connections,
            RecordingTransport::default(),
        );

        runtime
            .receive_frame(parent, frame)
            .expect("frame processes");
        assert!(matches!(runtime.effects()[0], RuntimeEffect::Local(_)));
    }

    #[test]
    fn dispatch_local_call_reaches_registered_leaf() {
        let parent = ConnectionId::new(1);
        let mut connections = Connections::new();
        connections.push(Connection::registered(
            parent,
            ConnectionDirection::Parent,
            vec![],
            ConnectionGeneration::INITIAL,
        ));

        let leaf_name = "org.example.v1.echo";
        let endpoint = ProtocolEndpoint::new(
            vec![String::from("agent")],
            Some(vec![]),
            vec![],
            vec![LeafSpec {
                name: String::from(leaf_name),
                procedures: vec![String::from("org.example.v1.echo.invoke")],
            }],
        );
        let frame = encode_packet(
            &PacketHeader {
                packet_type: PacketType::Call,
                src_path: vec![],
                dst_path: vec![String::from("agent")],
                dst_leaf: Some(String::from(leaf_name)),
                hook_id: None,
            },
            &CallMessage {
                procedure_id: String::from("org.example.v1.echo.invoke"),
                data: vec![9],
                response_hook: None,
            },
        )
        .expect("frame encodes");
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut runtime = NodeRuntime::new(
            EndpointState::new(endpoint),
            connections,
            RecordingTransport::default(),
        );
        runtime.register_leaf(RecordingLeaf::new(leaf_name, Rc::clone(&calls)));

        runtime
            .receive_frame(parent, frame)
            .expect("frame processes");
        let dispatched = runtime.dispatch_local_effects().expect("dispatch succeeds");

        assert_eq!(dispatched, 1);
        assert!(runtime.effects().is_empty());
        assert_eq!(calls.borrow().len(), 1);
        assert_eq!(calls.borrow()[0].message.data, [9]);
        assert_eq!(runtime.leaf_actions().len(), 1);
        let (action_leaf, action) = &runtime.leaf_actions()[0];
        assert_eq!(action_leaf.as_str(), leaf_name);
        let LeafAction::SendHookData(data) = action else {
            panic!("leaf action should be retained hook data");
        };
        assert_eq!(data.dst_path, Vec::<String>::new());
        assert_eq!(data.hook_id, 7);
        assert_eq!(data.procedure_id, "org.example.v1.echo.invoke");
        assert_eq!(data.payload, [1, 2, 3]);
        assert!(data.end_hook);
        assert!(runtime.transport().sent.is_empty());
    }

    #[test]
    fn leaf_hook_data_reduces_to_parent_transport_frame() {
        let parent = ConnectionId::new(1);
        let mut connections = Connections::new();
        connections.push(Connection::registered(
            parent,
            ConnectionDirection::Parent,
            vec![],
            ConnectionGeneration::INITIAL,
        ));

        let leaf_name = "org.example.v1.echo";
        let endpoint = ProtocolEndpoint::new(
            vec![String::from("agent")],
            Some(vec![]),
            vec![],
            vec![LeafSpec {
                name: String::from(leaf_name),
                procedures: vec![String::from("org.example.v1.echo.invoke")],
            }],
        );
        let frame = encode_packet(
            &PacketHeader {
                packet_type: PacketType::Call,
                src_path: vec![],
                dst_path: vec![String::from("agent")],
                dst_leaf: Some(String::from(leaf_name)),
                hook_id: None,
            },
            &CallMessage {
                procedure_id: String::from("org.example.v1.echo.invoke"),
                data: vec![9],
                response_hook: Some(HookTarget {
                    hook_id: 7,
                    return_path: vec![],
                }),
            },
        )
        .expect("frame encodes");
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut runtime = NodeRuntime::new(
            EndpointState::new(endpoint),
            connections,
            RecordingTransport::default(),
        );
        runtime.register_leaf(RecordingLeaf::new(leaf_name, Rc::clone(&calls)));

        runtime
            .receive_frame(parent, frame)
            .expect("frame processes");
        runtime.dispatch_local_effects().expect("dispatch succeeds");
        let reduced = runtime.reduce_leaf_actions().expect("hook data reduces");
        let outcome = runtime.tick(TickBudget::default()).expect("tick flushes");

        assert_eq!(reduced, 1);
        assert!(runtime.leaf_actions().is_empty());
        assert_eq!(outcome.outbound_frames, 1);
        assert_eq!(runtime.transport().sent.len(), 1);
        assert_eq!(runtime.transport().sent[0].0, parent);
        let parsed = decode_frame(&runtime.transport().sent[0].1).expect("sent data decodes");
        let header = parsed.header();
        assert_eq!(header.packet_type, PacketType::Data);
        assert_eq!(header.src_path, [String::from("agent")]);
        assert_eq!(header.dst_path, Vec::<String>::new());
        assert_eq!(header.hook_id, Some(7));
        let data = parsed.deserialize_data().expect("payload is data");
        assert_eq!(data.procedure_id, "org.example.v1.echo.invoke");
        assert_eq!(data.data, [1, 2, 3]);
        assert!(data.end_hook);
    }

    #[test]
    fn leaf_fail_hook_reduces_to_parent_fault_frame() {
        let parent = ConnectionId::new(1);
        let mut connections = Connections::new();
        connections.push(Connection::registered(
            parent,
            ConnectionDirection::Parent,
            vec![],
            ConnectionGeneration::INITIAL,
        ));

        let leaf_name = "org.example.v1.echo";
        let endpoint = ProtocolEndpoint::new(
            vec![String::from("agent")],
            Some(vec![]),
            vec![],
            vec![LeafSpec {
                name: String::from(leaf_name),
                procedures: vec![String::from("org.example.v1.echo.invoke")],
            }],
        );
        let frame = encode_packet(
            &PacketHeader {
                packet_type: PacketType::Call,
                src_path: vec![],
                dst_path: vec![String::from("agent")],
                dst_leaf: Some(String::from(leaf_name)),
                hook_id: None,
            },
            &CallMessage {
                procedure_id: String::from("org.example.v1.echo.invoke"),
                data: vec![9],
                response_hook: Some(HookTarget {
                    hook_id: 7,
                    return_path: vec![],
                }),
            },
        )
        .expect("frame encodes");
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut runtime = NodeRuntime::new(
            EndpointState::new(endpoint),
            connections,
            RecordingTransport::default(),
        );
        runtime.register_leaf(RecordingLeaf::new(leaf_name, Rc::clone(&calls)));
        runtime
            .receive_frame(parent, frame)
            .expect("call activates hook");
        runtime.dispatch_local_effects().expect("dispatch succeeds");
        runtime.leaf_actions.clear();
        runtime.leaf_actions.push((
            crate::leaf::LeafId::new(String::from(leaf_name)),
            LeafAction::FailHook {
                hook_id: 7,
                fault: ProtocolFault::INTERNAL_ERROR,
            },
        ));

        let reduced = runtime.reduce_leaf_actions().expect("fault reduces");
        let outcome = runtime.tick(TickBudget::default()).expect("tick flushes");

        assert_eq!(reduced, 1);
        assert!(runtime.leaf_actions().is_empty());
        assert_eq!(outcome.outbound_frames, 1);
        assert_eq!(runtime.transport().sent.len(), 1);
        assert_eq!(runtime.transport().sent[0].0, parent);
        let parsed = decode_frame(&runtime.transport().sent[0].1).expect("fault decodes");
        assert_eq!(parsed.header().packet_type, PacketType::Fault);
        assert_eq!(parsed.header().src_path, [String::from("agent")]);
        assert_eq!(parsed.header().dst_path, Vec::<String>::new());
        assert_eq!(parsed.header().hook_id, Some(7));
        let fault = parsed.deserialize_fault().expect("payload is fault");
        assert_eq!(fault.fault, ProtocolFault::INTERNAL_ERROR);
    }

    #[test]
    fn leaf_send_call_reduces_to_child_transport_frame() {
        let child = ConnectionId::new(1);
        let mut connections = Connections::new();
        connections.push(Connection::registered(
            child,
            ConnectionDirection::Child,
            vec![String::from("agent"), String::from("worker")],
            ConnectionGeneration::INITIAL,
        ));

        let leaf_id = crate::leaf::LeafId::new(String::from("org.example.v1.client"));
        let endpoint = ProtocolEndpoint::new(
            vec![String::from("agent")],
            None,
            vec![ChildRoute::registered(vec![
                String::from("agent"),
                String::from("worker"),
            ])],
            Vec::new(),
        );
        let mut runtime = NodeRuntime::new(
            EndpointState::new(endpoint),
            connections,
            RecordingTransport::default(),
        );
        runtime.leaf_actions.push((
            leaf_id,
            LeafAction::SendCall(OutboundCall {
                dst_path: vec![String::from("agent"), String::from("worker")],
                dst_leaf: Some(String::from("org.example.v1.echo")),
                procedure_id: String::from("org.example.v1.echo.invoke"),
                payload: vec![4, 5, 6],
                expects_response: false,
            }),
        ));

        let reduced = runtime.reduce_leaf_actions().expect("call reduces");
        let outcome = runtime.tick(TickBudget::default()).expect("tick flushes");

        assert_eq!(reduced, 1);
        assert!(runtime.leaf_actions().is_empty());
        assert_eq!(outcome.outbound_frames, 1);
        assert_eq!(runtime.transport().sent.len(), 1);
        assert_eq!(runtime.transport().sent[0].0, child);
        let parsed = decode_frame(&runtime.transport().sent[0].1).expect("sent call decodes");
        let header = parsed.header();
        assert_eq!(header.packet_type, PacketType::Call);
        assert_eq!(header.src_path, [String::from("agent")]);
        assert_eq!(
            header.dst_path,
            [String::from("agent"), String::from("worker")]
        );
        assert_eq!(header.dst_leaf.as_deref(), Some("org.example.v1.echo"));
        let call = parsed.deserialize_call().expect("payload is call");
        assert_eq!(call.procedure_id, "org.example.v1.echo.invoke");
        assert_eq!(call.data, [4, 5, 6]);
        assert!(call.response_hook.is_none());
    }

    #[test]
    fn expected_response_send_call_preflights_route_and_uses_retry_hook() {
        let child = ConnectionId::new(1);
        let mut connections = Connections::new();
        connections.push(Connection::connected(child, ConnectionGeneration::INITIAL));

        let leaf_id = crate::leaf::LeafId::new(String::from("org.example.v1.client"));
        let endpoint = ProtocolEndpoint::new(
            vec![String::from("agent")],
            None,
            vec![ChildRoute::registered(vec![
                String::from("agent"),
                String::from("worker"),
            ])],
            Vec::new(),
        );
        let mut runtime = NodeRuntime::new(
            EndpointState::new(endpoint),
            connections,
            RecordingTransport::default(),
        );
        runtime.leaf_actions.push((
            leaf_id,
            LeafAction::SendCall(OutboundCall {
                dst_path: vec![String::from("agent"), String::from("worker")],
                dst_leaf: Some(String::from("org.example.v1.echo")),
                procedure_id: String::from("org.example.v1.echo.invoke"),
                payload: vec![],
                expects_response: true,
            }),
        ));

        let error = runtime
            .reduce_leaf_actions()
            .expect_err("missing child connection is reported");

        assert!(matches!(error, NodeRuntimeError::MissingRouteConnection));
        assert_eq!(runtime.leaf_actions().len(), 1);
        assert!(runtime.effects().is_empty());

        runtime
            .register_child_connection(
                child,
                vec![String::from("agent"), String::from("worker")],
                ConnectionGeneration::INITIAL,
            )
            .expect("child route restored");
        let reduced = runtime
            .reduce_leaf_actions()
            .expect("retry reduces after route exists");
        let outcome = runtime.tick(TickBudget::default()).expect("tick flushes");

        assert_eq!(reduced, 1);
        assert_eq!(outcome.outbound_frames, 1);
        let parsed = decode_frame(&runtime.transport().sent[0].1).expect("sent call decodes");
        let call = parsed.deserialize_call().expect("payload is call");
        assert_eq!(
            call.response_hook,
            Some(HookTarget {
                hook_id: 1,
                return_path: vec![String::from("agent")],
            })
        );

        let response = encode_packet(
            &PacketHeader {
                packet_type: PacketType::Data,
                src_path: vec![String::from("agent"), String::from("worker")],
                dst_path: vec![String::from("agent")],
                dst_leaf: None,
                hook_id: Some(1),
            },
            &unshell_protocol::DataMessage {
                procedure_id: String::from("org.example.v1.echo.invoke"),
                data: vec![9],
                end_hook: true,
            },
        )
        .expect("response encodes");
        runtime
            .receive_frame(child, response)
            .expect("response hook is accepted");

        assert!(
            matches!(runtime.effects()[0], RuntimeEffect::Local(LocalEvent::Data { ref hook_key, .. }) if hook_key.hook_id == 1)
        );
    }

    #[test]
    fn invalid_send_call_does_not_affect_next_response_hook_id() {
        let child = ConnectionId::new(1);
        let mut connections = Connections::new();
        connections.push(Connection::registered(
            child,
            ConnectionDirection::Child,
            vec![String::from("agent"), String::from("worker")],
            ConnectionGeneration::INITIAL,
        ));

        let leaf_id = crate::leaf::LeafId::new(String::from("org.example.v1.client"));
        let endpoint = ProtocolEndpoint::new(
            vec![String::from("agent")],
            None,
            vec![ChildRoute::registered(vec![
                String::from("agent"),
                String::from("worker"),
            ])],
            Vec::new(),
        );
        let mut runtime = NodeRuntime::new(
            EndpointState::new(endpoint),
            connections,
            RecordingTransport::default(),
        );
        runtime.leaf_actions.push((
            leaf_id.clone(),
            LeafAction::SendCall(OutboundCall {
                dst_path: vec![String::from("agent"), String::from("worker")],
                dst_leaf: Some(String::from("org.example.v1.echo")),
                procedure_id: String::new(),
                payload: vec![],
                expects_response: false,
            }),
        ));

        let error = runtime
            .reduce_leaf_actions()
            .expect_err("invalid procedure is rejected");

        assert!(matches!(error, NodeRuntimeError::Endpoint(_)));
        assert_eq!(runtime.leaf_actions().len(), 1);
        runtime.leaf_actions.clear();
        runtime.leaf_actions.push((
            leaf_id,
            LeafAction::SendCall(OutboundCall {
                dst_path: vec![String::from("agent"), String::from("worker")],
                dst_leaf: Some(String::from("org.example.v1.echo")),
                procedure_id: String::from("org.example.v1.echo.invoke"),
                payload: vec![],
                expects_response: true,
            }),
        ));

        runtime.reduce_leaf_actions().expect("valid retry reduces");
        runtime.tick(TickBudget::default()).expect("tick flushes");

        let parsed = decode_frame(&runtime.transport().sent[0].1).expect("sent call decodes");
        let call = parsed.deserialize_call().expect("payload is call");
        assert_eq!(
            call.response_hook,
            Some(HookTarget {
                hook_id: 1,
                return_path: vec![String::from("agent")],
            })
        );
    }

    #[test]
    fn failed_leaf_send_call_routing_retains_failed_and_remaining_actions() {
        let child = ConnectionId::new(1);
        let mut connections = Connections::new();
        connections.push(Connection::connected(child, ConnectionGeneration::INITIAL));

        let leaf_id = crate::leaf::LeafId::new(String::from("org.example.v1.client"));
        let endpoint = ProtocolEndpoint::new(
            vec![String::from("agent")],
            None,
            vec![ChildRoute::registered(vec![
                String::from("agent"),
                String::from("worker"),
            ])],
            Vec::new(),
        );
        let mut runtime = NodeRuntime::new(
            EndpointState::new(endpoint),
            connections,
            RecordingTransport::default(),
        );
        runtime.leaf_actions.push((
            leaf_id.clone(),
            LeafAction::SendCall(OutboundCall {
                dst_path: vec![String::from("agent"), String::from("worker")],
                dst_leaf: Some(String::from("org.example.v1.echo")),
                procedure_id: String::from("org.example.v1.echo.invoke"),
                payload: vec![],
                expects_response: true,
            }),
        ));
        runtime.leaf_actions.push((
            leaf_id,
            LeafAction::FailHook {
                hook_id: 7,
                fault: ProtocolFault::INTERNAL_ERROR,
            },
        ));

        let error = runtime
            .reduce_leaf_actions()
            .expect_err("missing child connection is reported");

        assert!(matches!(error, NodeRuntimeError::MissingRouteConnection));
        assert_eq!(runtime.leaf_actions().len(), 2);
        assert!(matches!(
            runtime.leaf_actions()[0].1,
            LeafAction::SendCall(_)
        ));
        assert!(matches!(
            runtime.leaf_actions()[1].1,
            LeafAction::FailHook { .. }
        ));
        assert!(runtime.effects().is_empty());
    }

    #[test]
    fn unsupported_connection_action_is_reported_and_retained() {
        let leaf_id = crate::leaf::LeafId::new(String::from("org.example.v1.echo"));
        let mut runtime = NodeRuntime::new(
            EndpointState::new(ProtocolEndpoint::new(
                vec![String::from("agent")],
                Some(vec![]),
                vec![],
                vec![],
            )),
            Connections::new(),
            RecordingTransport::default(),
        );
        runtime.leaf_actions.push((
            leaf_id.clone(),
            LeafAction::Connection(ConnectionAction::Unregister {
                connection: ConnectionId::new(99),
            }),
        ));

        let error = runtime
            .reduce_leaf_actions()
            .expect_err("unsupported action is reported");

        assert!(matches!(
            error,
            NodeRuntimeError::UnsupportedLeafAction { ref leaf_id, action }
                if leaf_id.as_str() == "org.example.v1.echo" && action == "Connection"
        ));
        assert_eq!(runtime.leaf_actions().len(), 1);
        assert!(matches!(
            runtime.leaf_actions()[0].1,
            LeafAction::Connection(_)
        ));
    }

    #[test]
    fn failed_leaf_hook_data_routing_retains_failed_and_remaining_actions() {
        let parent = ConnectionId::new(1);
        let mut connections = Connections::new();
        connections.push(Connection::registered(
            parent,
            ConnectionDirection::Parent,
            vec![],
            ConnectionGeneration::INITIAL,
        ));

        let leaf_name = "org.example.v1.echo";
        let endpoint = ProtocolEndpoint::new(
            vec![String::from("agent")],
            Some(vec![]),
            vec![],
            vec![LeafSpec {
                name: String::from(leaf_name),
                procedures: vec![String::from("org.example.v1.echo.invoke")],
            }],
        );
        let frame = encode_packet(
            &PacketHeader {
                packet_type: PacketType::Call,
                src_path: vec![],
                dst_path: vec![String::from("agent")],
                dst_leaf: Some(String::from(leaf_name)),
                hook_id: None,
            },
            &CallMessage {
                procedure_id: String::from("org.example.v1.echo.invoke"),
                data: vec![],
                response_hook: Some(HookTarget {
                    hook_id: 7,
                    return_path: vec![],
                }),
            },
        )
        .expect("frame encodes");
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut runtime = NodeRuntime::new(
            EndpointState::new(endpoint),
            connections,
            RecordingTransport::default(),
        );
        runtime.register_leaf(RecordingLeaf::new(leaf_name, Rc::clone(&calls)));
        runtime
            .receive_frame(parent, frame)
            .expect("frame processes and activates response hook");
        runtime.dispatch_local_effects().expect("dispatch succeeds");
        runtime.leaf_actions.push((
            crate::leaf::LeafId::new(String::from(leaf_name)),
            LeafAction::FailHook {
                hook_id: 7,
                fault: ProtocolFault::INTERNAL_ERROR,
            },
        ));
        runtime
            .connections
            .get_mut(parent)
            .expect("parent connection exists")
            .set_state(ConnectionState::Connected {
                generation: ConnectionGeneration::INITIAL,
            });

        let error = runtime
            .reduce_leaf_actions()
            .expect_err("missing route connection is reported");

        assert!(matches!(error, NodeRuntimeError::MissingRouteConnection));
        assert_eq!(runtime.leaf_actions().len(), 2);
        assert!(matches!(
            runtime.leaf_actions()[0].1,
            LeafAction::SendHookData(_)
        ));
        assert!(matches!(
            runtime.leaf_actions()[1].1,
            LeafAction::FailHook { .. }
        ));

        runtime
            .register_parent_connection(parent, vec![], ConnectionGeneration::INITIAL)
            .expect("parent route restored");
        let reduced = runtime
            .reduce_leaf_actions()
            .expect("remaining supported actions reduce");

        assert_eq!(reduced, 2);
        assert!(runtime.leaf_actions().is_empty());
        assert!(matches!(
            runtime.effects()[0],
            RuntimeEffect::SendFrame { connection, .. } if connection == parent
        ));
        assert!(matches!(
            runtime.effects()[1],
            RuntimeEffect::SendFrame { connection, .. } if connection == parent
        ));
    }

    #[test]
    fn missing_fail_hook_route_preserves_action_and_hook_for_retry() {
        let parent = ConnectionId::new(1);
        let mut connections = Connections::new();
        connections.push(Connection::registered(
            parent,
            ConnectionDirection::Parent,
            vec![],
            ConnectionGeneration::INITIAL,
        ));

        let leaf_name = "org.example.v1.echo";
        let endpoint = ProtocolEndpoint::new(
            vec![String::from("agent")],
            Some(vec![]),
            vec![],
            vec![LeafSpec {
                name: String::from(leaf_name),
                procedures: vec![String::from("org.example.v1.echo.invoke")],
            }],
        );
        let frame = encode_packet(
            &PacketHeader {
                packet_type: PacketType::Call,
                src_path: vec![],
                dst_path: vec![String::from("agent")],
                dst_leaf: Some(String::from(leaf_name)),
                hook_id: None,
            },
            &CallMessage {
                procedure_id: String::from("org.example.v1.echo.invoke"),
                data: vec![],
                response_hook: Some(HookTarget {
                    hook_id: 7,
                    return_path: vec![],
                }),
            },
        )
        .expect("frame encodes");
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut runtime = NodeRuntime::new(
            EndpointState::new(endpoint),
            connections,
            RecordingTransport::default(),
        );
        runtime.register_leaf(RecordingLeaf::new(leaf_name, Rc::clone(&calls)));
        runtime
            .receive_frame(parent, frame)
            .expect("call activates hook");
        runtime.dispatch_local_effects().expect("dispatch succeeds");
        runtime.leaf_actions.clear();
        runtime.leaf_actions.push((
            crate::leaf::LeafId::new(String::from(leaf_name)),
            LeafAction::FailHook {
                hook_id: 7,
                fault: ProtocolFault::INTERNAL_ERROR,
            },
        ));
        runtime.leaf_actions.push((
            crate::leaf::LeafId::new(String::from(leaf_name)),
            LeafAction::Connection(ConnectionAction::Unregister { connection: parent }),
        ));
        runtime
            .connections
            .get_mut(parent)
            .expect("parent connection exists")
            .set_state(ConnectionState::Connected {
                generation: ConnectionGeneration::INITIAL,
            });

        let error = runtime
            .reduce_leaf_actions()
            .expect_err("missing route connection is reported");

        assert!(matches!(error, NodeRuntimeError::MissingRouteConnection));
        assert_eq!(runtime.leaf_actions().len(), 2);
        assert!(matches!(
            runtime.leaf_actions()[0].1,
            LeafAction::FailHook { .. }
        ));
        assert!(matches!(
            runtime.leaf_actions()[1].1,
            LeafAction::Connection(_)
        ));
        assert!(runtime.effects().is_empty());

        runtime
            .register_parent_connection(parent, vec![], ConnectionGeneration::INITIAL)
            .expect("parent route restored");
        let error = runtime
            .reduce_leaf_actions()
            .expect_err("retry faults hook then stops at connection action");
        let outcome = runtime.tick(TickBudget::default()).expect("tick flushes");

        assert!(matches!(
            error,
            NodeRuntimeError::UnsupportedLeafAction {
                action: "Connection",
                ..
            }
        ));
        assert_eq!(runtime.leaf_actions().len(), 1);
        assert!(matches!(
            runtime.leaf_actions()[0].1,
            LeafAction::Connection(_)
        ));
        assert_eq!(outcome.outbound_frames, 1);
        let parsed = decode_frame(&runtime.transport().sent[0].1).expect("fault decodes");
        assert_eq!(parsed.header().packet_type, PacketType::Fault);
        assert_eq!(parsed.header().hook_id, Some(7));
    }

    #[test]
    fn dropped_fail_hook_route_preserves_action_and_hook_for_retry() {
        let parent = ConnectionId::new(1);
        let mut connections = Connections::new();
        connections.push(Connection::registered(
            parent,
            ConnectionDirection::Parent,
            vec![],
            ConnectionGeneration::INITIAL,
        ));

        let leaf_name = "org.example.v1.echo";
        let endpoint = ProtocolEndpoint::new(
            vec![String::from("agent")],
            Some(vec![]),
            vec![],
            vec![LeafSpec {
                name: String::from(leaf_name),
                procedures: vec![String::from("org.example.v1.echo.invoke")],
            }],
        );
        let frame = encode_packet(
            &PacketHeader {
                packet_type: PacketType::Call,
                src_path: vec![],
                dst_path: vec![String::from("agent")],
                dst_leaf: Some(String::from(leaf_name)),
                hook_id: None,
            },
            &CallMessage {
                procedure_id: String::from("org.example.v1.echo.invoke"),
                data: vec![],
                response_hook: Some(HookTarget {
                    hook_id: 7,
                    return_path: vec![],
                }),
            },
        )
        .expect("frame encodes");
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut runtime = NodeRuntime::new(
            EndpointState::new(endpoint),
            connections,
            RecordingTransport::default(),
        );
        runtime.register_leaf(RecordingLeaf::new(leaf_name, Rc::clone(&calls)));
        runtime
            .receive_frame(parent, frame)
            .expect("call activates hook with dropped return path");
        assert!(matches!(runtime.effects()[0], RuntimeEffect::Local(_)));
        runtime.dispatch_local_effects().expect("dispatch succeeds");
        runtime
            .endpoint
            .endpoint_mut()
            .set_parent_path(None)
            .expect("parent route removes");
        assert_eq!(
            runtime.endpoint.hook_fault_route(7),
            Some(RouteDecision::Drop)
        );
        runtime.leaf_actions.clear();
        runtime.leaf_actions.push((
            crate::leaf::LeafId::new(String::from(leaf_name)),
            LeafAction::FailHook {
                hook_id: 7,
                fault: ProtocolFault::INTERNAL_ERROR,
            },
        ));

        let error = runtime
            .reduce_leaf_actions()
            .expect_err("dropped fault route is reported before mutation");

        assert!(matches!(error, NodeRuntimeError::MissingRouteConnection));
        assert_eq!(runtime.leaf_actions().len(), 1);
        assert!(runtime.effects().is_empty());

        runtime
            .register_parent_connection(parent, vec![], ConnectionGeneration::INITIAL)
            .expect("parent route restored");
        let reduced = runtime
            .reduce_leaf_actions()
            .expect("retained fault retries after route is restored");
        let outcome = runtime.tick(TickBudget::default()).expect("tick flushes");

        assert_eq!(reduced, 1);
        assert_eq!(outcome.outbound_frames, 1);
        assert_eq!(runtime.transport().sent[0].0, parent);
        let parsed = decode_frame(&runtime.transport().sent[0].1).expect("fault decodes");
        assert_eq!(parsed.header().packet_type, PacketType::Fault);
        assert_eq!(parsed.header().hook_id, Some(7));
    }

    #[test]
    fn unmatched_local_event_remains_queued() {
        let mut runtime = NodeRuntime::new(
            EndpointState::new(ProtocolEndpoint::new(
                vec![String::from("agent")],
                Some(vec![]),
                vec![],
                vec![],
            )),
            Connections::new(),
            RecordingTransport::default(),
        );
        runtime.effects.push(RuntimeEffect::Local(LocalEvent::Call {
            header: PacketHeader {
                packet_type: PacketType::Call,
                src_path: vec![],
                dst_path: vec![String::from("agent")],
                dst_leaf: Some(String::from("org.example.v1.missing")),
                hook_id: None,
            },
            message: CallMessage {
                procedure_id: String::from("org.example.v1.missing.invoke"),
                data: vec![],
                response_hook: None,
            },
        }));

        let dispatched = runtime.dispatch_local_effects().expect("dispatch succeeds");

        assert_eq!(dispatched, 0);
        assert_eq!(runtime.effects().len(), 1);
        assert!(matches!(runtime.effects()[0], RuntimeEffect::Local(_)));
    }

    #[test]
    fn local_dispatch_preserves_send_frame_and_dropped_effects() {
        let parent = ConnectionId::new(1);
        let frame = FrameBytes::new();
        let mut runtime = NodeRuntime::new(
            EndpointState::new(ProtocolEndpoint::new(
                vec![String::from("agent")],
                Some(vec![]),
                vec![],
                vec![],
            )),
            Connections::new(),
            RecordingTransport::default(),
        );
        runtime.effects.push(RuntimeEffect::SendFrame {
            connection: parent,
            generation: ConnectionGeneration::INITIAL,
            frame,
        });
        runtime.effects.push(RuntimeEffect::Dropped);

        let dispatched = runtime.dispatch_local_effects().expect("dispatch succeeds");

        assert_eq!(dispatched, 0);
        assert_eq!(runtime.effects().len(), 2);
        assert!(matches!(
            runtime.effects()[0],
            RuntimeEffect::SendFrame { .. }
        ));
        assert!(matches!(runtime.effects()[1], RuntimeEffect::Dropped));
    }

    #[test]
    fn failed_local_dispatch_preserves_failed_and_remaining_effects() {
        let parent = ConnectionId::new(1);
        let leaf_name = "org.example.v1.fail";
        let mut runtime = NodeRuntime::<_, &'static str>::new_with_leaf_error(
            EndpointState::new(ProtocolEndpoint::new(
                vec![String::from("agent")],
                Some(vec![]),
                vec![],
                vec![],
            )),
            Connections::new(),
            RecordingTransport::default(),
        );
        runtime.register_leaf(FailingLeaf::new(leaf_name));
        runtime.effects.push(RuntimeEffect::Local(LocalEvent::Call {
            header: PacketHeader {
                packet_type: PacketType::Call,
                src_path: vec![],
                dst_path: vec![String::from("agent")],
                dst_leaf: Some(String::from(leaf_name)),
                hook_id: None,
            },
            message: CallMessage {
                procedure_id: String::from("org.example.v1.fail.invoke"),
                data: vec![],
                response_hook: None,
            },
        }));
        runtime.effects.push(RuntimeEffect::Dropped);
        runtime.effects.push(RuntimeEffect::SendFrame {
            connection: parent,
            generation: ConnectionGeneration::INITIAL,
            frame: FrameBytes::new(),
        });

        let error = runtime
            .dispatch_local_effects()
            .expect_err("leaf callback failure is returned");

        assert_eq!(error.leaf_id.as_str(), leaf_name);
        assert_eq!(error.source, "leaf failed");
        assert!(runtime.leaf_actions().is_empty());
        assert_eq!(runtime.effects().len(), 3);
        assert!(matches!(runtime.effects()[0], RuntimeEffect::Local(_)));
        assert!(matches!(runtime.effects()[1], RuntimeEffect::Dropped));
        assert!(matches!(
            runtime.effects()[2],
            RuntimeEffect::SendFrame { .. }
        ));
    }

    #[test]
    fn failed_send_preserves_failed_and_unprocessed_effects() {
        let parent = ConnectionId::new(1);
        let mut connections = Connections::new();
        connections.push(Connection::registered(
            parent,
            ConnectionDirection::Parent,
            vec![],
            ConnectionGeneration::INITIAL,
        ));

        let mut endpoint =
            ProtocolEndpoint::new(vec![String::from("agent")], Some(vec![]), vec![], vec![]);
        endpoint
            .add_endpoint_procedure("org.example.v1.echo.invoke")
            .expect("procedure registers");
        let frame = encode_packet(
            &PacketHeader {
                packet_type: PacketType::Call,
                src_path: vec![],
                dst_path: vec![String::from("agent")],
                dst_leaf: None,
                hook_id: None,
            },
            &CallMessage {
                procedure_id: String::from("org.example.v1.echo.invoke"),
                data: vec![],
                response_hook: None,
            },
        )
        .expect("frame encodes");

        let mut runtime = NodeRuntime::new(
            EndpointState::new(endpoint),
            connections,
            RecordingTransport {
                inbound: None,
                sent: Vec::new(),
                fail_send: true,
            },
        );

        runtime.effects.push(RuntimeEffect::SendFrame {
            connection: parent,
            generation: ConnectionGeneration::INITIAL,
            frame: frame.clone(),
        });
        runtime
            .receive_frame(parent, frame.clone())
            .expect("local frame processes");
        runtime.effects.push(RuntimeEffect::SendFrame {
            connection: parent,
            generation: ConnectionGeneration::INITIAL,
            frame,
        });

        let error = runtime.flush_outbound().expect_err("send fails");

        assert!(matches!(error, NodeRuntimeError::Transport(SendError)));
        assert!(runtime.transport().sent.is_empty());
        assert_eq!(runtime.effects().len(), 3);
        assert!(matches!(
            runtime.effects()[0],
            RuntimeEffect::SendFrame { .. }
        ));
        assert!(matches!(runtime.effects()[1], RuntimeEffect::Local(_)));
        assert!(matches!(
            runtime.effects()[2],
            RuntimeEffect::SendFrame { .. }
        ));
    }

    #[test]
    fn tick_counts_only_new_local_events() {
        let parent = ConnectionId::new(1);
        let mut connections = Connections::new();
        connections.push(Connection::registered(
            parent,
            ConnectionDirection::Parent,
            vec![],
            ConnectionGeneration::INITIAL,
        ));

        let mut endpoint =
            ProtocolEndpoint::new(vec![String::from("agent")], Some(vec![]), vec![], vec![]);
        endpoint
            .add_endpoint_procedure("org.example.v1.echo.invoke")
            .expect("procedure registers");
        let frame = encode_packet(
            &PacketHeader {
                packet_type: PacketType::Call,
                src_path: vec![],
                dst_path: vec![String::from("agent")],
                dst_leaf: None,
                hook_id: None,
            },
            &CallMessage {
                procedure_id: String::from("org.example.v1.echo.invoke"),
                data: vec![],
                response_hook: None,
            },
        )
        .expect("frame encodes");

        let transport = RecordingTransport {
            inbound: Some((parent, frame)),
            sent: Vec::new(),
            fail_send: false,
        };
        let mut runtime = NodeRuntime::new(EndpointState::new(endpoint), connections, transport);

        let first = runtime.tick(TickBudget::default()).expect("tick succeeds");
        assert_eq!(first.local_events, 1);
        assert!(matches!(runtime.effects()[0], RuntimeEffect::Local(_)));

        let second = runtime.tick(TickBudget::default()).expect("tick succeeds");
        assert_eq!(second.local_events, 0);
        assert!(matches!(runtime.effects()[0], RuntimeEffect::Local(_)));
    }

    #[test]
    fn drained_local_event_is_not_peeked_or_recounted() {
        let parent = ConnectionId::new(1);
        let mut connections = Connections::new();
        connections.push(Connection::registered(
            parent,
            ConnectionDirection::Parent,
            vec![],
            ConnectionGeneration::INITIAL,
        ));

        let mut endpoint =
            ProtocolEndpoint::new(vec![String::from("agent")], Some(vec![]), vec![], vec![]);
        endpoint
            .add_endpoint_procedure("org.example.v1.echo.invoke")
            .expect("procedure registers");
        let frame = encode_packet(
            &PacketHeader {
                packet_type: PacketType::Call,
                src_path: vec![],
                dst_path: vec![String::from("agent")],
                dst_leaf: None,
                hook_id: None,
            },
            &CallMessage {
                procedure_id: String::from("org.example.v1.echo.invoke"),
                data: vec![],
                response_hook: None,
            },
        )
        .expect("frame encodes");

        let transport = RecordingTransport {
            inbound: Some((parent, frame)),
            sent: Vec::new(),
            fail_send: false,
        };
        let mut runtime = NodeRuntime::new(EndpointState::new(endpoint), connections, transport);

        let first = runtime.tick(TickBudget::default()).expect("tick succeeds");
        assert_eq!(first.local_events, 1);

        let drained: Vec<_> = runtime.drain_local_effects().collect();
        assert_eq!(drained.len(), 1);
        assert!(matches!(drained[0], RuntimeEffect::Local(_)));
        assert!(runtime.effects().is_empty());

        let second = runtime.tick(TickBudget::default()).expect("tick succeeds");
        assert_eq!(second.local_events, 0);
        assert!(runtime.effects().is_empty());
    }

    #[test]
    fn tick_counts_only_new_dropped_frames() {
        let child = ConnectionId::new(1);
        let mut connections = Connections::new();
        connections.push(Connection::registered(
            child,
            ConnectionDirection::Child,
            vec![String::from("agent"), String::from("kid")],
            ConnectionGeneration::INITIAL,
        ));

        let mut endpoint =
            ProtocolEndpoint::new(vec![String::from("agent")], Some(vec![]), vec![], vec![]);
        endpoint
            .add_endpoint_procedure("org.example.v1.echo.invoke")
            .expect("procedure registers");
        let frame = encode_packet(
            &PacketHeader {
                packet_type: PacketType::Call,
                src_path: vec![String::from("agent"), String::from("kid")],
                dst_path: vec![String::from("agent")],
                dst_leaf: None,
                hook_id: None,
            },
            &CallMessage {
                procedure_id: String::from("org.example.v1.echo.invoke"),
                data: vec![],
                response_hook: None,
            },
        )
        .expect("frame encodes");

        let transport = RecordingTransport {
            inbound: Some((child, frame)),
            sent: Vec::new(),
            fail_send: false,
        };
        let mut runtime = NodeRuntime::new(EndpointState::new(endpoint), connections, transport);

        let first = runtime.tick(TickBudget::default()).expect("tick succeeds");
        assert_eq!(first.dropped_frames, 1);
        assert!(matches!(runtime.effects()[0], RuntimeEffect::Dropped));

        let second = runtime.tick(TickBudget::default()).expect("tick succeeds");
        assert_eq!(second.dropped_frames, 0);
        assert!(matches!(runtime.effects()[0], RuntimeEffect::Dropped));
    }

    #[test]
    fn drained_dropped_effect_is_not_peeked_or_recounted() {
        let child = ConnectionId::new(1);
        let mut connections = Connections::new();
        connections.push(Connection::registered(
            child,
            ConnectionDirection::Child,
            vec![String::from("agent"), String::from("kid")],
            ConnectionGeneration::INITIAL,
        ));

        let mut endpoint =
            ProtocolEndpoint::new(vec![String::from("agent")], Some(vec![]), vec![], vec![]);
        endpoint
            .add_endpoint_procedure("org.example.v1.echo.invoke")
            .expect("procedure registers");
        let frame = encode_packet(
            &PacketHeader {
                packet_type: PacketType::Call,
                src_path: vec![String::from("agent"), String::from("kid")],
                dst_path: vec![String::from("agent")],
                dst_leaf: None,
                hook_id: None,
            },
            &CallMessage {
                procedure_id: String::from("org.example.v1.echo.invoke"),
                data: vec![],
                response_hook: None,
            },
        )
        .expect("frame encodes");

        let transport = RecordingTransport {
            inbound: Some((child, frame)),
            sent: Vec::new(),
            fail_send: false,
        };
        let mut runtime = NodeRuntime::new(EndpointState::new(endpoint), connections, transport);

        let first = runtime.tick(TickBudget::default()).expect("tick succeeds");
        assert_eq!(first.dropped_frames, 1);

        let drained: Vec<_> = runtime.drain_local_effects().collect();
        assert_eq!(drained.len(), 1);
        assert!(matches!(drained[0], RuntimeEffect::Dropped));
        assert!(runtime.effects().is_empty());

        let second = runtime.tick(TickBudget::default()).expect("tick succeeds");
        assert_eq!(second.dropped_frames, 0);
        assert!(runtime.effects().is_empty());
    }
}
