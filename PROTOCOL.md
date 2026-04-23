# UnShell Network Protocol Specification

**Version:** 0.4.0  
**Status:** Draft — implementation in progress  
**Last updated:** 2026-04-22

---

## Core Design Tenants

Two constraints govern every structural decision in this protocol.

**Minimal complexity.** The protocol's minimal form must fit inside shellcode or a small embedded implant. Features that can be implemented as a leaf or an application-layer convention must not be part of the protocol. The protocol exists only to move packets between tree endpoints and to enforce authority relationships at the connection level.

**Extensibility.** The protocol defines a substrate for arbitrary application-layer capabilities. Content types, leaf procedures, and packet payloads are opaque to the router. New capabilities are added by defining new leaves and content types, not by modifying the protocol itself.

When these two principles appear to conflict, prefer the minimal option and delegate complexity to the leaf or application layer.

---

## Glossary

| Term | Definition |
|------|------------|
| **Tree** | The network of all connected endpoints, addressed by path. |
| **Endpoint** | Any node connected to the tree, identified by its registered path. |
| **Leaf** | A hosted service or data object on an endpoint (e.g. a shell session, a file system). Addressed by endpoint path plus leaf name. |
| **Path** | An ordered sequence of segments uniquely identifying an endpoint. Written as `/seg1/seg2` for readability; transmitted as `Vec<String>`. |
| **Actual Authority** | The endpoint that directly admitted another into the tree via the handshake. Has protocol-enforced control over that specific connection only. |
| **Router** | An endpoint that forwards packets rather than handling them. Not a special node type — any endpoint may act as a router. |
| **Hook** | A response channel declared by the authority inside a `CallProcedure` request. The target leaf fires data back through it. |
| **Stream** | A persistent bidirectional data channel established as part of a `Stream`-type hook. |
| **Packet** | A single framed transmission: one header plus one payload. |

---

## Overview

UnShell is a **tree-addressed, authority-hierarchical, message-passing protocol** for command and control (C2) operations.

Endpoints are arranged in a tree. Each endpoint owns a path. A parent endpoint is the actual authority over the children it has directly admitted. Communication is directional: authorities send `Request` packets downward to their clients; clients send data upward exclusively through hooks.

```
/                      ← root (operator or root router)
/abc123                ← endpoint registered under root
/abc123/pivot          ← sub-endpoint registered under /abc123
```

Leaves are addressed by endpoint path plus a leaf name. The notation used throughout this document is:

```
/abc123 { leaf: tty0 }         ← TTY leaf on /abc123
/abc123 { leaf: files }        ← filesystem leaf on /abc123
/abc123/pivot { leaf: tty0 }   ← TTY leaf on /abc123/pivot
```

The `{ leaf: name }` notation is a documentation convention. On the wire, the endpoint path is carried in `dst_path` and the leaf name is carried in the separate `dst_leaf` field of the packet header. Leaf names are not path segments and are invisible to the router.

---

## Authority Model

### Actual Authority

Each connection has exactly one authority and one client. The authority is the endpoint that accepted the connection and ran the handshake. Actual authority grants:

1. The right to admit or reject the client's registration.
2. The right to send unsolicited `Request` packets to the client.
3. The right to declare hooks on the client via a `CallProcedure`.

Actual authority is **per-connection and one hop only**. The root has actual authority over `/abc123` because it directly admitted it. The root does not have actual authority over `/abc123/pivot` — that connection is managed by `/abc123` independently. Routers must reject `Request` packets whose sender is not the direct parent of the destination.

### Hierarchy

Endpoints closer to the root have implied precedence over deeper endpoints they did not directly admit. This is an operational expectation and is not enforced by the protocol. The operator at `/` trusts that `/abc123` will not admit hostile sub-endpoints. Only network architecture and pre-shared secrets can enforce this on the protocol's behalf.

### Cycles

Two endpoints may each be registered in the other's subtree, creating mutual actual authority. This is useful in multi-datacenter topologies where either site should be able to issue commands to the other's endpoints. A compromised node in a cycle has upward reach into the other side; cycles should be created deliberately and documented explicitly in deployment architecture.

---

## Path Conventions

