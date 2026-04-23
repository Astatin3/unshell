# UnShell Protocol Specification

**Version:** 0.6.0  
**Status:** Draft  
**Last updated:** 2026-04-23

## 1. Introduction

**Non-Normative**

The UnShell protocol is a tree-addressed packet protocol for remote procedure calls, response hooks, and bidirectional streams across a hierarchy of connected endpoints.

The protocol is intended to be small, extensible, and canonical.

Small means the core protocol stays narrow enough for constrained implementations. Extensible means new behavior is introduced through leaves, procedures, and payload schemas instead of frequent protocol redesign. Canonical means there should be one clearly defined way to express each core protocol behavior.

This document combines exact protocol definition with rationale. Rationale blocks explain why a rule exists, but do not define interoperability requirements.

> **Rationale:** This document uses a formal specification layout: descriptive sections first, exact protocol definition later, and rationale kept adjacent to the rules it explains.

## 2. Document Conventions

**Normative**

The key words `MUST`, `MUST NOT`, `REQUIRED`, `SHALL`, `SHALL NOT`, `SHOULD`, `SHOULD NOT`, `RECOMMENDED`, `MAY`, and `OPTIONAL` in this document are to be interpreted as described in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119.txt) when, and only when, they appear in all capitals.

Unless a section is explicitly marked otherwise, sections labeled `Normative` define protocol requirements and sections labeled `Non-Normative` provide description, rationale, deployment guidance, or open design commentary.

All `Rationale` blocks in this document are non-normative.

## 3. Purpose and Scope

**Non-Normative**

The purpose of this specification is to define the set of protocol components required to assemble complete UnShell protocol packets and to provide a framework through which the protocol can be extended through leaves and procedure contracts.

To achieve this purpose, the scope of this specification includes:

- endpoint addressing by path
- packet framing
- packet structure
- local authority rules for downwards procedure calls
- path-based routing behavior
- upwards and downwards packet semantics
- hook behavior
- stream behavior
- the required introspection procedure
- extension through leaves, procedures, and payload schemas

The UnShell protocol assumes that a connection already exists, that the local implementation has decided whether a peer should be admitted into routing, and that any required authentication or authorization has already been handled by the surrounding system.

The following items are beyond the scope of this specification:

- authentication
- authorization
- connection establishment
- admission protocol
- transport selection
- transport-specific serialization formats
- encryption
- obfuscation
- router management interfaces
- deployment-specific orchestration behavior
- sensing, analytics, and decision-making systems above the protocol layer

Every implementation is expected to maintain its own live connection set and its own ground truth about which peers are connected, admitted, and routable.

> **Rationale:** Authentication and handshakes were intentionally removed from the core scope. They are too deployment-specific to define canonically without bloating the protocol.

## 4. Protocol Overview

**Non-Normative**

Endpoints are addressed by path.

Leaves are hosted by endpoints.

A superior endpoint issues a downwards `Call` toward a subordinate endpoint or one of its leaves.

If the caller wants output, it declares a hook inside the call. The recipient returns one or more `Data` packets upwards toward the hook host. If that hook is stream-oriented, the same `Data` packet type is also used for subsequent bidirectional stream traffic.

The protocol therefore has two core packet roles:

- `Call` for downwards invocation
- `Data` for returned data and stream traffic

This document uses the following notation for readability:

- `/a/b/c` for endpoint paths
- `/a/b/c { leaf: tty0 }` for a leaf on an endpoint
- `/a/b/c { hook: 7 }` for a hook hosted by an endpoint

These notations are descriptive only. Leaves and hooks are not encoded as path segments.

## 5. Terms and Definitions

**Normative**

