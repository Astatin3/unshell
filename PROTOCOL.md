# UnShell Network Protocol Specification

**Version:** 0.1.0  
**Status:** Draft — implementation in progress  
**Last updated:** 2026-04-20

---

## Overview

The UnShell protocol is a **tree-addressed, message-passing protocol** for command
and control (C2) operations. It is designed around a homogeneous node model: every
participant (payload, operator, router) is structurally identical from the protocol's
perspective. Each node owns a set of **paths** in a global tree and responds to
requests addressed to those paths.

```
  /agents/abc123/shell/exec    ← a path owned by payload node "abc123"
  /agents/abc123/files/read    ← another path on the same payload
  /operator/sess1              ← operator node's own registration path
  /router/nodes                ← router's built-in endpoint
```

A **router** is a dumb relay. It reads the destination path from a packet header and
forwards the packet body to whichever node registered that path. It has no application
logic. It does not interpret payloads. Think of it as a post office: it reads the
address on the envelope and delivers the contents without opening them.

---

## Design Goals

1. **Minimal footprint on the payload.** The payload binary must stay small. The
   protocol must work in a `no_std + alloc` environment.

2. **Transport independence.** TCP is the first transport, but the protocol must not
   assume TCP. HTTPS, ICMP, and other transports will be added later. The protocol
   layer sits above the transport layer via a `Transport` trait.

3. **Router-opaque payloads.** The router only reads the packet header (destination
   path, source path, packet type). The payload body is forwarded as opaque bytes.
   This means the protocol can evolve without touching router code.

4. **Forward compatibility.** Adding new fields to message types must not break
   existing implementations. Use rkyv's archived format, which supports this.

5. **Operator experience.** The operator CLI is a first-class node, not a special
   client. It connects and registers like any payload, just with a terminal attached.

---

## Node Types

```
┌─────────────────┐    ┌─────────────────────────────────────────────┐
│   Payload Node  │    │             Router Node                      │
│                 │    │                                              │
│  - Registers at │    │  - Accepts TCP from all node types           │
│    /agents/<id> │    │  - Maintains: node_id → (paths, tx_channel) │
│  - Hosts modules│    │  - Routes packets by longest-prefix match    │
│    as endpoints │    │  - Has own endpoints at /router/...          │
│  - no_std + alloc│   │  - NO application logic beyond routing      │
└────────┬────────┘    └─────────────────────────────────────────────┘
         │ TCP (reverse connect: payload → router)
         │
┌────────▼────────┐
│  Operator Node  │
│  (ush-cli)      │
│                 │
│  - Registers at │
│    /operator/<n>│
│  - Interactive  │
│    REPL shell   │
│  - Issues Tree  │
│    Requests to  │
│    any path     │
└─────────────────┘
```

**Path conventions:**
- Payload nodes: `/agents/<node_id>/` prefix (e.g., `/agents/abc123/shell/exec`)
- Operator nodes: `/operator/<session_id>/` prefix
- Router built-ins: `/router/` prefix (e.g., `/router/nodes`, `/router/ping`)

**NodeType enum (v1):**
```rust
pub enum NodeType {
    Payload,
    Operator,
    // Router variant added when multi-hop/pivoting is implemented
}
```

---

## Wire Format

Every transmission uses a **two-part framed message**:

```
┌──────────────────────────────────────────────────────────────────────┐
│  Part 1: Header                          │  Part 2: Payload          │
│                                          │                           │
│  [u32 big-endian length]                 │  [u32 big-endian length]  │
│  [rkyv-serialised PacketHeader bytes]    │  [rkyv payload bytes]     │
│                                          │                           │
│  Router reads this to determine routing  │  Router forwards opaque   │
└──────────────────────────────────────────┴───────────────────────────┘
```

Both length fields are **big-endian `u32`**, so the maximum frame size is ~4GB per
part. In practice, packets should be much smaller. A future streaming extension will
allow chunked payloads for large data transfers.

### Why two parts?

The router needs to know where to send a packet. With a single rkyv blob, the router
would have to deserialise the entire packet just to read the destination path. With a
separate header, the router deserialises only the small header (typically < 100 bytes)
and forwards the payload bytes untouched. This is efficient and keeps the protocol
transport-agnostic at the router level.

### PacketHeader