Paths are transmitted as `Vec<String>`. Each element is one segment. Written in this document as `/seg1/seg2` for readability. The router operates on the segment array directly — no string joining or splitting occurs.

Segments beginning with `_` are **protocol-reserved**. External endpoints may not register paths containing `_`-prefixed segments.

| Reserved prefix | Owner | Purpose |
|-----------------|-------|---------|
| `_router` | Router | Built-in router endpoints (e.g. `/_router/nodes`) |

All other path segments are application-defined. Leaf names, hook IDs, and stream IDs are carried in dedicated header fields — not encoded into path segments.

---

## Packet Format

Every transmission is a two-part framed packet:

```
+----------------------------------+------------------------------+
|  Part 1: Header                  |  Part 2: Payload             |
|                                  |                              |
|  [u32 big-endian length]         |  [u32 big-endian length]     |
|  [rkyv-serialised PacketHeader]  |  [rkyv payload bytes]        |
|                                  |                              |
|  Router reads this to route      |  Router forwards opaque      |
+----------------------------------+------------------------------+
```

Both length prefixes are big-endian `u32`. The `packet_type` field in the header fully determines the structure of the payload. The router never inspects the payload — it reads only the header to make all routing decisions.

Packet types are `u16` discriminants produced by rkyv serialisation of the `PacketType` enum. Parsers in any language should treat them as `u16` values matching the discriminants defined below.

### PacketHeader

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct PacketHeader {
    /// Determines the payload type and routing behaviour.
    pub packet_type: PacketType,

    /// Destination endpoint path.
    /// Required for: Request, HookData.
    /// None for: Response (routed by request_id),
    ///           StreamData, StreamClose (routed by stream_id).
    pub dst_path: Option<Vec<String>>,

    /// Destination leaf name. Set when the packet targets a specific leaf
    /// on the destination endpoint. None for packets targeting the endpoint
    /// itself (e.g. GetProcedures, Handshake).
    pub dst_leaf: Option<String>,

    /// Source path of the sender. Used by the router to route responses
    /// and hook data back to the originating endpoint.
    pub src_path: Vec<String>,

    /// Correlation ID for Request / Response pairs.
    /// Set on Request; echoed on Response. None otherwise.
    pub request_id: Option<u64>,

    /// Stream ID for StreamData / StreamClose fastpath routing.
    /// Also set on a CallProcedure that establishes a Stream-type hook,
    /// pre-assigned by the authority. None otherwise.
    pub stream_id: Option<u32>,

    /// Hook ID for HookData packets. Set by the authority when declaring
    /// the hook in a CallProcedure. Used by the receiving authority to
    /// demultiplex incoming hook data. None for non-hook packets.
    pub hook_id: Option<u64>,
}
```

### PacketType → Payload Mapping

Each `PacketType` variant maps to exactly one payload type. The router discards packets with unknown variants without closing the connection.

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum PacketType {
    // -- Handshake -----------------------------------------------------------
    /// Authority → Client. Payload: AuthChallenge
    AuthChallenge  = 0x10,
    /// Client → Authority. Payload: AuthResponse
    AuthResponse   = 0x11,
    /// Client → Authority. Payload: HandshakeMessage
    Handshake      = 0x12,
    /// Authority → Client. Payload: HandshakeAck
    HandshakeAck   = 0x13,

    // -- Request / Response --------------------------------------------------
    /// Authority → Client. Payload: TreeRequest
    Request        = 0x01,
    /// Client → Authority. Payload: TreeResponse
    Response       = 0x02,

    // -- Streams -------------------------------------------------------------
    /// Data on an established stream. Fastpath routed by stream_id.
    /// Payload: raw bytes.
    StreamData     = 0x04,
    /// Closes an established stream. Fastpath routed by stream_id.
    /// Payload: empty.
    StreamClose    = 0x05,

    // -- Hooks ---------------------------------------------------------------
    /// Leaf fires a hook declared by the authority in a prior CallProcedure.
    /// Routed to the authority's base path via dst_path.
    /// Payload: HookDataMessage
    HookData       = 0x06,
}
```

---

## Handshake and Authentication

The handshake is **authority-initiated**. The connecting node does not speak until challenged. If a connecting node sends any packet before receiving `AuthChallenge`, the authority closes the connection immediately without sending a response.