| Term | Definition |
|---|---|
| Tree | The set of connected endpoints arranged by path. |
| Endpoint | A participant in the protocol that can send, receive, host leaves, and route packets. |
| Path | An ordered sequence of segments identifying an endpoint, serialized as `Vec<String>`. |
| Upwards | In the direction of rising authority, closer to the root node. |
| Downwards | In the direction of falling authority, farther from the root node. |
| Leaf | A named service or object hosted by an endpoint. |
| Call | A downwards packet that invokes a procedure on an endpoint or leaf. |
| Procedure | An application-defined operation identified by `procedure_id`. |
| Hook | A response channel declared inside a `Call`. |
| Stream | A bidirectional exchange of `Data` packets associated with a hook and a local `stream_id`. |
| Authority | The endpoint that directly maintains a child connection at a local routing boundary. |
| Subordinate | The lower of two endpoints in a described authority relationship. |
| Registered | Local connection state in which a peer participates in routing. |
| Unregistered | Local connection state in which a peer is connected but not routable. |

## 6. Naming and Structural Conventions

**Normative**

Paths are serialized as `Vec<String>`.

Leaf identity is carried in `dst_leaf`.

Hook identity is carried in `hook_id`.

Stream identity is carried in `stream_id`.

No path prefixes are reserved by this protocol.

`procedure_id` is the canonical identifier for a procedure contract. A procedure contract includes the source library or namespace, the specific procedure identity, and the expected input and output schema pair.

The same `procedure_id` is used on both `Call` and `Data` packets.

> **Rationale:** `procedure_id` is intentionally stricter than a method name or content type. It identifies a full callable contract, not just a label.

## 7. Endpoint Model

**Normative**

### 7.1 Local Authority

Each endpoint enforces authority only at the connections it directly maintains.

At a local routing boundary:

- a `Call` packet MUST be accepted only if it arrives from the direct parent connection permitted to issue downwards calls into the destination subtree represented by that boundary
- a `Call` packet that violates that rule MUST be dropped silently
- a `Data` packet MAY arrive from either direction if it belongs to a valid hook or stream flow and routes correctly by path

This protocol does not define a protocol-level authority error packet.

### 7.2 Local Connection States

Each implementation MUST maintain at least the following local states:

| State | Meaning |
|---|---|
| `Unregistered` | The connection exists locally but is not part of routing state. |
| `Registered` | The connection is admitted into local routing state and may send, receive, or forward protocol traffic. |

While a connection is `Unregistered`, an implementation:

- MUST NOT forward protocol packets through it
- MUST NOT trust its path claims for routing
- MUST NOT allocate hook or stream state on its behalf

Transition into `Registered` is implementation-defined and out of scope for this document.

Transition out of `Registered` MUST invalidate all local routing entries, hook state, and stream state associated with that connection.

> **Rationale:** The protocol no longer defines a handshake, but it still needs a hard boundary between connected peers and admitted peers.

## 8. Packet Framing

**Normative**

Each protocol packet consists of two length-prefixed byte sections:

1. header bytes
2. payload bytes

Both lengths MUST be encoded as big-endian `u32`.

The header MUST be serialized before the payload.

Routing decisions MUST be made from header fields only.

Routers MUST NOT inspect payload structure in order to route a packet.

## 9. Packet Types

**Normative**

This protocol defines exactly two packet types.

| Packet Type | Value | Meaning |
|---|---|---|
| `Call` | `0x01` | Downwards procedure invocation. |
| `Data` | `0x02` | Hook output, event output, or stream traffic. |

