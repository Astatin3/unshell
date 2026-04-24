# UnShell Protocol Specification

**Version:** 0.7.0  
**Status:** Draft  
**Last updated:** 2026-04-23

## 1. Introduction

**Non-Normative**

The UnShell protocol is a tree-addressed packet protocol for remote procedure calls and bidirectional hook-backed data exchange across a hierarchy of connected endpoints.

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
- protocol fault behavior
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

If the caller wants output, it declares a hook inside the call. The recipient returns one or more `Data` packets toward the hook host. Once a hook exists, either side MAY continue exchanging `Data` packets associated with that hook until one side terminates the interaction.

The protocol therefore has two core packet roles:

- `Call` for downwards invocation
- `Data` for returned data, protocol faults, and ongoing hook traffic

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
| Hook | A bidirectional interaction channel declared inside a `Call` and identified by `hook_id` at the hook host. |
| Authority | The endpoint that directly maintains a child connection at a local routing boundary. |
| Subordinate | The lower of two endpoints in a described authority relationship. |
| Registered | Local connection state in which a peer participates in routing. |
| Unregistered | Local connection state in which a peer is connected but not routable. |

## 6. Naming and Structural Conventions

**Normative**

Paths are serialized as `Vec<String>`.

Leaf identity is carried in `dst_leaf`.

Hook identity is carried in `hook_id`.

No path prefixes are reserved by this protocol.

`procedure_id` is the canonical identifier for a procedure contract. A procedure contract includes the source library or namespace, the specific procedure identity, and the expected input and output schema pair.

`procedure_id` MUST use the canonical dotted form `org.product.vN.part.name`, where:

- `org` identifies the owning organization or namespace root
- `product` identifies the product or system namespace
- `vN` identifies the contract version, where `N` is a positive integer written in decimal form
- `part` identifies the subsystem, leaf family, or functional area
- `name` identifies the exact procedure or payload contract name

Each segment MUST be non-empty. Implementations SHOULD restrict segments to lowercase ASCII letters, digits, and underscores for portability. The version segment MUST appear exactly in the third position.

The same `procedure_id` is used on both `Call` and `Data` packets.

> **Rationale:** `procedure_id` is intentionally stricter than a method name or content type. It identifies a full callable contract, not just a label.

## 7. Endpoint Model

**Normative**

### 7.1 Local Authority

Each endpoint enforces authority only at the connections it directly maintains.

At a local routing boundary:

- a `Call` packet MUST be accepted only if it arrives from the direct parent connection permitted to issue downwards calls into the destination subtree represented by that boundary
- a `Call` packet that violates that rule MUST be dropped silently
- a `Data` packet MAY arrive from either direction if it belongs to a valid hook flow and routes correctly by path

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
- MUST NOT allocate hook state on its behalf

Transition into `Registered` is implementation-defined and out of scope for this document.

Transition out of `Registered` MUST invalidate all local routing entries and hook state associated with that connection.

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
| `Data` | `0x02` | Hook output, protocol fault output, or ongoing hook traffic. |

Example in the current Rust implementation:

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum PacketType {
    Call = 0x01,
    Data = 0x02,
}
```

`Call` is used for downwards invocation.

`Data` is used for hook output, protocol fault output, and ongoing hook traffic.

> **Rationale:** This is the canonical simplification of the earlier model. Separate response packet variants were removed.

## 10. Packet Header

**Normative**

| Field | Meaning |
|---|---|
| `packet_type` | Selects packet semantics. |
| `src_path` | Path of the sending endpoint. |
| `dst_path` | Path of the destination endpoint. |
| `dst_leaf` | Target leaf for a `Call`, if any. |
| `hook_id` | Hook identifier local to the endpoint hosting the hook. |

Header rules:

- `src_path` and `dst_path` MUST be present on all packets
- `dst_leaf` MUST be `None` on `Data`
- `hook_id` MUST appear on `Data` when the packet belongs to a hook flow, including returned data and protocol faults

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

The sender of a `Data` packet MUST set `dst_path` to the path of the hook peer or the hook host.

### 11.4 Hook Fastpath

An implementation MAY maintain an internal fastpath keyed by locally validated hook state for performance.

Such an optimization MUST remain behaviorally equivalent to path-based routing.

The protocol itself does not route by `hook_id` alone.

> **Rationale:** `hook_id` is local to the hook host, so path remains the canonical routing key.

## 12. Call Definition

**Normative**

| Field | Meaning |
|---|---|
| `procedure_id` | Identifier of the invoked procedure contract. |
| `data` | Application-defined procedure input payload. |
| `response_hook` | Optional hook declaration for returned data and follow-on bidirectional hook traffic. |

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

`org.unshell.protocol.v1.meta.introspect` is reserved as the required introspection procedure.

Every endpoint MUST implement `procedure_id == "org.unshell.protocol.v1.meta.introspect"`.

Behavior:

- when `dst_leaf` is `None`, the call requests endpoint introspection
- when `dst_leaf` is set, the call requests introspection for that specific leaf

The result MUST be returned through the declared response hook.

### 12.2 Failure Behavior

If the destination endpoint does not exist, the packet is dropped during routing.

If the destination endpoint exists but `dst_leaf` names no local leaf, the endpoint SHOULD report a protocol fault through the declared hook. If no hook exists, the endpoint MUST discard the `Call` silently.

If `procedure_id` is unknown or unsupported, the endpoint SHOULD report a protocol fault through the declared hook. If no hook exists, the endpoint MUST discard the call silently.

## 13. Hook Definition

**Normative**

Hooks are declared only inside `CallMessage.response_hook`.

There is no standalone hook-open packet.

| Field | Meaning |
|---|---|
| `hook_id` | Identifier local to the endpoint that hosts the hook and expects returned traffic. |
| `return_path` | Endpoint path to which returned `Data` packets are sent. |

Rules:

- `hook_id` MUST be unique within the receiving endpoint's active hook set
- `return_path` MUST name the endpoint hosting the hook
- once a hook is established, either side MAY send `Data` packets associated with that hook until the interaction ends or is canceled
- all protocol faults associated with the call MUST use that same `hook_id`

Example in the current Rust implementation:

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct HookTarget {
    pub hook_id: u64,
    pub return_path: Vec<String>,
}
```