```
Client                                   Authority
  |                                           |
  |──── TCP connect ────────────────────────→ |
  |                                           |
  |←─── AuthChallenge (nonce: [u8;32]) ───────|
  |                                           |
  |──── AuthResponse (hmac: [u8;32]) ───────→ |
  |                                           |
  |──── Handshake (registered_paths) ───────→ |
  |                                           |
  |←─── HandshakeAck (accepted/rejected) ─────|
  |                                           |
  |    [registered; may now send/receive]     |
```

The authority issues a 32-byte random nonce. The client responds with `HMAC-SHA256(pre_shared_secret, nonce)`. The pre-shared secret is provisioned out-of-band. A failed HMAC closes the connection immediately, before any path data is exchanged.

**Timeouts:**
- Client must respond to `AuthChallenge` within 5 seconds.
- Client must send `Handshake` within 10 seconds of sending a valid `AuthResponse`.
- Client must receive `HandshakeAck` within 5 seconds of sending `Handshake`.

### Payload Types

```rust
/// Payload for PacketType::AuthChallenge
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct AuthChallenge {
    pub nonce: [u8; 32],
}

/// Payload for PacketType::AuthResponse
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct AuthResponse {
    /// HMAC-SHA256(pre_shared_secret, nonce)
    pub hmac: [u8; 32],
}

/// Payload for PacketType::Handshake
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct HandshakeMessage {
    /// Paths this node wants to own. Each entry must be a single segment,
    /// exactly one level below the authority's base path.
    /// Segments beginning with `_` are rejected.
    pub registered_paths: Vec<Vec<String>>,

    /// Human-readable label for diagnostics. Stored by the router and
    /// returned via /_router/nodes. Not used for routing.
    /// Cannot be updated after registration.
    pub display_name: Option<String>,
}

/// Payload for PacketType::HandshakeAck
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct HandshakeAck {
    pub accepted: bool,

    /// The canonical base path the authority assigned. May differ from the
    /// requested path if the authority adjusts it (e.g. to avoid collisions).
    pub assigned_base_path: Vec<String>,

    /// Human-readable rejection reason when accepted == false.
    pub rejection_reason: Option<String>,
}
```

**Registration is all-or-nothing.** If any path in `registered_paths` fails validation, the entire handshake is rejected with the reason for the first failed path. Partial registration is not supported.

**Rejection reasons:**

| Reason | Meaning |
|--------|---------|
| `"auth_failed"` | HMAC did not match |
| `"invalid_path"` | A path segment is malformed |
| `"duplicate_path"` | Path already registered by another endpoint |
| `"reserved_segment"` | A segment begins with `_` |
| `"out_of_subtree"` | Requested path is not within the authority's own subtree |

---

## Request / Response

The authority sends a `Request` to an endpoint it has actual authority over. The endpoint replies with a `Response` carrying the same `request_id` in the header.

**Direction enforcement.** A lower-authority endpoint may never send a `Request` to a higher-authority endpoint. All upward data flow goes through hooks. The router rejects `Request` packets whose sender is not the direct parent of the destination, returning `AuthorityViolation` to the sender.

**Response routing.** When the router forwards a `Request`, it records `request_id → src_connection` in an internal request table. When the corresponding `Response` arrives, the router forwards it to the recorded source and removes the entry. A `Response` with an unrecognised `request_id` is discarded with a warning.

**Timeouts.** There is no protocol-level timeout on a `Request` / `Response` pair. The calling endpoint is responsible for implementing application-layer timeouts.

### Payload Types