Example in the current Rust implementation:

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum PacketType {
    Call = 0x01,
    Data = 0x02,
}
```

`Call` is used for downwards invocation.

`Data` is used for hook output, event output, and stream traffic.

> **Rationale:** This is the canonical simplification of the earlier model. Separate response and stream-close packet types were removed.

## 10. Packet Header

**Normative**

| Field | Meaning |
|---|---|
| `packet_type` | Selects packet semantics. |
| `src_path` | Path of the sending endpoint. |
| `dst_path` | Path of the destination endpoint. |
| `dst_leaf` | Target leaf for a `Call`, if any. |
| `hook_id` | Hook identifier local to the endpoint hosting the hook. |
| `stream_id` | Stream identifier local to the endpoint receiving the stream traffic. |

Header rules:

- `src_path` and `dst_path` MUST be present on all packets
- `dst_leaf` MUST be `None` on `Data`
- `stream_id` MUST NOT appear on `Call` unless the call declares a stream-oriented hook
- `hook_id` MUST appear on `Data` when the packet belongs to a hook or hook-backed stream
- `stream_id` MUST appear on `Data` when the packet belongs to a stream

A packet whose header violates these rules MUST be discarded.

Example in the current Rust implementation:

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct PacketHeader {
    pub packet_type: PacketType,
    pub src_path: Vec<String>,
    pub dst_path: Vec<String>,
    pub dst_leaf: Option<String>,
    pub hook_id: Option<u64>,
    pub stream_id: Option<u32>,
}
```

## 11. Routing Rules

**Normative**

### 11.1 Path Routing

All protocol routing is path-based.

When forwarding a packet, an implementation MUST:

1. compare `dst_path` against its locally registered child paths
2. choose the longest matching prefix
3. forward the packet toward that child if such a child exists
4. otherwise, deliver the packet locally if `dst_path` identifies the local endpoint
5. otherwise, drop the packet silently

The protocol defines no mandatory error packet for unresolved destinations.

### 11.2 Call Enforcement

When forwarding or receiving a `Call`, an endpoint MUST verify the local parent-child relationship at the boundary where the packet arrives.

If the sender on that connection is not the direct parent permitted to issue downwards calls into the relevant subtree, the endpoint MUST drop the packet silently.

### 11.3 Data Routing

`Data` packets are routed by `dst_path` using the same path-routing rules as `Call` packets.

The sender of a `Data` packet MUST set `dst_path` to the path of the stream peer or the hook host.

### 11.4 Stream Fastpath

An implementation MAY maintain an internal fastpath keyed by `(local_connection, stream_id)` for performance.

Such an optimization MUST remain behaviorally equivalent to path-based routing.

The protocol itself does not route by `stream_id` alone.

> **Rationale:** `stream_id` is intentionally not treated as a globally routable identifier.

## 12. Call Definition

**Normative**

| Field | Meaning |
|---|---|
| `procedure_id` | Identifier of the invoked procedure contract. |
| `data` | Application-defined procedure input payload. |
| `response_hook` | Optional hook declaration for returned data. |

Rules:

- the receiver MUST interpret `procedure_id` as the identifier of the procedure being invoked
- the protocol does not define argument encoding beyond raw bytes in `data`
- a `Call` that expects a result MUST include `response_hook`
- if `response_hook` is absent, the receiver MAY execute the procedure but MUST NOT fabricate an implicit response path

Example in the current Rust implementation:

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct CallMessage {
    pub procedure_id: String,
    pub data: Vec<u8>,
    pub response_hook: Option<HookTarget>,
}
```

### 12.1 Required Introspection Procedure

The empty string `""` is reserved as the required introspection procedure.

Every endpoint MUST implement `procedure_id == ""`.

Behavior:

- when `dst_leaf` is `None`, the call requests endpoint introspection
- when `dst_leaf` is set, the call requests introspection for that specific leaf

The result MUST be returned through the declared response hook.

### 12.2 Failure Behavior

If the destination endpoint does not exist, the packet is dropped during routing.

If the destination endpoint exists but `dst_leaf` names no local leaf, the endpoint MUST discard the `Call` silently.

If `procedure_id` is unknown or unsupported, the endpoint SHOULD report failure through the declared hook using an application-defined error payload. If no hook exists, the endpoint MUST discard the call silently.

## 13. Hook Definition

**Normative**

Hooks are declared only inside `CallMessage.response_hook`.

There is no standalone hook-open packet.

| Field | Meaning |
|---|---|
| `hook_id` | Identifier local to the endpoint that hosts the hook and expects responses. |
| `return_path` | Endpoint path to which returned `Data` packets are sent. |
| `response_type` | Advisory indication of whether the expected response is event-like or stream-like. |

Rules:

- `hook_id` MUST be unique within the receiving endpoint's active hook set
- `return_path` MUST name the endpoint hosting the hook
- `response_type` is advisory and does not itself terminate or prolong hook lifetime

Example in the current Rust implementation:

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct HookTarget {
    pub hook_id: u64,
    pub return_path: Vec<String>,
    pub response_type: HookResponseType,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum HookResponseType {
    Event = 0,
    Stream = 1,
}
```

