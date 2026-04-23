# UnShell Network Protocol Specification

**Version:** 0.3.0
**Status:** Draft — implementation in progress
**Last updated:** 2026-04-22

---

## Complexity Budget

The single most important constraint on this protocol is that its minimal form must
fit inside shellcode or a small embedded implant. Every structural decision must be
weighed against this. When in doubt, remove rather than add.

**The rule:** if a feature can be implemented as a leaf or an application-layer
convention, it must not be in the protocol. The protocol exists only to move packets
between tree endpoints and to enforce authority relationships at the connection level.
Everything else belongs above it.

Concretely:
- Authority is encoded in the tree structure alone. No node type annotations.
- Hook declaration is embedded in a `CallProcedure` request, not a separate packet type.
- Call parameters directly correspond to the fields of a configurable struct on the leaf.
- Paths are `Vec<String>` on the wire. The `/`-delimited form used throughout this
  document is a human-readable representation only.

---

## Glossary

| Term | Definition |
|------|------------|
| **Tree** | The network of all connected endpoints, addressed by path. |
| **Endpoint** | Any node connected to the tree, identified by its registered path. Payloads, operators, and routers are all endpoints. |
| **Leaf** | A hosted service or data object living on an endpoint (e.g. a shell session, a TCP tunnel). Accessed via `CallProcedure`. |
| **Path** | An ordered sequence of segments uniquely identifying an endpoint or leaf in the tree. Written as `/seg1/seg2/seg3` for readability; transmitted as `Vec<String>`. |
| **Actual Authority** | The endpoint that directly admitted another into the tree via the handshake. Has protocol-enforced control over that specific connection only. |
| **Implicit Authority** | Endpoints closer to the root have implied precedence over deeper endpoints they did not directly admit. However they will not have the actual ability to send packets to them, since they are not an actual authority. |
| **Router** | An endpoint that forwards packets rather than handling them. Not a special type — any endpoint may act as a router. |
| **Hook** | A pub/sub channel declared by an authority inside a `CallProcedure`. The target endpoint pushes response data back through it. Authority always declares; client always fires. |
| **Stream** | A persistent bidirectional data channel between any two endpoints in the tree. |
| **Packet** | A single framed transmission: one header plus one payload. |
| **Wire** | The packet framing layer — the binary format transmitted over the transport. Paths are `Vec<String>`; packet types are `u16` discriminators. |

---

## Overview

The UnShell protocol is a **tree-addressed, authority-hierarchical, message-passing
protocol** for command and control (C2) operations.

Paths are written in `/`-delimited form for readability throughout this document.
The wire representation is `Vec<String>`. `/abc123/_self/tty0` is the human-readable
form of `["abc123", "_self", "tty0"]`.

```
/                          <- root (operator or root router)
/abc123/                   <- endpoint registered under root
/_self/tty0                <- leaf on the root endpoint itself
/abc123/_self/tty0         <- leaf owned by /abc123/
/abc123/pivot/              <- sub-endpoint registered under /abc123/
/abc123/pivot/_self/files   <- leaf owned by /abc123/pivot/
```

`/_self/<leaf>` denotes leaves hosted by the root endpoint itself. The `_self`
segment always belongs to the endpoint immediately above it in the path.

Every connection has an **authority side** and a **client side**:

- The **authority** decides whether the connecting client is admitted.
- A node may simultaneously be the authority over its children and the client to its parent.
- There is **no monolithic root authority** — each hop enforces its own admission policy.

```
/ (root)
+-- /abc123/           (actual authority over /abc123/ only)
|     +-- /abc123/pivot/   (actual authority over /abc123/pivot/ only)
+-- /xyz456/
```

---

## Authority Model

### Actual Authority

Each connection has exactly one authority and one client. The authority is the
endpoint that accepted the connection and ran the handshake. Actual authority grants:

1. The right to admit or reject the client's registration.
2. The right to send unsolicited `Request` packets to the client.
3. The right to declare hooks on the client via a `CallProcedure`.

Actual authority is **per-connection and one hop only**. `/` has actual authority
over `/abc123/` because it directly admitted it. The root does **not** have actual
authority over `/abc123/pivot/` — that connection is managed by `/abc123/`
independently.