```rust
/// The packet header that every node sends before the payload.
/// The router reads ONLY this to determine routing.
/// The payload body is opaque to the router.
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct PacketHeader {
    /// Destination path in the global tree.
    /// The router does a longest-prefix match against registered node paths.
    /// Example: "/agents/abc123/shell/exec"
    pub dst_path: String,

    /// Source path of the sending node.
    /// Used by the destination to know where to send the response.
    /// Example: "/operator/sess1"
    pub src_path: String,

    /// Discriminates between handshake and protocol messages.
    pub packet_type: PacketType,
}

/// Discriminates the payload type so the receiver knows how to deserialise it.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum PacketType {
    /// Sent by a newly connected node to register itself.
    Handshake,
    /// Sent by the router in response to a handshake.
    HandshakeAck,
    /// An application-level request (the main protocol message).
    Request,
    /// An application-level response.
    Response,
}
```

**Why `String` for paths instead of `Vec<String>`?**

A single `/`-delimited string serialises smaller (one allocation, no Vec overhead)
and is easier for the router to do prefix matching on. Components are split at
application layer, not at the wire level.

---

## Handshake Protocol

When any node connects to the router, it must complete a handshake before sending
application messages. The handshake registers the node's identity and the paths it
owns.

```
Node                         Router
 │                              │
 │──── TCP connect ────────────>│
 │                              │
 │──── HandshakeMessage ───────>│  (PacketType::Handshake)
 │     node_id: "abc123"        │
 │     node_type: Payload       │
 │     registered_paths: [...]  │
 │     platform: "linux-x86_64" │
 │                              │
 │<─── HandshakeAck ────────────│  (PacketType::HandshakeAck)
 │     accepted: true           │
 │     assigned_base_path: "..." │
 │                              │
 │  [now registered, can send   │
 │   and receive Requests]      │
```

**Handshake timeout:** If the node does not receive a `HandshakeAck` within **5
seconds**, it closes the connection and retries.

**Router timeout:** If the router does not receive a `HandshakeMessage` within **10
seconds** of a TCP connect, it closes the connection.

### HandshakeMessage

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct HandshakeMessage {
    /// Node identifier. For payloads: baked at compile time (base62).
    /// For operator CLI: random per session (UUID or random base62).
    pub node_id: String,

    /// Whether this node is a payload or an operator shell.
    pub node_type: NodeType,

    /// The path prefixes this node owns. The router registers these.
    /// Example: ["/agents/abc123"]
    /// All sub-paths are implicitly owned by this prefix.
    pub registered_paths: Vec<String>,

    /// Human-readable platform string for operator visibility.
    /// Example: "linux-x86_64", "windows-x86_64", "operator"
    pub platform: String,
}
```

### HandshakeAck

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct HandshakeAck {
    /// Whether the router accepted this node's registration.
    pub accepted: bool,

    /// The canonical base path assigned by the router (usually matches
    /// the first registered_path the node sent, but the router may adjust it).
    /// Empty string if rejected.
    pub assigned_base_path: String,

    /// Human-readable rejection reason if accepted == false.
    pub rejection_reason: Option<String>,
}
```

**Rejection reasons (v1):**
- `"duplicate_node_id"` — a node with this ID is already registered
- `"invalid_path"` — a registered path is malformed or conflicts with a reserved prefix

---

## Application Protocol: TreeRequest / TreeResponse

After the handshake, nodes communicate using `TreeRequest` / `TreeResponse` pairs.

A request travels: **sender → router → destination node**  
A response travels: **destination → router → original sender** (using `src_path` from the request header as the destination path for the response)

