//! Single-threaded runtime shell around endpoint packet state.
//!
//! This first slice owns transport and connection metadata, derives ingress from
//! registered connections, delegates packet invariants to [`EndpointState`], and
//! queues concrete runtime effects. Leaf dispatch and leaf-action application are
//! intentionally not implemented in this slice.

use crate::connections::{ConnectionDirection, ConnectionId, Connections, RegisteredConnection};
use crate::effects::{EffectQueue, RuntimeEffect};
use crate::transport::Transport;
use unshell_protocol::FrameBytes;
use unshell_protocol::tree::{EndpointError, EndpointOutcome, Ingress, RouteDecision};

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
        }
    }
}

impl<TransportError> core::error::Error for NodeRuntimeError<TransportError> where
    TransportError: core::error::Error + 'static
{
}

/// Runtime owner for one endpoint, transport, and connection table.
#[derive(Debug)]
pub struct NodeRuntime<T> {
    endpoint: EndpointState,
    connections: Connections,
    transport: T,
    effects: EffectQueue,
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

    /// Returns currently queued effects.
    #[must_use]
    pub fn effects(&self) -> &[RuntimeEffect] {
        self.effects.entries()
    }
}

impl<T> NodeRuntime<T>
where
    T: Transport,
{
    /// Processes one nonblocking runtime step.
    pub fn tick(&mut self, budget: TickBudget) -> Result<TickOutcome, NodeRuntimeError<T::Error>> {
        let mut outcome = TickOutcome::default();

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
            .filter(|effect| matches!(effect, RuntimeEffect::Dropped))
            .count();
        outcome.local_events += self
            .effects
            .entries()
            .iter()
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
        let (connection, generation) = match route {
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
        for effect in self.effects.drain() {
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
                    self.transport
                        .send_frame(connection, frame)
                        .map_err(NodeRuntimeError::Transport)?;
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

#[cfg(test)]
mod tests {
    use crate::alloc::string::String;
    use crate::alloc::vec;
    use crate::alloc::vec::Vec;
    use crate::connections::{
        Connection, ConnectionDirection, ConnectionGeneration, ConnectionId, Connections,
    };
    use crate::effects::RuntimeEffect;
    use crate::transport::Transport;
    use unshell_protocol::tree::{ChildRoute, ProtocolEndpoint};
    use unshell_protocol::{CallMessage, FrameBytes, PacketHeader, PacketType, encode_packet};

    use super::{EndpointState, NodeRuntime, TickBudget};

    #[derive(Debug, Default)]
    struct RecordingTransport {
        inbound: Option<(ConnectionId, FrameBytes)>,
        sent: Vec<(ConnectionId, FrameBytes)>,
    }

    impl Transport for RecordingTransport {
        type Error = core::convert::Infallible;

        fn poll_recv(&mut self) -> Result<Option<(ConnectionId, FrameBytes)>, Self::Error> {
            Ok(self.inbound.take())
        }

        fn send_frame(
            &mut self,
            connection: ConnectionId,
            frame: FrameBytes,
        ) -> Result<(), Self::Error> {
            self.sent.push((connection, frame));
            Ok(())
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
        };
        let mut runtime = NodeRuntime::new(EndpointState::new(endpoint), connections, transport);

        let outcome = runtime.tick(TickBudget::default()).expect("tick succeeds");

        assert_eq!(outcome.inbound_frames, 1);
        assert_eq!(outcome.outbound_frames, 1);
        assert!(runtime.effects().is_empty());
        assert_eq!(runtime.transport().sent[0].0, child);
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
}