## 14. Data Definition

**Normative**

| Field | Meaning |
|---|---|
| `procedure_id` | Identifier of the procedure contract to which this returned payload belongs. |
| `data` | Application-defined output payload. |
| `end` | Sender indicates completion of its participation in the hook or stream. |
| `cancel` | Sender requests termination of the associated stream processing. |

Rules:

- the receiver MUST interpret `procedure_id` as the contract identifier for the returned payload
- the router MUST NOT inspect or validate `procedure_id`
- the receiver MAY validate that the returned `procedure_id` matches the hook or stream context it established

Example in the current Rust implementation:

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct DataMessage {
    pub procedure_id: String,
    pub data: Vec<u8>,
    pub end: bool,
    pub cancel: bool,
}
```

### 14.1 Event Data

For non-stream responses:

- `hook_id` MUST be present
- `stream_id` MUST be absent
- `end` SHOULD be `true` on the final packet for that hook

An event-style hook MAY still emit multiple `Data` packets if the application requires chunking or phased output.

### 14.2 Stream Establishment

A stream exists only as part of a hook whose `response_type` is `Stream`.

There is no standalone stream-open packet.

The first `Data` packet for a stream MUST:

- carry the hook's `hook_id`
- carry a `stream_id`
- set `dst_path` to the hook host's `return_path`

Once established, either side MAY continue exchanging `Data` packets carrying that `stream_id` and the appropriate peer `dst_path`.

`stream_id` is local to the endpoint that receives and demultiplexes that stream.

An endpoint MUST NOT reuse an active `stream_id` within its local stream table.

### 14.3 Stream Data

For stream-associated traffic:

- `stream_id` MUST be present
- `hook_id` SHOULD be present on every packet and MUST be present on the first packet
- `dst_path` MUST identify the peer endpoint for that stream packet

### 14.4 End and Cancel

Rules:

- a sender MAY set `end = true` without `cancel = true`
- a sender MAY set `cancel = true` without `end = true`
- a sender MAY set both when it intends immediate termination
- a receiver of `cancel = true` SHOULD stop local processing for that stream as soon as practical

There is no separate stream-close or hook-close packet.

### 14.5 Unknown Stream IDs

If an endpoint receives `Data` with an unknown or expired `stream_id`, it MUST discard the packet.

The protocol does not define a mandatory error response for this case.

## 15. Introspection Payloads

**Normative**

The required introspection procedure `""` MUST return one of the following payloads through the declared hook.

### 15.1 Endpoint Introspection

Returned when `procedure_id == ""` and `dst_leaf == None`.

| Field | Meaning |
|---|---|
| `leaves` | List of introspection summaries for the endpoint's hosted leaves. |

Each `LeafIntrospectionSummary` contains:

| Field | Meaning |
|---|---|
| `leaf_name` | The leaf's local name. |
| `description` | Optional human-readable description. |
| `procedures` | Introspection records for the leaf's supported procedures. |
| `state_procedure_id` | Procedure contract identifier associated with the serialized `state` payload. |
| `state` | Serialized current-state payload. |

Example in the current Rust implementation:

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct EndpointIntrospection {
    pub leaves: Vec<LeafIntrospectionSummary>,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct LeafIntrospectionSummary {
    pub leaf_name: String,
    pub description: Option<String>,
    pub procedures: Vec<ProcedureIntrospection>,
    pub state_procedure_id: String,
    pub state: Vec<u8>,
}
```