### TreeRequest

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct TreeRequest {
    /// Unique ID for this request, generated by the sender.
    /// The responder echoes this back in TreeResponse.request_id.
    /// Enables correlation when multiple requests are in-flight.
    pub request_id: u64,

    /// The operation type.
    pub request_type: RequestType,

    /// Content-type string describing how to interpret `data`.
    /// Convention: "core/None", "core/Utf8String", "core/Bytes", etc.
    pub content_type: String,

    /// The operation payload. Interpretation depends on content_type.
    pub data: Vec<u8>,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum RequestType {
    /// Read a value at this path.
    Read = 0,

    /// List available sub-paths and procedures at this path.
    GetProcedures = 1,

    /// Write a value to this path.
    Write = 2,

    /// Invoke a named procedure at this path.
    CallProcedure = 3,
}
```

### TreeResponse

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct TreeResponse {
    /// Echoed from the corresponding TreeRequest.request_id.
    pub request_id: u64,

    /// Whether the operation succeeded or failed.
    pub status: ResponseStatus,

    /// Content-type of the response data.
    pub content_type: String,

    /// Response payload. Empty if status is an error with no data.
    pub data: Vec<u8>,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ResponseStatus {
    /// Operation completed successfully.
    Ok = 0,

    /// The requested path does not exist at the destination node.
    NoBranchError = 1,

    /// The requested operation is not supported at this path.
    UnsupportedOperation = 2,

    /// The destination node encountered an error executing the request.
    ExecutionError = 3,

    /// The request payload was malformed.
    ProtocolError = 4,
}
```

---

## Content Type Convention

The `content_type` field in requests and responses follows a namespaced string
convention, similar to MIME types but simpler:

| Content type | Meaning |
|---|---|
| `"core/None"` | No data (empty payload) |
| `"core/Utf8String"` | Raw UTF-8 string in `data` |
| `"core/Bytes"` | Raw bytes (no specific interpretation) |
| `"core/ProcedureList"` | Response to `GetProcedures`: rkyv-serialised `Vec<ProcedureDescriptor>` |
| `"shell/Output"` | Shell command output (UTF-8 stdout + stderr) |
| `"files/Bytes"` | Raw file contents |

Custom module content types should use the module name as the namespace:
`"mymodule/MyType"`.

---

## Path Routing

The router uses **longest-prefix match** to route packets to nodes.

```
Registered paths:        Incoming dst_path:         Routes to:
/agents/abc123           /agents/abc123/shell/exec  → node "abc123"
/agents/xyz456           /agents/xyz456/files/read  → node "xyz456"
/router                  /router/nodes              → router's built-in handler
```

**Rules:**
1. Split `dst_path` by `/`, find all nodes whose `registered_paths` is a prefix of `dst_path`.
2. Choose the node with the longest matching prefix (most specific).
3. If no match, return a `TreeResponse { status: NoBranchError, ... }` to the sender.
4. If multiple nodes match with equal prefix length (should not happen if registration is correct), route to the most recently registered node and log a warning.

---

## Router Built-in Endpoints

The router itself hosts a small set of endpoints at `/router/`:

| Path | RequestType | Returns |
|---|---|---|
| `/router/nodes` | `GetProcedures` | List of all connected nodes with their paths and types |
| `/router/ping` | `Read` | `"pong"` (latency check) |

---

## Real-World Scenario Analysis

This section stress-tests the protocol against conditions you'll actually encounter
on an engagement or in the wild.

### Scenario 1: Flaky Network / Payload Reconnect

**Situation:** A payload is behind a NAT and its TCP connection to the router drops
(firewall timeout, network hiccup, target rebooted).

**What happens:**
1. Payload's `recv()` call returns `TransportError::Disconnected` (EOF) or `TransportError::Io`.
2. Payload closes the TcpStream, waits **5 seconds**, attempts reconnect.
3. Router's node thread for this connection receives EOF, removes the `NodeInfo` entry from the registry, exits cleanly.
4. Payload reconnects, sends a new `HandshakeMessage` with the **same** `node_id`.
5. Router re-registers it. The operator runs `list` and sees the payload appear again.

**Operator experience:** The operator may see the payload disappear from `list` briefly
during the reconnect window. Sessions associated with that payload become temporarily
unresponsive. After reconnect they work again.

**Failure mode:** If the payload's `node_id` was stored as persistent session state on
the operator side, it should survive the reconnect without the operator re-typing `use`.

**Protocol requirement:** The router must handle re-registration of a node ID that was
previously registered. The old entry is already gone (thread exited), so this is a
clean re-registration.

---

### Scenario 2: Operator Disconnects Mid-Session

**Situation:** The operator closes the CLI (`Ctrl+C`, terminal crash) while a payload
is still connected.

**What happens:**
1. Router's operator node thread receives EOF. Removes `/operator/sess1` from registry.
2. Any in-flight `TreeRequest` from that operator that the payload hasn't responded to
   yet: the payload sends a `TreeResponse` back, router tries to route it to
   `/operator/sess1`, finds no registered node, discards the response and logs a warning.
3. Payloads remain connected. The payload's modules keep running (persistence).

**Operator experience:** When the operator reconnects, it gets a **new session ID**
(`/operator/sess2`). It runs `list` to see what payloads are still connected. Background
operations on payloads that were running continue.

**Key insight:** The payload is the persistent state. The operator is ephemeral.
This is the "background services without another process" design — payload modules
keep running even when no operator is connected.

---

### Scenario 3: Multiple Operators

**Situation:** Two operators connect simultaneously (e.g., red team lead and junior
analyst).

**What happens:**
1. Both connect, get unique session IDs: `/operator/sess1` and `/operator/sess2`.
2. Both can send requests to any payload path.
3. Responses go back to the requesting operator's `src_path`.
4. There is no access control in v1. Both operators have full access to all paths.

**Collision scenario:** Both operators call `/agents/abc123/shell/exec "ls"` at the
same time. The payload processes requests sequentially (single-threaded recv loop).
It sends two responses, each echoing the correct `request_id`. Each response routes
to the operator that sent the matching request (via `src_path` in the request header).

**Failure mode in v1:** No locking on the payload side. If a `Write` and a `Read` to
the same resource happen simultaneously, the result is whatever order the TCP stack
delivers them. This is acceptable for v1 red team use where multiple operators are
unlikely to stomp each other on the same target simultaneously.

**Future:** Add an optional exclusive-lock request type for sensitive operations.

---

### Scenario 4: Large Data Transfer (File Exfiltration)

**Situation:** Operator requests a large file (100MB) from a target.

**Problem with current design:** The `u32` length prefix allows up to 4GB per packet,
but buffering 100MB in RAM on the payload before sending is problematic on constrained
targets.

**V1 approach:** Accept this limitation. Files up to ~50MB should be fine in practice
for most engagements. The `TreeRequest.data` field holds the serialised request;
the `TreeResponse.data` field holds the file bytes. For v1, the payload reads the
entire file into a `Vec<u8>` and sends it.

**Future (chunked streaming):** Add `PacketType::Stream` and `PacketType::StreamEnd`
to support chunked transfers. The router passes stream packets through without buffering.
The operator reassembles chunks. This requires a stream ID in the header to demultiplex
concurrent streams.

---

### Scenario 5: AV / EDR Detection via Network Traffic

**Situation:** The payload is on a monitored network. The router is a VPS. Plain TCP
connections from the target to an unknown IP may trigger alerts.

**V1 limitation:** Plaintext TCP. Easy to detect.

**Transport abstraction payoff:** The `Transport` trait makes this the router's and
payload's responsibility, not the protocol's. To switch to HTTPS:
1. Implement `HttpsTransport: Transport` for the payload.
2. Have the payload connect to a domain name (baked at compile time) on port 443.
3. The router terminates TLS and speaks the same framing protocol underneath.
4. From the network's perspective: an HTTPS connection to what looks like a CDN.

Nothing in the protocol spec changes. Only the `Transport` implementation swaps.

---

### Scenario 6: Router Crash / Restart

**Situation:** The router process crashes or is restarted (e.g., VPS reboot).

**What happens:**
1. All node TCP connections drop simultaneously.
2. All nodes (payloads and operators) receive `Disconnected` errors.
3. All nodes enter reconnect loops.
4. Once the router restarts and starts accepting connections, nodes reconnect and
   re-register in whatever order their reconnect loops fire.
5. The router comes back to a clean state (no session persistence across restarts in v1).

**Failure mode:** In-flight requests at the time of crash are lost. The operator may
see commands that appear to hang. The operator should use a timeout on requests.

**V1 mitigation:** Request timeout is on the operator's TODO list. For now, the
operator can detect a crash by the payload disappearing from `list`.

**Future:** The router could persist its node registry to disk and recover after restart.

---

### Scenario 7: Malformed Packet / Bad Actor

**Situation:** Something sends a malformed packet to the router (fuzzer, compromised
node, network corruption).

**Defense layers:**
1. **Length prefix:** If the announced frame length is > a max limit (e.g., 64MB), the
   router closes the connection with `TransportError::FrameTooLarge`. No allocation.
2. **rkyv deserialisation:** If the header bytes don't decode to a valid `PacketHeader`,
   `rkyv::access` returns an error. The router closes the connection.
3. **Unknown `dst_path`:** Routes to no node, sends back `NoBranchError`.
4. **No authentication in v1:** Any node can send to any path. This is acceptable for
   v1 where the router address is only known to the operator. Authentication (shared
   secret or challenge-response) is a v2 concern.

---

### Scenario 8: Pivot / Multi-Hop (Future)

**Situation:** A payload on an internal network can only reach another internal host,
not the external router. A "pivot" payload acts as a relay.

**How the tree model enables this:**
1. Pivot payload registers at `/agents/pivot1/` on the external router.
2. Pivot payload also acts as a *local router* for sub-agents.
3. Sub-agents connect to the pivot payload's local listener and register.
4. The pivot payload's `/agents/pivot1/agents/` prefix forwards packets to sub-agents.
5. From the external operator's perspective: `/agents/pivot1/agents/sub1/shell/exec`
   is just a deeper path. The routing is recursive.

**Protocol requirement to enable this:** Add `NodeType::Router` to the enum. A pivot
payload registers as a `Router` node, not a `Payload` node. The external router
knows to forward any path with `/agents/pivot1/` prefix to the pivot connection,
and the pivot routes further from there.

This does not require protocol changes to v1. Only the `NodeType` enum needs the
`Router` variant added back.

---

## Transport Trait

All transports implement this interface:

```rust
/// A bidirectional framed transport.
///
/// Implementations are responsible for framing: the two-part header+payload format
/// described in the wire format spec. Each `send` call transmits exactly one
/// logical packet (header + payload). Each `recv` call receives exactly one.
///
/// Implementations MUST use `read_exact`-style loops (not single `read` calls)
/// because TCP is a stream protocol and may deliver partial frames.
///
/// # Example
///
/// ```rust
/// // TCP implementation skeleton
/// impl Transport for TcpTransport {
///     fn send(&mut self, header: &PacketHeader, payload: &[u8]) -> Result<(), TransportError> {
///         // 1. Serialise header to bytes
///         // 2. Write [u32 header_len][header bytes][u32 payload_len][payload bytes]
///         // 3. Use write_all() to ensure complete write
///     }
///     fn recv(&mut self) -> Result<(PacketHeader, Vec<u8>), TransportError> {
///         // 1. read_exact 4 bytes → header length
///         // 2. read_exact N bytes → header bytes
///         // 3. Deserialise header
///         // 4. read_exact 4 bytes → payload length
///         // 5. read_exact M bytes → payload bytes
///         // 6. Return (header, payload)
///     }
/// }
/// ```
pub trait Transport: Send {
    /// Send a packet (header + payload) over this transport.
    /// Blocks until all bytes are written.
    fn send(&mut self, header: &PacketHeader, payload: &[u8]) -> Result<(), TransportError>;

    /// Receive one packet from this transport.
    /// Blocks until a complete header+payload pair is received.
    fn recv(&mut self) -> Result<(PacketHeader, Vec<u8>), TransportError>;
}

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("frame header too large: {0} bytes (max {1})")]
    FrameTooLarge(usize, usize),

    #[error("connection closed cleanly")]
    Disconnected,

    #[error("rkyv deserialisation failed")]
    DeserialiseError,
}
```

### Reconnect Policy

**Payloads:** On `Disconnected` or `Io(_)` from `recv()` or `send()`:
1. Close the transport.
2. Wait 5 seconds.
3. Attempt to create a new transport connection.
4. If connect fails, wait 5 more seconds, retry. No maximum retry limit.
5. On connect success, run the handshake again.

**Operator CLI:** On disconnect, print a message and exit. The operator restarts the
CLI manually. (In a future version, the CLI could auto-reconnect and restore session.)

---

## Frame Size Limits

| Limit | Value | Reason |
|---|---|---|
| Max header length | 64 KB | Headers should never be this large; anything bigger is a bug or attack |
| Max payload length | 64 MB | Sufficient for most file transfers; larger files need chunked streaming (future) |
| Handshake timeout | 10 s (router) | Prevent resource exhaustion from hanging connections |
| Handshake ack timeout | 5 s (node) | Keep reconnect loops responsive |

---

## Version Compatibility

rkyv's archived format allows adding new fields (with `#[rkyv(default)]` for missing
fields when reading older messages). This means:

- New fields can be added to any message type without breaking existing implementations.
- Removing or renaming fields IS a breaking change.
- The `PacketType` enum should only gain variants, never lose them.

When breaking changes are necessary, bump the protocol version (future: add a version
field to the framing format).

---

## Implementation Checklist

- [ ] `src/protocol/mod.rs` — re-exports all protocol types
- [ ] `src/protocol/types.rs` — PacketHeader, PacketType, TreeRequest, TreeResponse, HandshakeMessage, HandshakeAck
- [ ] `src/protocol/content_types.rs` — content type constants
- [ ] `src/transport/mod.rs` — Transport trait, TransportError
- [ ] `src/transport/tcp.rs` — TcpTransport implementing Transport
- [ ] `src/tree/mod.rs` — Tree, Endpoint trait (new implementation with correct routing)
- [ ] `ush-router/` — router binary
- [ ] `ush-payload/` — payload binary with transport layer
- [ ] `ush-cli/` — operator REPL binary
- [ ] Unit tests for framing round-trips, tree routing correctness
- [ ] Integration test: two nodes through a real router