### Implicit Authority

Endpoints closer to the root have **implicit authority** over deeper endpoints they
did not directly admit. This is not enforced by the protocol. It is an operational
expectation: the operator at `/` trusts that `/abc123/` will not admit hostile
sub-endpoints. The protocol cannot enforce this on its behalf — only network
architecture and pre-shared secrets can.

### Cycles

Two endpoints may each be registered in the other's subtree, creating mutual actual
authority. This is useful in multi-datacenter topologies where either site should be
able to admit the other's endpoints. A compromised node in a cycle has upward reach
into the other side; cycles should be created deliberately, not accidentally.

---

## Path Conventions

**On the wire:** paths are `Vec<String>`. Each element is one segment. The router
operates on this array directly — no string joining or splitting.

**In documentation:** paths are written as `/seg1/seg2/seg3` for readability.

Segments beginning with `_` are **protocol-owned**. External endpoints may not
register paths containing `_`-prefixed segments.

| Path | Meaning |
|------|---------|
| `/abc123/` | Endpoint registered directly under root |
| `/abc123/pivot/` | Endpoint registered under `/abc123/` |
| `/_self/tty0` | Leaf `tty0` on the root endpoint itself |
| `/abc123/_self/` | Leaf namespace owned by `/abc123/` |
| `/abc123/_self/tty0` | A TTY leaf on `/abc123/` |
| `/abc123/_self/tty1` | A second TTY instance on `/abc123/` |
| `/abc123/_hooks/` | Hook fire targets owned by `/abc123/`. Reserved. |

### Flat Leaves

All leaves are flat inside `_self/`. There is no sub-hierarchy within `_self/`.
Instance identifiers (e.g. `tty0`, `tty1`) are part of the leaf name string, not
a separate path level. `/abc123/_self/tty/0` is **invalid**; the correct form is
`/abc123/_self/tty0`.

---

## Packet Format

Every transmission is a **two-part framed packet**:

```
+-----------------------------------------+------------------------------+
|  Part 1: Header                         |  Part 2: Payload             |
|                                         |                              |
|  [u32 big-endian length]                |  [u32 big-endian length]     |
|  [rkyv-serialised PacketHeader bytes]   |  [rkyv payload bytes]        |
|                                         |                              |
|  Router reads this to route the packet  |  Router forwards opaque       |
+-----------------------------------------+------------------------------+
```

Both length fields are big-endian `u32`. See §Packet Size Limits for enforced maxima.

The `packet_type` field in the header **fully determines the structure of the payload**.
The router never inspects the payload; it reads only the header.

### What's on the wire

- Paths are `Vec<String>` — not `/`-delimited strings
- Packet types are `u16` discriminators — not string enums
- The router operates on path segments directly, not string prefix matching

### What's NOT on the wire

This document uses `/`-delimited path notation for readability only. The application
layer converts to/from `Vec<String>` for transmission.

### PacketHeader

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct PacketHeader {
    /// Determines the payload type and routing behaviour.
    pub packet_type: PacketType,

    /// Destination path. Required for Request and StreamOpen.
    /// None for Response (routed by request_id),
    ///       StreamData, StreamClose (routed by stream_id),
    ///       HookData, HookClose (routed by hook_id).
    pub dst_path: Option<Vec<String>>,

    /// Source path of the sender. Used to route responses and hook data back.
    pub src_path: Vec<String>,

    /// Correlation ID for Request / Response pairs. None otherwise.
    pub request_id: Option<u64>,

    /// Stream ID for StreamData / StreamClose fastpath routing. None otherwise.
    pub stream_id: Option<u16>,

    /// Hook ID for HookData / HookClose. Embedded in the CallProcedure that
    /// declared the hook. None for non-hook packets.
    pub hook_id: Option<u64>,
}
```

### PacketType -> Payload Mapping

Each `PacketType` variant maps to exactly one payload type. The router discards
packets with unknown variants without closing the connection.

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum PacketType {
    // -- Handshake ------------------------------------------------------------
    /// Authority -> Client. Payload: AuthChallenge
    AuthChallenge  = 0x10,
    /// Client -> Authority. Payload: AuthResponse
    AuthResponse   = 0x11,
    /// Client -> Authority. Payload: HandshakeMessage
    Handshake      = 0x12,
    /// Authority -> Client. Payload: HandshakeAck
    HandshakeAck   = 0x13,

    // -- Request / Response ---------------------------------------------------
    /// Authority -> Client. Payload: TreeRequest
    Request        = 0x01,
    /// Client -> Authority. Payload: TreeResponse
    Response       = 0x02,

    // -- Streams --------------------------------------------------------------
    /// Open a persistent bidirectional channel. Payload: StreamOpenRequest
    StreamOpen     = 0x03,
    /// Data over an established stream (fastpath by stream_id). Payload: raw bytes
    StreamData     = 0x04,
    /// Close a stream. Payload: empty
    StreamClose    = 0x05,

    // -- Hooks ----------------------------------------------------------------
    /// Client fires a hook declared by the authority in a prior CallProcedure.
    /// Payload: HookDataMessage  [PROVISIONAL - see Hooks section]
    HookData       = 0x06,
    /// Either side tears down the hook.
    /// Payload: HookCloseMessage  [PROVISIONAL - see Hooks section]
    HookClose      = 0x07,
}
```

