# UnShell Runtime API Proposal

This document records the proposed public API direction for the runtime redesign.
The goal is to split packet processing from node orchestration while keeping the
implant-facing runtime single-threaded, explicit, and hard to misuse.

## Goals

- Keep `unshell-protocol` focused on packet types, framing, encoding, decoding,
  and static validation.
- Move endpoint state, routing state, hook state, connection admission, transport
  ownership, leaf dispatch, and scheduling into `unshell-runtime`.
- Run without internal threads. Progress happens only when the caller drives the
  runtime with `tick` or explicit local actions.
- Let every leaf request calls, hook data, faults, and connection changes without
  giving leaves direct access to routes, hooks, endpoint internals, or transports.
- Preserve protocol authority rules by deriving ingress from registered connection
  metadata, never from caller-provided values.
- Keep hot packet paths allocation-aware and move toward borrowed packet/event
  views where the current protocol API permits it.

## Crate Boundary

```text
unshell-protocol
  PacketHeader, CallMessage, DataMessage, FaultMessage
  encode_packet, decode_frame
  validate_header, validate_call, validate_procedure_id
  introspection payload schemas

unshell-runtime
  EndpointState
  NodeRuntime
  Connections
  Transport
  Leaf, LeafContext, LeafAction
  runtime effects and scheduling

unshell
  facade re-exports: protocol, runtime, leaves, macros
```

`EndpointState` is transitional. Today it wraps the existing
`ProtocolEndpoint`. Long term, the endpoint state machine should live in
`unshell-runtime`, while `unshell-protocol` becomes packet-only.

## Transport API

Transports move already-framed protocol packets. They do not know paths, leaves,
hooks, routing, or admission policy.

```rust
pub trait Transport {
    type Error;

    fn poll_recv(&mut self) -> Result<Option<(ConnectionId, FrameBytes)>, Self::Error>;

    fn send_frame(
        &mut self,
        connection: ConnectionId,
        frame: &FrameBytes,
    ) -> Result<(), Self::Error>;

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
```

Rules:

- `poll_recv` must not block.
- `ConnectionId` is a runtime handle, not a protocol path.
- The runtime maps `ConnectionId` to protocol ingress.

## Connection API

Connections are not routable until registered.

```rust
pub struct ConnectionId(u64);
pub struct ConnectionGeneration(u64);

pub enum ConnectionDirection {
    Parent,
    Child,
}

pub struct RegisteredConnection {
    direction: ConnectionDirection,
    peer_path: Vec<String>,
    generation: ConnectionGeneration,
}

pub enum ConnectionState {
    Connected { generation: ConnectionGeneration },
    Authenticating { generation: ConnectionGeneration },
    Registered(RegisteredConnection),
    Draining { generation: ConnectionGeneration },
    Closed { generation: ConnectionGeneration },
}
```

Rules:

- Only `Registered` connections can produce protocol ingress or receive routed
  frames.
- Parent registration must be exactly the direct parent path.
- Child registration must be exactly one segment below the local path.
- Registering or unregistering a connection must update connection state,
  endpoint routes, hook cleanup, and queued generation checks atomically.
- Queued outbound frames carry `ConnectionGeneration`; stale sends are dropped
  when a connection slot is reused.

## Runtime API

`NodeRuntime` owns endpoint packet state, connections, transport, and queued
effects.

```rust
pub struct NodeRuntime<T, LeafError = core::convert::Infallible> {
    endpoint: EndpointState,
    connections: Connections,
    transport: T,
    effects: EffectQueue,
    leaves: Vec<RegisteredLeaf<LeafError>>,
    leaf_actions: Vec<(LeafId, LeafAction)>,
}

pub struct TickBudget {
    pub max_inbound_frames: usize,
    pub flush_outbound: bool,
}

pub struct TickOutcome {
    pub inbound_frames: usize,
    pub outbound_frames: usize,
    pub dropped_frames: usize,
    pub local_events: usize,
}
```

Primary operations:

```rust
impl<T: Transport> NodeRuntime<T> {
    pub fn tick(&mut self, budget: TickBudget) -> Result<TickOutcome, NodeRuntimeError<T::Error>>;

    pub fn receive_frame(
        &mut self,
        connection: ConnectionId,
        frame: FrameBytes,
    ) -> Result<(), NodeRuntimeError<T::Error>>;
}

impl<T, LeafError> NodeRuntime<T, LeafError> {
    pub fn new_with_leaf_error(
        endpoint: EndpointState,
        connections: Connections,
        transport: T,
    ) -> Self;

    pub fn drain_local_effects(&mut self) -> impl Iterator<Item = RuntimeEffect>;

    pub fn register_leaf<L>(&mut self, leaf: L) -> LeafId
    where
        L: Leaf<Error = LeafError> + 'static;

    pub fn dispatch_local_effects(&mut self) -> Result<usize, LeafDispatchError<LeafError>>;

    pub fn reduce_leaf_actions(&mut self) -> Result<usize, NodeRuntimeError<T::Error>>
    where
        T: Transport;

    pub fn drain_leaf_actions(&mut self) -> impl Iterator<Item = (LeafId, LeafAction)>;
}

impl<T> NodeRuntime<T> {
    pub fn register_parent_connection(
        &mut self,
        connection: ConnectionId,
        parent_path: Vec<String>,
        generation: ConnectionGeneration,
    ) -> Result<(), EndpointError>;

    pub fn register_child_connection(
        &mut self,
        connection: ConnectionId,
        child_path: Vec<String>,
        generation: ConnectionGeneration,
    ) -> Result<(), EndpointError>;
}
```