```rust
/// Payload for PacketType::Request
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct TreeRequest {
    pub request_type: RequestType,
    pub content_type: String,
    pub data: Vec<u8>,

    /// Required for CallProcedure; must be None for all other request types.
    /// If set on a non-CallProcedure request, it is silently ignored.
    pub response_hook: Option<HookTarget>,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum RequestType {
    /// Read the current value or state at this path.
    /// Behavior when targeting a leaf is undefined — see Known Open Problems.
    Read          = 0,
    /// List available leaves and their procedures at this endpoint.
    GetProcedures = 1,
    /// Write a value to this path.
    /// Behavior when targeting a leaf is undefined — see Known Open Problems.
    Write         = 2,
    /// Invoke a named procedure on a leaf. response_hook must be set.
    CallProcedure = 3,
}

/// Payload for PacketType::Response
/// Used for GetProcedures replies and router-generated error responses.
/// CallProcedure always responds via hook and never produces a TreeResponse,
/// except when the router itself cannot resolve the destination path,
/// in which case it returns NoBranchError.
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct TreeResponse {
    pub status: ResponseStatus,
    pub content_type: String,
    pub data: Vec<u8>,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ResponseStatus {
    Ok                 = 0,
    /// Destination path not found at this endpoint or router.
    NoBranchError      = 1,
    /// Operation not supported at this path.
    UnsupportedOp      = 2,
    /// Endpoint-side execution error.
    ExecutionError     = 3,
    /// Request payload was malformed.
    ProtocolError      = 4,
    /// Attempt by a non-parent endpoint to send a Request downward.
    /// See Known Open Problems for enforcement details.
    AuthorityViolation = 5,
}
```

---

## Hooks

A hook is a response channel declared by the authority inside a `CallProcedure` request. There is no separate hook declaration packet — the hook is born inside the call that establishes it.

### Declaration

The authority embeds a `HookTarget` in the `response_hook` field of `TreeRequest` when invoking `CallProcedure`:

```rust
/// Embedded in TreeRequest.response_hook for CallProcedure requests.
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct HookTarget {
    /// Authority-assigned identifier, unique within this authority's active hooks.
    pub hook_id: u64,

    /// The authority's own base path. HookData packets are sent here
    /// and routed by the router using normal path-based routing.
    pub fire_path: Vec<String>,

    /// Advisory declaration of expected response cardinality.
    /// Event: the leaf is expected to fire once and set end_hook = true.
    /// Stream: the leaf is expected to fire repeatedly.
    /// The protocol does not enforce this — end_hook governs actual termination.
    pub response_type: HookResponseType,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum HookResponseType {
    /// Leaf fires exactly once.
    Event  = 0,
    /// Leaf fires repeatedly until end_hook = true.
    Stream = 1,
}
```

`hook_id` is assigned by the authority and must be unique within that authority's set of currently active hooks. The router routes `HookData` to `fire_path` using normal path-based routing (longest-prefix match on `dst_path`). The authority demultiplexes incoming `HookData` packets by `hook_id` from the packet header.

### HookData Payload

```rust
/// Payload for PacketType::HookData
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct HookDataMessage {
    pub content_type: String,
    pub data: Vec<u8>,
    /// When true, the leaf considers this hook complete and will fire
    /// no further HookData on this hook_id. The authority removes the
    /// hook from its table on receipt. No protocol response is required.
    pub end_hook: bool,
}
```

When `end_hook: true` is received, the hook is complete. The authority removes it from its internal hook table. The router may discard any associated routing state for that `hook_id` after forwarding the final packet.

### Hook Cancellation

There is no protocol-level mechanism to cancel a running hook. To abort a streaming hook early, the authority invokes a leaf-defined cancel procedure (e.g. `CallProcedure("halt", {})`). Leaf implementers must provide such a procedure if early cancellation is required.

### Stream Establishment via Hook

When a `CallProcedure` declares a `Stream`-type hook, a bidirectional `StreamData` channel is established as part of the same call. The authority pre-assigns a `stream_id` (a `u32`) and places it in the `PacketHeader` of the `CallProcedure` packet alongside `stream_id`. The router, upon forwarding the `CallProcedure`, registers this `stream_id` in its stream table, mapping it to the pair `(authority_connection, leaf_connection)`.

The stream is considered live when the leaf sends its first `HookData` packet. Both sides may then exchange `StreamData` packets freely using the pre-assigned `stream_id`. The stream is closed when either side sends `StreamClose`, or when `end_hook: true` is received on the associated hook.

The authority is responsible for assigning unique `stream_id` values across all streams it currently manages. Because `stream_id` is a `u32` and the router's stream table is a flat `HashMap<u32, ...>`, values from two different authorities that happen to collide will corrupt each other's streams. Authorities should use random or cryptographically generated `stream_id` values to make collision probability negligible in practice.