### 15.2 Leaf Introspection

Returned when `procedure_id == ""` and `dst_leaf` names a specific leaf.

| Field | Meaning |
|---|---|
| `leaf_name` | The leaf's local name. |
| `description` | Optional human-readable description. |
| `procedures` | Introspection records for the leaf's supported procedures. |
| `state_procedure_id` | Procedure contract identifier associated with the serialized `state` payload. |
| `state` | Serialized current-state payload. |

Each `ProcedureIntrospection` contains:

| Field | Meaning |
|---|---|
| `name` | Procedure name within the leaf. |
| `description` | Optional human-readable description. |
| `params` | Parameter definitions accepted by the procedure. |
| `response_type` | Advisory indication of whether the procedure normally responds as an event or stream. |

Each `ProcedureParameter` contains:

| Field | Meaning |
|---|---|
| `name` | Parameter name. |
| `value_type` | Application-defined parameter type name. |

Example in the current Rust implementation:

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct LeafIntrospection {
    pub leaf_name: String,
    pub description: Option<String>,
    pub procedures: Vec<ProcedureIntrospection>,
    pub state_procedure_id: String,
    pub state: Vec<u8>,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct ProcedureIntrospection {
    pub name: String,
    pub description: Option<String>,
    pub params: Vec<ProcedureParameter>,
    pub response_type: HookResponseType,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct ProcedureParameter {
    pub name: String,
    pub value_type: String,
}
```

Rules:

- `state_procedure_id` MUST identify the procedure contract associated with the serialized `state` payload
- `params` MUST describe the accepted parameter names and parameter types for that procedure
- introspection SHOULD describe current state, but does not establish a cache coherence guarantee

## 16. Protocol Description

**Non-Normative**

The UnShell protocol has a deliberately narrow center:

- addressing by path
- one downwards packet type
- one returned-data packet type
- hooks for correlation
- streams as an extension of hook-backed data flow

This is meant to make the protocol easier to reason about and easier to implement in small agents.

`procedure_id` is the main semantic anchor. In this design, the caller and callee are expected to share knowledge of what a procedure contract means. The protocol does not carry a global registry.

## 17. Security Considerations

**Non-Normative**

Although security is not defined by the protocol itself, implementations should treat the `Unregistered` state as a strict quarantine boundary.

Recommended behavior:

- authenticate or otherwise validate a peer before moving it to `Registered`
- rate-limit or expire idle unregistered peers
- avoid disclosing topology before admission
- avoid detailed admission failure reasons
- invalidate hooks and streams on disconnect unless a higher-layer session mechanism exists

## 18. Serialization and Implementation Notes

**Non-Normative**

This document uses Rust-like `rkyv` struct notation to describe fields because it matches the current implementation language. The notation is explanatory. The protocol semantics are language-agnostic.

Recommended implementation limits:

| Item | Recommended limit |
|---|---|
| header length | 64 KiB |
| payload length | 64 MiB |

## 19. Known Hard Problems

**Non-Normative**

### 19.1 Loop Prevention Outside Strict Trees

The protocol does not carry a hop count, route vector, or loop-detection token.

That keeps packets small, but it means loop prevention must be handled by topology discipline or implementation policy.

### 19.2 Canonical Connection Management

The document defines `Registered` and `Unregistered` states but intentionally does not define how a peer moves between them.

That preserves flexibility, but it means interoperable admission behavior requires a higher-layer convention.

### 19.3 Shared Meaning of `procedure_id`

`procedure_id` is only useful if both sides share its meaning.

The protocol intentionally does not define a global registry or schema negotiation mechanism. That keeps the core minimal, but it pushes interoperability for procedure contracts into shared libraries, operator knowledge, or higher-layer conventions.

### 19.4 Stream Resumption Across Disconnects

Hook and stream state are tied to local connection state.

When a connection disappears, the associated hook and stream context disappears with it. Any resumable behavior therefore requires a higher-layer session mechanism.