Runtime flow:

```text
transport poll -> (ConnectionId, FrameBytes)
  -> look up registered connection
  -> derive Ingress from registered direction/path
  -> EndpointState::process_frame
  -> RuntimeEffect::SendFrame | RuntimeEffect::Local | RuntimeEffect::Dropped
  -> flush SendFrame effects through Transport
```

Rules:

- Callers never pass `Ingress` into `NodeRuntime`.
- Callers should register parent and child connections through `NodeRuntime` so
  route topology and connection metadata are mutated together. Directly changing
  only `Connections` or only `EndpointState` can leave a connected peer
  unroutable or a route without a registered connection.
- Runtime counts per-tick progress, not retained backlog.
- Local events should be dispatched to leaves, not retained forever.
- `dispatch_local_effects` attempts queued `RuntimeEffect::Local` values in FIFO
  order, calls the matching leaf callback, records queued `LeafAction` values for
  later reducer work, and leaves unmatched locals queued for a future attempt.
- Dispatch does not consume `SendFrame` or `Dropped` effects. Outbound sends remain
  runtime-owned, and drop notifications remain available to callers that drain
  local/drop effects.
- Send failures must not drop unrelated queued effects.

## Leaf API

Leaves are request-only. They can ask the runtime to do work, but cannot mutate
endpoint state, hooks, route tables, connection maps, or transports.

```rust
pub trait Leaf {
    type Error;

    fn capabilities(&self) -> &LeafCapabilities;

    fn on_call(&mut self, ctx: &mut LeafContext<'_>, call: IncomingCall)
        -> Result<(), Self::Error>;

    fn on_data(&mut self, ctx: &mut LeafContext<'_>, data: IncomingData)
        -> Result<(), Self::Error>;

    fn on_fault(&mut self, ctx: &mut LeafContext<'_>, fault: IncomingFault)
        -> Result<(), Self::Error>;

    fn poll(&mut self, ctx: &mut LeafContext<'_>) -> Result<(), Self::Error>;
}
```

Leaf permissions:

```rust
pub struct LeafPermissions {
    pub send_calls: bool,
    pub send_hook_data: bool,
    pub manage_connections: bool,
}
```

Leaf actions:

```rust
pub enum LeafAction {
    SendCall(OutboundCall),
    SendHookData(OutboundHookData),
    FailHook { hook_id: u64, fault: ProtocolFault },
    Connection(ConnectionAction),
}

pub enum ConnectionAction {
    Register {
        connection: ConnectionId,
        direction: ConnectionDirection,
        peer_path: Vec<String>,
    },
    Unregister { connection: ConnectionId },
}
```

Rules:

- A leaf may queue only actions allowed by its `LeafPermissions`.
- Runtime policy still validates every action. Permission is not authority.
- Connection actions request runtime changes. They do not mutate state directly.
- Leaf callbacks must be bounded and nonblocking.
- No nested leaf dispatch. Leaf actions are applied after the callback returns.

## Required Runtime Semantics

### Inbound Forwarding

```text
parent frame for /agent/grand
  -> NodeRuntime derives Ingress::Parent
  -> EndpointState routes to child /agent/grand
  -> RuntimeEffect::SendFrame { connection: grandchild, generation, frame }
  -> Transport::send_frame(grandchild, frame)
```

### Local Call Delivery

```text
parent frame for local endpoint
  -> NodeRuntime derives ingress
  -> EndpointState validates and returns Local(Call)
  -> NodeRuntime dispatches to matching Leaf::on_call
  -> leaf queues LeafAction values
  -> runtime retains actions for a later reducer pass
```

### Outbound Leaf Call

```text
leaf queues LeafAction::SendCall
  -> runtime validates permission and target
  -> EndpointState builds/routes call
  -> pending hook is reserved if needed
  -> RuntimeEffect::SendFrame or RuntimeEffect::Local
```

### Disconnect

```text
connection closes or unregisters
  -> mark connection Draining/Closed and advance generation
  -> remove matching route entries
  -> remove pending hooks associated with peer/subtree
  -> remove active hooks associated with peer/subtree
  -> notify or close leaf sessions
  -> drop queued SendFrame effects with stale generation
```

## Known Gaps In The Current Branch

- `LeafAction::SendCall` and `LeafAction::SendHookData` are reduced by
  `NodeRuntime`; hook fault and connection action variants are still unsupported
  and must remain queued when encountered.
- Hook fault actions through the runtime are not implemented.
- Connection actions through the runtime are not implemented.
- Disconnect does not yet clean hooks, sessions, route state, and queued effects.
- Child ingress still allocates because the existing `Ingress::Child` owns a
  `Vec<String>`.

## Next Implementation Slice

Implement the next narrow leaf-action path:

1. Apply queued `LeafAction::FailHook` through endpoint packet state.
2. Preserve pending/active hook cleanup semantics without dropping unprocessed
   actions.
3. Keep connection registration actions queued until runtime-owned disconnect
   cleanup can update connections, routes, hooks, and queued effects atomically.

That slice should continue the one-variant-at-a-time reducer approach without
implementing connection actions early.