## 14. Data Definition

**Normative**

| Field | Meaning |
|---|---|
| `procedure_id` | Identifier of the procedure contract to which this returned payload belongs. |
| `data` | Application-defined output payload. |
| `end_hook` | Sender indicates that its application protocol is ending the hook interaction. |

Rules:

- the receiver MUST interpret `procedure_id` as the contract identifier for the returned payload
- the router MUST NOT inspect or validate `procedure_id`
- the receiver MAY validate that the returned `procedure_id` matches the hook context it established or a reserved protocol fault contract

Example in the current Rust implementation:

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct DataMessage {
    pub procedure_id: String,
    pub data: Vec<u8>,
    pub end_hook: bool,
}
```

### 14.1 Hook Data

For hook-associated responses:

- `hook_id` MUST be present
- `end_hook` SHOULD be `true` on the final packet a sender emits for that hook

A hook MAY emit multiple `Data` packets if the application requires chunking, phased output, or prolonged bidirectional interaction.

### 14.2 Hook Continuation

A hook exists only as part of a `Call` that declares `response_hook`.

There is no standalone hook-open packet.

The first `Data` packet for a hook MUST:

- carry the hook's `hook_id`
- set `dst_path` to the hook host's `return_path`

Once established, either side MAY continue exchanging `Data` packets carrying that `hook_id` and the appropriate peer `dst_path`.

`hook_id` is local to the endpoint that hosts and demultiplexes that hook.

An endpoint MUST NOT reuse an active `hook_id` within its local hook table.

### 14.3 Bidirectional Hook Data

For ongoing hook traffic:

- `hook_id` MUST be present on every packet
- `dst_path` MUST identify the peer endpoint for that hook packet

### 14.4 Hook End

Rules:

- a sender MAY set `end_hook = true` when its application protocol has decided to end the hook interaction
- a receiver of `end_hook = true` SHOULD treat the sender as finished with that hook
- any finer-grained shutdown, acknowledgment, or cancellation sequencing MUST be defined by the application protocol carried in `procedure_id` and `data`

There is no separate hook-close packet.

### 14.5 Protocol Faults

`org.unshell.protocol.v1.meta.fault` is reserved as the protocol fault `procedure_id`.

When an endpoint can attribute a protocol-level failure to a specific active hook, it SHOULD send a `Data` packet using:

- the same `hook_id`
- `procedure_id == "org.unshell.protocol.v1.meta.fault"`
- an application-independent fault payload describing the condition

At minimum, a protocol fault payload SHOULD identify a fault code and MAY include a human-readable message.

If an endpoint receives `Data` with an unknown or expired `hook_id`, it MUST discard the packet.

## 15. Introspection Payloads

**Normative**

When the required blank introspection procedure is called, it MUST return one of the following payloads through the declared hook.

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
- hooks for correlation and ongoing bidirectional interaction
- protocol faults returned through the same hook path

This is meant to make the protocol easier to reason about and easier to implement in small agents.

`procedure_id` is the main semantic anchor. In this design, the caller and callee are expected to share knowledge of what a procedure contract means. The protocol does not carry a global registry, but it does require a canonical dotted naming form so independently authored contracts remain distinguishable.

## 17. Security Considerations

**Non-Normative**

Although security is not defined by the protocol itself, implementations should treat the `Unregistered` state as a strict quarantine boundary.

Recommended behavior:

- authenticate or otherwise validate a peer before moving it to `Registered`
- rate-limit or expire idle unregistered peers
- avoid disclosing topology before admission
- avoid detailed admission failure reasons
- invalidate hooks on disconnect unless a higher-layer session mechanism exists

## 18. Serialization and Implementation Notes

**Non-Normative**

This document uses Rust-like `rkyv` struct notation to describe fields because it matches the current implementation language. The notation is explanatory. The protocol semantics are language-agnostic.

Recommended implementation limits:

| Item | Recommended limit |
|---|---|
| header length | 64 KiB |
| payload length | 64 MiB |