---

## Streams

Streams provide a bidirectional, low-overhead data channel between an authority and a leaf. They are established exclusively via the hook mechanism described above. There is no standalone stream-open packet.

`StreamData` payload is raw bytes. `StreamClose` payload is empty.

The router maintains a `HashMap<u32, (ConnectionHandle, ConnectionHandle)>` for active streams. Populated when a `CallProcedure` establishing a `Stream`-type hook is forwarded. Cleared by `StreamClose` or on endpoint disconnect.

If a `StreamData` or `StreamClose` packet arrives with an unrecognised `stream_id` — which may occur in the race window following a payload reconnect, before the stale stream entry has been cleared — the router returns an error to the sender and closes the stream entry if it exists. The sender should treat this as a hard stream termination.

**Flow control.** The protocol has no acknowledgement or backpressure mechanism. Flow control is delegated entirely to the transport. `TcpTransport` inherits TCP's sliding window natively. Any custom transport must implement equivalent backpressure internally. Application-level rate limiting is the responsibility of the leaf implementation.

---

## Leaf System

### Purpose

Leaves are the application layer of the protocol. A leaf represents a remote service or data object hosted on an endpoint: a shell session, a TCP tunnel, a file system, a running process. The protocol places no constraints on what a leaf does or what procedures it exposes.

### Addressing

A leaf is identified by its host endpoint path and its leaf name:

```
/abc123 { leaf: tty0 }         ← leaf tty0 on endpoint /abc123
/abc123 { leaf: files }        ← leaf files on endpoint /abc123
/abc123/pivot { leaf: tty0 }   ← leaf tty0 on endpoint /abc123/pivot
```

On the wire, `dst_path` carries the endpoint path and `dst_leaf` carries the leaf name. The leaf name is not part of the path and is not visible to the router.

### Procedures and Parameters

Leaves expose named procedures invoked via `CallProcedure`. Call parameters directly correspond to the fields of a configurable struct on the leaf.

- Calling a procedure **with parameters** updates those fields and fires the response hook with the resulting state.
- Calling a procedure **without parameters** reads the current state via the hook without modifying anything.

### Leaf Discovery

The authority sends `GetProcedures` to an endpoint's base path (with `dst_leaf: None`) to enumerate all leaves and their procedures:

```
Request
  dst_path:     ["abc123"]
  dst_leaf:     None
  request_type: GetProcedures
  content_type: "core/None"
  data:         []
```