---

## Handshake and Authentication

The handshake is **authority-initiated**. The connecting node does not speak until
challenged.

```
Client (connecting node)               Authority (parent endpoint / router)
 |                                              |
 |---- TCP connect --------------------------->|
 |                                              |
 |<--- AuthChallenge (nonce: [u8;32]) ----------|
 |                                              |
 |---- AuthResponse (hmac: [u8;32]) ---------->|
 |                                              |
 |---- Handshake (registered_paths) ---------->|
 |                                              |
 |<--- HandshakeAck (accepted, base_path) ------|
 |                                              |
 |  [registered; may now send and receive]      |
```

The authority issues a 32-byte random nonce. The client responds with
`HMAC-SHA256(pre_shared_secret, nonce)`. The pre-shared secret is provisioned
out-of-band. A failed HMAC closes the connection before any path data is exchanged.

**Timeouts:**
- Client must respond to `AuthChallenge` within 5 seconds.
- Client must send `Handshake` within 10 seconds of a valid `AuthResponse`.
- Client must receive `HandshakeAck` within 5 seconds of sending `Handshake`.

### Handshake Payload Types

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
    /// Paths this node wants to own. Each must be exactly one level below the
    /// authority's base path. Segments beginning with `_` are rejected.
    /// Example: registering /abc123/ under root sends [["abc123"]].
    pub registered_paths: Vec<Vec<String>>,

    /// Human-readable label for diagnostics. Not used for routing.
    pub display_name: Option<String>,
}