The endpoint replies with a `TreeResponse` whose `data` is an rkyv-serialised `Vec<LeafInfo>`:

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct LeafInfo {
    /// The name of this leaf (e.g. "tty0", "files").
    pub leaf_name: String,
    pub state: LeafState,
    pub procedures: Vec<ProcedureDescriptor>,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct LeafState {
    pub running: bool,
    /// Host process ID. None for leaves that do not correspond to an OS process.
    pub pid: Option<u32>,
    pub error: Option<String>,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum LeafValue {
    Int(i64),
    Bool(bool),
    String(String),
    Bytes(Vec<u8>),
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct ProcedureDescriptor {
    pub name: String,
    pub description: Option<String>,
    /// Parameter names and their current values, in call order.
    pub params: Vec<(String, LeafValue)>,
    /// Advisory: whether the hook fires once or streams repeatedly.
    pub hook_response_type: HookResponseType,
}
```

### Unresolvable CallProcedure

If the router cannot resolve the destination endpoint path, it returns `NoBranchError` to the sender. If the path resolves but the leaf name specified in `dst_leaf` is unknown to the endpoint, the endpoint silently discards the request. In either case, no hook fires. The calling endpoint is responsible for implementing an application-layer timeout.

### Reference Implementation: TTY Leaf

| Procedure | Parameters | Hook Type | Description |
|-----------|-----------|-----------|-------------|
| `start` | `shell: String, rows: u16, cols: u16` | Event | Spawn a PTY with the given config |
| `halt` | — | Event | Kill the PTY process |
| `resize` | `rows: u16, cols: u16` | Event | Update terminal dimensions |
| `state` | — | Event | Read current leaf state without modifying it |
| `stream` | `name: String ("input"\|"output"\|"both")` | Stream | Attach to PTY I/O bidirectionally |

Calling `start` with no arguments uses the leaf's stored defaults. Calling with arguments updates the stored defaults and spawns the process. The `stream` procedure establishes a `Stream`-type hook; the authority must pre-assign a `stream_id` in the `CallProcedure` header as described in the Hooks section.

---

## Path Routing

The router uses three routing mechanisms, selected by `PacketType`.

### 1. Path-Based Routing (Request, HookData)

Longest-prefix match on `dst_path`:

```
Registered paths    Incoming dst_path           Routes to
["abc123"]          ["abc123"]                  → node abc123
["abc123","pivot"]  ["abc123","pivot"]          → node pivot
["_router"]         ["_router","nodes"]         → router built-in
```

**Rules:**
1. Find all registered paths that are a prefix of `dst_path`.
2. Choose the longest matching prefix.
3. No match → return `TreeResponse { status: NoBranchError }` to sender.
4. Tie → route to the most recently registered node.

### 2. Stream ID Fastpath (StreamData, StreamClose)

O(1) lookup in `HashMap<u32, (ConnectionHandle, ConnectionHandle)>`. The entry is created when a `CallProcedure` carrying a `stream_id` is forwarded. The entry is removed on `StreamClose` or on endpoint disconnect. If `stream_id` is not found, the packet is discarded and an error is returned to the sender.

### 3. Response Routing (Response)

When a `Request` is forwarded, the router records `request_id → src_connection`. When the corresponding `Response` arrives, the router forwards it to the recorded source connection and removes the entry. A `Response` with an unrecognised `request_id` is discarded with a warning.

---

## Router Built-in Endpoints

Router built-ins are addressed at `/_router`. Sending any `RequestType` other than the one listed returns `UnsupportedOp`.

| Path | RequestType | Returns |
|------|-------------|---------|
| `/_router/nodes` | `GetProcedures` | All connected endpoints, their registered paths, and their `display_name` values |
| `/_router/ping` | `Read` | `"pong"` as `core/Utf8String` |

---

## Content Types

`content_type` is a free-form string namespaced by module. The protocol does not validate or interpret it — it is an application-layer concern. The router forwards it opaquely as part of the payload.

| Content type | Meaning |
|---|---|
| `"core/None"` | No data |
| `"core/Utf8String"` | Raw UTF-8 string |
| `"core/Bytes"` | Raw bytes |
| `"core/LeafList"` | rkyv `Vec<LeafInfo>` |
| `"tty/Data"` | PTY byte stream |
| `"tty/Output"` | Shell stdout/stderr (UTF-8) |
| `"files/Bytes"` | Raw file contents |

Custom modules use their module name as the namespace: `"mymodule/MyType"`.

---

## Transport Trait

The `Transport` trait abstracts the underlying network mechanism. The protocol layer is fully independent of transport.

```rust
pub trait Transport: Send {
    fn send(&mut self, header: &PacketHeader, payload: &[u8]) -> Result<(), TransportError>;
    fn recv(&mut self) -> Result<(PacketHeader, Vec<u8>), TransportError>;
}

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("header too large: {0} bytes (max {1})")]
    HeaderTooLarge(usize, usize),
    #[error("payload too large: {0} bytes (max {1})")]
    PayloadTooLarge(usize, usize),
    #[error("connection closed cleanly")]
    Disconnected,
    #[error("rkyv deserialisation failed")]
    DeserialiseError,
    #[error("authentication failed")]
    AuthFailed,
}
```

| Transport | Use case |
|-----------|----------|
| `TcpTransport` | Default |
| `TlsTransport` | Encrypted channel |
| `HttpTransport` | Tunnel over HTTP |
| `DnsTransport` | Tunnel over DNS |
| `IcmpTransport` | Tunnel over ICMP |

**Reconnect policy (payloads):** On `Disconnected` or `Io(_)`: close transport, wait 5 seconds, reconnect, run full auth and handshake. No maximum retry limit. All hook and stream state from the previous session is lost on disconnect. The authority must reissue any `CallProcedure` requests whose hooks it still needs after the payload reconnects.

**Reconnect policy (operator CLI):** Print a message and exit. The operator restarts manually.

---

## Packet Size Limits

| Limit | Value | Reason |
|---|---|---|
| Max header length | 64 KB | Anything larger is a bug or attack |
| Max payload length | 64 MB | Sufficient for most single-packet transfers |
| Auth challenge timeout | 5 s (client) | Fail fast on unresponsive authority |
| Auth response timeout | 5 s (authority) | Prevent connection table exhaustion |
| Handshake timeout | 10 s (authority) | After successful auth challenge |
| Handshake ack timeout | 5 s (client) | Keep reconnect loops responsive |

---

## Known Open Problems

The following issues have no clean resolution within the current design. They are documented here so that implementers understand the tradeoffs and can make informed decisions.

---

### 1. `AuthorityViolation` enforcement is unspecified

`ResponseStatus::AuthorityViolation` is defined for the case where a non-parent endpoint attempts to send a `Request` downward past a node it did not directly admit. The spec states that routers must reject such packets, but no mechanism for performing this check is defined.

To enforce this, the router would need to verify the authority relationship between a packet's sender and its destination before forwarding. The information is available in the routing table populated during admission. However, producing `AuthorityViolation` requires the router to generate a `TreeResponse` — an application-layer packet type — which conflicts with the design principle that the router is opaque to payloads and does not generate protocol-level responses of its own.

The options are:

- **Enforce it, accept the exception.** The router generates `AuthorityViolation` as a special case, accepting that this is the one place where the router produces application-layer content. This provides clear operational feedback but breaks the design principle.
- **Drop silently.** Consistent with the behavior of unresolvable `CallProcedure` destinations (no response, timeout at the application layer), but provides no diagnostic signal to a misbehaving or misconfigured endpoint.
- **Remove `AuthorityViolation`.** Drop the status code entirely on the grounds that a correctly implemented client will never send an upward `Request`. The check becomes a deploy-time correctness property rather than a runtime one.

Each option is a legitimate design choice. A decision should be made explicitly before v1.0.

---

### 2. `Read` and `Write` request types have no defined behavior

`RequestType::Read` and `RequestType::Write` are defined in the enum but no section of the spec describes what they do, what `data` should contain, or how an endpoint should respond to them. Every actual leaf interaction in the spec uses `CallProcedure`.

The coherent role for `Read` and `Write` would be for plain data endpoints — nodes that store a value without hosting a full leaf. However, the spec has no concept of a non-leaf data endpoint anywhere else. Introducing one would require defining how such nodes are registered, what types they hold, and how reads and writes are serialised.

The alternative is to remove `Read` and `Write` from `RequestType`, leaving only `GetProcedures` and `CallProcedure`. This is a deliberate narrowing of the protocol's scope: all structured interaction goes through leaves and procedures, and there is no raw read/write layer below that. This is consistent with the complexity budget and the extensibility tenant, but should be an explicit decision rather than an implicit one made by leaving the types undefined.

---

### 3. `LeafInfo` contains overlapping state representations

`GetProcedures` returns a `Vec<LeafInfo>`, each containing a `LeafState` (running, pid, error) and a `Vec<ProcedureDescriptor>` whose `params` fields carry current parameter values. `CallProcedure("state", {})` returns the same information through a hook. There are three overlapping paths to the current state of a leaf, with no stated difference in authority, freshness, or purpose.

The problem is that these serve roles that are conceptually distinct but practically redundant. `GetProcedures` is a discovery call — the operator asks what the leaf can do and what its current configuration is. `LeafState` answers whether the process is alive. `ProcedureDescriptor.params` answers what the current settings are. `CallProcedure("state")` is a live query that returns the same data.

The question is whether discovery should return live state at all. One approach is to separate static schema from dynamic state: `GetProcedures` returns only the shape of the leaf (available procedures, parameter names, and their types) with no live values, and all live state queries go through `CallProcedure`. This removes the redundancy but requires a separate round-trip to learn current state after discovery. The other approach is to keep the current design and simply document that `GetProcedures` returns a snapshot that may be stale by the time it is acted on, and that `CallProcedure("state")` is the authoritative live query. Either is defensible; the spec needs to commit to one.