/// Payload for PacketType::HandshakeAck
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct HandshakeAck {
    pub accepted: bool,

    /// The canonical base path the authority assigned. May differ from the
    /// requested path if the authority adjusts it.
    pub assigned_base_path: Vec<String>,

    /// Human-readable rejection reason when accepted == false.
    pub rejection_reason: Option<String>,
}
```

**Rejection reasons:**
- `"auth_failed"` — HMAC did not match
- `"invalid_path"` — a path segment is malformed
- `"duplicate_path"` — path already registered by another endpoint
- `"reserved_segment"` — a segment begins with `_`
- `"out_of_subtree"` — requested path is not within the authority's own subtree

---

## Request / Response

The authority sends a `Request` to an endpoint it has actual authority over.
The endpoint replies with a `Response` carrying the same `request_id`.

A lower-authority endpoint **may never send a `Request` to a higher**.
All upward data flow goes through hooks.

### Payload Types

```rust
/// Payload for PacketType::Request
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct TreeRequest {
    pub request_id: u64,
    pub request_type: RequestType,
    pub content_type: String,
    pub data: Vec<u8>,

    /// For CallProcedure only: the hook the leaf should fire with its result.
    /// Must be set for CallProcedure; must be None for all other request types.
    pub response_hook: Option<HookTarget>,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum RequestType {
    /// Read the current value or state at this path.
    Read          = 0,
    /// List available leaves and procedures at this path.
    GetProcedures = 1,
    /// Write a value to this path.
    Write         = 2,
    /// Invoke a named procedure on a leaf. response_hook must be set.
    CallProcedure = 3,
}

/// Declares the hook the authority has prepared to receive Call results.
/// Embedded in the CallProcedure request itself - no separate packet needed.
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct HookTarget {
    /// Authority-assigned ID for this hook.
    pub hook_id: u64,

    /// Path the leaf should address HookData packets to.
    /// Always within the authority's own /_hooks/ namespace.
    /// Example: /op/_hooks/7 is transmitted as ["op", "_hooks", "7"]
    pub fire_path: Vec<String>,

    /// Whether the hook carries a single event or an ongoing stream of data.
    pub response_type: HookResponseType,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum HookResponseType {
    /// Leaf fires exactly once and sets end_hook = true.
    Event  = 0,
    /// Leaf fires repeatedly; authority receives a stream of HookData packets.
    Stream = 1,
}

/// Payload for PacketType::Response
/// Used for Read, GetProcedures, and Write replies only.
/// CallProcedure always responds via hook; it never produces a TreeResponse.
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct TreeResponse {
    pub request_id: u64,
    pub status: ResponseStatus,
    pub content_type: String,
    pub data: Vec<u8>,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ResponseStatus {
    Ok                 = 0,
    /// Destination path not found at this endpoint.
    NoBranchError      = 1,
    /// Operation not supported at this path.
    UnsupportedOp      = 2,
    /// Endpoint-side execution error.
    ExecutionError     = 3,
    /// Request payload was malformed.
    ProtocolError      = 4,
    /// Attempt by a lower endpoint to initiate contact upward.
    AuthorityViolation = 5,
}
```

---

## Streams

Streams are **not restricted to authority/client pairs**. Any two endpoints that
share a common router ancestor may open a stream to each other.

`StreamOpen` is sent by the initiating endpoint. The router assigns a `stream_id`,
registers the pair in its stream table, and returns a `Response` containing the ID.
Both sides exchange `StreamData` packets freely until either side sends `StreamClose`.

```
Initiator                                     Recipient
    |                                              |
    |---- StreamOpen (dst: /abc123/_self/tty0) --->|
    |<--- Response (stream_id = 42) ---------------|
    |                                              |
    |<--> StreamData (stream_id = 42) <----------->|  (bidirectional)
    |                                              |
    |---- StreamClose (stream_id = 42) ----------->|
```

```rust
/// Payload for PacketType::StreamOpen
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct StreamOpenRequest {
    pub label: Option<String>,
}
```

`StreamData` payload is raw bytes. `StreamClose` payload is empty.

The router maintains a `HashMap<u16, (NodeHandle, NodeHandle)>` for active streams.
`StreamOpen` and `StreamClose` update this table. `StreamData` uses it for O(1)
fastpath routing with no path matching.

---

## Hooks

// TODO: Hook packet semantics are not yet fully defined.
//
// Known constraints:
// - Hooks are declared by the authority inside a CallProcedure (via the
//   response_hook field in TreeRequest). There is no dedicated declare packet.
// - The fire_path in HookTarget is always within the authority's _hooks/ namespace,
//   e.g. /op/_hooks/7.
// - HookData packets carry an end_hook flag signalling the hook is complete.
// - HookData may arrive once (Event) or many times (Stream), depending on the
//   HookResponseType declared in the CallProcedure.
// - HookClose may be sent by either side to tear down the hook early.
// - Payload structures below are provisional and subject to change.

```rust
/// Payload for PacketType::HookData  [PROVISIONAL]
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct HookDataMessage {
    pub hook_id: u64,
    pub content_type: String,
    pub data: Vec<u8>,
    /// If true, the client considers this hook complete.
    /// The authority should send HookClose to confirm teardown.
    pub end_hook: bool,
}

/// Payload for PacketType::HookClose  [PROVISIONAL]
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct HookCloseMessage {
    pub hook_id: u64,
    pub reason: Option<String>,
}
```

---

## Leaf System

### Purpose

Leaves are where the protocol's application-layer complexity lives. A leaf represents
a remote service or data object hosted on an endpoint: a shell session, a TCP tunnel,
a log queue, a running process.

### Paths

All leaves are flat inside `_self/`. There is no sub-hierarchy within `_self/`.
Instance identifiers are part of the leaf name string, not a separate path level.
`/abc123/_self/tty/0` is **invalid**; the correct form is `/abc123/_self/tty0`.

```
/_self/tty0            <- TTY leaf on the root endpoint
/abc123/_self/tty0     <- TTY leaf on /abc123/
/abc123/_self/tty1     <- second TTY instance on /abc123/
/abc123/_self/files    <- filesystem leaf on /abc123/
/abc123/_self/tcp0     <- TCP tunnel leaf on /abc123/
```

### Calls and Configurable Structs

Call parameters directly correspond to the fields of the leaf's configurable struct.
There is no separate `config.get` / `config.set` mechanism.

- Calling a procedure **with parameters** updates those fields and fires the response
  hook with the resulting state.
- Calling a procedure **with no parameters** reads current state via the hook without
  changing anything.

```
// Set rows and cols, receive updated state:
CallProcedure("resize", { rows: 40, cols: 120 })
  -> hook fires: { state: Running, rows: 40, cols: 120 }

// Read current state without changing anything:
CallProcedure("state", {})
  -> hook fires: { state: Running, pid: 42, rows: 24, cols: 80 }
```

### Leaf Data Types

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct LeafState {
    pub running: bool,
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
    /// Whether the hook fires once or streams many times.
    pub hook_response_type: HookResponseType,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct LeafInfo {
    /// Absolute path of this leaf, e.g. /abc123/_self/tty0
    pub path: Vec<String>,
    pub state: LeafState,
    pub procedures: Vec<ProcedureDescriptor>,
}
```

### Leaf Discovery

The authority sends `GetProcedures` to the endpoint's base path:

```
REQUEST  dst: /abc123/
  request_type: GetProcedures
  content_type: "core/None"

RESPONSE
  status: Ok
  content_type: "core/LeafList"
  data: rkyv(Vec<LeafInfo>) = [
    LeafInfo {
      path: /abc123/_self/tty0,
      state: Running { pid: 1234 },
      procedures: [
        { name: "start",  params: [("shell","/bin/sh"),("rows",24),("cols",80)], hook_response_type: Event  },
        { name: "halt",   params: [],                                            hook_response_type: Event  },
        { name: "resize", params: [("rows",24),("cols",80)],                    hook_response_type: Event  },
        { name: "state",  params: [],                                            hook_response_type: Event  },
        { name: "stream", params: [("name","both")],                            hook_response_type: Stream },
      ],
    },
    LeafInfo {
      path: /abc123/_self/files,
      ...
    }
  ]
```

### Reference Implementation: TTY Leaf

Path: `/<endpoint>/_self/tty<id>`

| Procedure | Parameters | Hook Type | Description |
|-----------|-----------|-----------|-------------|
| `start` | `shell: String, rows: u16, cols: u16` | Event | Spawn a PTY with given config |
| `halt` | - | Event | Kill the PTY process |
| `resize` | `rows: u16, cols: u16` | Event | Update terminal dimensions |
| `state` | - | Event | Read current state |
| `stream` | `name: String ("input"|"output"|"both")` | Stream | Attach to PTY I/O |

Calling `start` with no arguments uses the leaf's stored defaults. Calling with
arguments both updates the stored defaults and spawns the process.

---

## Path Routing

The router uses two routing methods.

### 1. Path-Based Routing (Request, StreamOpen)

Longest-prefix match on `dst_path`:

```
Registered paths     Incoming dst_path             Routes to
/abc123/             /abc123/_self/tty0            -> node abc123
/xyz456/             /xyz456/_self/files           -> node xyz456
/abc123/pivot/       /abc123/pivot/_self/tcp0      -> node pivot
/_router/            /_router/nodes                -> router built-in
```

**Rules:**
1. Find all nodes whose registered path is a prefix of `dst_path`.
2. Choose the longest matching prefix.
3. No match -> return `TreeResponse { status: NoBranchError }` to sender.
4. Tie -> route to the most recently registered node.

### 2. Stream ID Fastpath (StreamData, StreamClose)

O(1) lookup in `HashMap<u16, (NodeHandle, NodeHandle)>`. Populated by `StreamOpen`,
cleared by `StreamClose`. If the `stream_id` is not found, the packet is discarded
with a warning.

### 3. Hook ID Routing (HookData, HookClose)

// TODO: Exact routing mechanism depends on finalised hook semantics.
// Provisional: the router maintains a HashMap<u64, fire_path> populated when a
// CallProcedure with response_hook is routed through. HookData packets are
// forwarded to the recorded fire_path.

---

## Router Built-in Endpoints

| Path | RequestType | Returns |
|------|-------------|---------|
| `/_router/nodes` | `GetProcedures` | All connected endpoints with their registered paths |
| `/_router/ping` | `Read` | `"pong"` |

---

## Content Types

| Content type | Meaning |
|---|---|
| `"core/None"` | No data |
| `"core/Utf8String"` | Raw UTF-8 string |
| `"core/Bytes"` | Raw bytes |
| `"core/LeafList"` | rkyv `Vec<LeafInfo>` |
| `"shell/Output"` | Shell stdout+stderr (UTF-8) |
| `"files/Bytes"` | Raw file contents |
| `"tty/Data"` | PTY byte stream |

Custom modules use their module name as the namespace: `"mymodule/MyType"`.

---

## Real-World Scenario Analysis

### Scenario 1: Flaky Network / Payload Reconnect

On disconnect, the payload closes the transport, waits 5 seconds, and attempts
reconnect. The authority removes the payload's registered paths from its routing
table. On reconnect, the full auth challenge -> handshake sequence runs again. Any
hooks embedded in prior RPC calls are lost; the authority must reissue any
`CallProcedure` requests whose hooks it still needs.

### Scenario 2: Operator Disconnects Mid-Session

Payloads remain connected. Their leaves keep running. In-flight `HookData` packets
targeting the operator's `/_hooks/` path are discarded when that path is
deregistered. On reconnect the operator gets a new session path and reissues any
calls it needs. The payload is the persistent state; the operator is ephemeral.

### Scenario 3: Multiple Operators

Both operators have separate session paths and separate `/_hooks/` namespaces.
Both may independently issue `CallProcedure` to the same leaf. The leaf processes
requests sequentially; concurrent requests from different operators do not collide
provided they carry different hook IDs. No access control in this version -- any
authenticated endpoint may address any reachable path.

### Scenario 4: Large Data Transfer (File Exfiltration)

Issue a `CallProcedure` with `hook_response_type: Stream`. The leaf fires sequential
`HookData` packets with successive chunks of the file. The authority reassembles
the chunks. No buffering required on the router. Practical limit per individual
packet is ~50 MB.

### Scenario 5: AV / EDR Detection

The `Transport` trait abstracts this entirely. Swapping `TcpTransport` for
`TlsTransport` or `HttpTransport` requires no changes to the protocol layer.

### Scenario 6: Router Crash / Restart

All state is in-memory. On restart, all endpoints receive `Disconnected`, enter
their reconnect loops, and re-authenticate and re-register. The router comes up
with a clean routing table.

### Scenario 7: Malformed Packet / Bad Actor

The authority challenges before accepting any path data. A node that fails HMAC
is dropped before it can register. After auth, length-prefix enforcement prevents
oversized packets. Malformed rkyv bytes return `DeserialiseError` and close the
connection. Unknown `dst_path` values return `NoBranchError` to the sender.

### Scenario 8: Pivot / Multi-Hop

A node simultaneously connects to an external router as a client and listens for
incoming connections as a local authority. It registers `/pivot/` on the external
router and admits sub-agents under `/pivot/sub1/` on its local side.

```
External router (/)
  +-- /pivot/
        +-- /pivot/sub1/   <- admitted by pivot's local authority
```

The external operator addresses `/pivot/sub1/_self/tty0`. The external router
longest-prefix matches to `/pivot/` and forwards to the pivot. The pivot runs a
second longest-prefix match locally and forwards to `/pivot/sub1/`. No special
endpoint type is required -- the pivot is simply an endpoint that also runs a
router loop.

### Scenario 9: Authority Cycle

```
/dc-a/dc-b/  <- dc-b admitted by dc-a (actual authority)
/dc-b/dc-a/  <- dc-a admitted by dc-b (actual authority, cycle)
```

Both sides now have mutual actual authority. The protocol does not prohibit this.
It is useful in multi-datacenter topologies where either site can issue commands to
the other. A compromised node in a cycle has upward reach into the other side.

### Scenario 10: Reverse Shell via Leaf

```
1. Authority sends CallProcedure("start", {}) to /abc123/_self/tty0
   with response_hook = { hook_id: 1, fire_path: /op/_hooks/1, response_type: Event }.
   Leaf spawns PTY, fires HookData(hook_id=1, { Running, pid: 42 }, end_hook=true).
   Authority sends HookClose(1).

2. Authority sends CallProcedure("stream", { name: "both" })
   with response_hook = { hook_id: 2, fire_path: /op/_hooks/2, response_type: Stream }.
   Leaf fires HookData packets containing PTY output as it arrives.
   Authority sends keystrokes back via the bidirectional stream.

3. Authority sends HookClose(2) when done.
   Authority sends CallProcedure("halt", {}) to kill the PTY.
```

---

## Transport Trait

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
| `TlsTransport` | Encrypted, looks like HTTPS |
| `HttpTransport` | Tunnel over HTTP |
| `DnsTransport` | Tunnel over DNS |
| `IcmpTransport` | Tunnel over ICMP |

**Reconnect policy (payloads):** On `Disconnected` or `Io(_)`: close transport,
wait 5 s, reconnect, run full auth + handshake. No maximum retry limit.

**Reconnect policy (operator CLI):** Print message and exit. Operator restarts manually.

---

## Packet Size Limits

| Limit | Value | Reason |
|---|---|---|
| Max header length | 64 KB | Anything larger is a bug or attack |
| Max payload length | 64 MB | Sufficient for most transfers |
| Auth challenge timeout | 5 s (client) | Fail fast on unresponsive authority |
| Auth response timeout | 5 s (authority) | Prevent resource exhaustion |
| Handshake timeout | 10 s (authority) | After successful auth |
| Handshake ack timeout | 5 s (client) | Keep reconnect loops responsive |

---

## Version Compatibility

rkyv's archived format allows new fields (via `#[rkyv(default)]`) without breaking
existing readers. Removing or renaming fields is a breaking change. `PacketType` may
gain variants; existing variants are never removed. Breaking changes will bump the
major version; a version field will be added to the packet format in v1.0.

---

## Implementation Checklist

- [ ] `src/protocol/mod.rs` -- re-exports all protocol types
- [ ] `src/protocol/types.rs` -- PacketHeader, PacketType, TreeRequest, TreeResponse,
      AuthChallenge, AuthResponse, HandshakeMessage, HandshakeAck, HookTarget,
      HookResponseType, HookDataMessage, HookCloseMessage, StreamOpenRequest,
      LeafInfo, LeafState, LeafValue, ProcedureDescriptor
- [ ] `src/protocol/content_types.rs` -- content type constants
- [ ] `src/transport/mod.rs` -- Transport trait, TransportError
- [ ] `src/transport/tcp.rs` -- TcpTransport
- [ ] `src/auth/mod.rs` -- HMAC-SHA256 challenge/response helpers
- [ ] `src/tree/mod.rs` -- Tree, authority rule enforcement, path prefix matching on Vec<String>
- [ ] `src/hooks/mod.rs` -- Hook table keyed by hook_id, routing integration
      (pending finalised hook semantics)
- [ ] `src/leaves/mod.rs` -- Leaf trait, LeafInfo serialisation
- [ ] `src/leaves/tty.rs` -- TTY leaf reference implementation
- [ ] `ush-router/` -- router binary: path routing, stream fastpath, hook routing
- [ ] `ush-payload/` -- payload binary: transport, auth, leaf hosting
- [ ] `ush-cli/` -- operator REPL: auth, leaf discovery, TTY integration
- [ ] Unit tests: packet round-trips, HMAC auth, authority rule enforcement,
      path prefix matching on Vec<String>
- [ ] Integration test: two endpoints through a real router, full auth flow
- [ ] Integration test: CallProcedure with response_hook -> HookData -> HookClose
- [ ] Stream test: open, bidirectional data, close
- [ ] Leaf test: TTY start -> stream -> keystrokes -> output -> halt
- [ ] Transport: TlsTransport (stealth mode)