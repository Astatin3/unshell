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

If the caller wants output, it declares a hook inside the call. The recipient returns one or more `Data` packets toward the hook host. Once a hook exists, either side MAY continue exchanging `Data` packets associated with that hook until one side terminates the interaction. If normal execution cannot proceed, the endpoint MAY instead send a `Fault` packet upstream for that hook.

The protocol therefore has three core packet roles:

- `Call` for downwards invocation
- `Data` for returned data and ongoing hook traffic
- `Fault` for upstream protocol failure reporting tied to a hook

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
| Hook | A bidirectional interaction channel declared inside a `Call` and identified by `hook_id` relative to the calling endpoint that declared it. |
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

`procedure_id` MUST use the canonical dotted form `org.product.vN.part.name`, except for the reserved empty string `""` used by the required introspection procedure defined in Section 12.1, where:

- `org` identifies the owning organization or namespace root
- `product` identifies the product or system namespace
- `vN` identifies the contract version, where `N` is a positive integer written in decimal form
- `part` identifies the subsystem, leaf family, or functional area
- `name` identifies the exact procedure or payload contract name

Each segment MUST be non-empty. Implementations SHOULD restrict segments to lowercase ASCII letters, digits, and underscores for portability. The version segment MUST appear exactly in the third position.

For `Data` packets, the same `procedure_id` is used on both `Call` and `Data` packets.

> **Rationale:** `procedure_id` is intentionally stricter than a method name or content type. It identifies a full callable contract, not just a label.

## 7. Endpoint Model

**Normative**

### 7.1 Local Authority

Each endpoint enforces authority only at the connections it directly maintains.

At a local routing boundary:

- a `Call` packet MUST be accepted only if it arrives from the direct parent connection permitted to issue downwards calls into the destination subtree represented by that boundary
- a `Call` packet that violates that rule MUST be dropped silently
- a `Data` packet MAY arrive from either direction if it belongs to a valid hook flow, routes correctly by path, and its `src_path` matches the expected peer recorded in local hook state
- a `Fault` packet MAY arrive only from the subordinate side of a hook-attributable call flow, and its `src_path` MUST match the expected subordinate peer recorded in local hook state or pending call context

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
- MUST NOT execute protocol procedures received from it

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

This protocol defines exactly three packet types.

| Packet Type | Value | Meaning |
|---|---|---|
| `Call` | `0x01` | Downwards procedure invocation. |
| `Data` | `0x02` | Hook output or ongoing hook traffic. |
| `Fault` | `0xFF` | Upstream protocol failure reporting for a hook. |

Example in the current Rust implementation:

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum PacketType {
    Call = 0x01,
    Data = 0x02,
    Fault = 0xFF,
}
```

`Call` is used for downwards invocation.

`Data` is used for hook output and ongoing hook traffic.

`Fault` is used for upstream protocol failure reporting associated with a hook.

> **Rationale:** `Fault` is separated from `Data` so ordinary application output does not need to share semantics with protocol failure signaling. A receiver can distinguish successful hook traffic from protocol failure immediately from `packet_type`, without inspecting `procedure_id` or the payload contract.

## 10. Packet Header

**Normative**

| Field | Meaning |
|---|---|
| `packet_type` | Selects packet semantics. |
| `src_path` | Path of the sending endpoint. |
| `dst_path` | Path of the destination endpoint. |
| `dst_leaf` | Target leaf for a `Call`, if any. |
| `hook_id` | Hook identifier scoped to the calling endpoint that declared the hook, for hook-associated packets. |

Header rules:

- `src_path` and `dst_path` MUST be present on all packets
- the immediate receiver MUST validate that `src_path` is valid for the connection on which the packet arrived
- `dst_leaf` MUST be `None` on `Data` and `Fault`
- `hook_id` MUST be `None` on `Call`
- `hook_id` MUST appear on `Data` and `Fault`

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

Each registered endpoint path in the tree MUST be unique.

At a local routing boundary, an implementation MUST NOT maintain two registered child routes with the same claimed endpoint path.

When forwarding a packet, an implementation MUST:

1. compare `dst_path` against its locally registered child paths
2. choose the longest matching prefix
3. forward the packet toward that child if such a child exists
4. otherwise, deliver the packet locally if `dst_path` identifies the local endpoint
5. otherwise, forward the packet upward toward the direct parent connection if the destination lies outside the local endpoint's subtree
6. otherwise, drop the packet silently

The protocol defines no mandatory error packet for unresolved destinations.

> **Rationale:** Longest-prefix routing is defined as a path-selection rule, not as a way to resolve duplicate ownership. The tree model assumes each endpoint path names exactly one place in the topology. If two child routes claim the same path, the local routing table is already invalid.

### 11.2 Call Enforcement

When forwarding or receiving a `Call`, an endpoint MUST verify the local parent-child relationship at the boundary where the packet arrives.

If the sender on that connection is not the direct parent permitted to issue downwards calls into the relevant subtree, the endpoint MUST drop the packet silently.

### 11.3 Data and Fault Routing

`Data` packets are routed by `dst_path` using the same path-routing rules as `Call` packets.

The sender of a `Data` packet MUST set `dst_path` to the path of the peer endpoint for that hook packet.

`Fault` packets are routed by `dst_path` using the same path-routing rules as `Call` packets.

The sender of a `Fault` packet MUST set `dst_path` to the path of the hook host recorded in the active hook context or pending call context.

### 11.4 Hook Fastpath

An implementation MAY maintain an internal fastpath keyed by locally validated hook state for performance.

Such an optimization MUST remain behaviorally equivalent to path-based routing.

The protocol itself does not route by `hook_id` alone.

> **Rationale:** `hook_id` is scoped to the calling endpoint and is not globally routable, so path remains the canonical routing key.

## 12. Call Definition

**Normative**

| Field | Meaning |
|---|---|
| `procedure_id` | Identifier of the invoked procedure contract. |
| `data` | Application-defined procedure input payload. |
| `response_hook` | Optional hook declaration for returned data, fault delivery, and follow-on bidirectional hook traffic. |

Rules:

- the receiver MUST interpret `procedure_id` as the identifier of the procedure being invoked
- the protocol does not define argument encoding beyond raw bytes in `data`
- a `Call` that expects a result MUST include `response_hook`
- if `response_hook` is present, `response_hook.return_path` MUST be present and MUST equal `src_path`
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

The empty string `""` is reserved as the required introspection `procedure_id`.

Every endpoint MUST implement `procedure_id == ""`.

Behavior:

- when `dst_leaf` is `None`, the call requests endpoint introspection
- when `dst_leaf` is set, the call requests introspection for that specific leaf

The result MUST be returned through the declared response hook.

A `Call` with `procedure_id == ""` MUST include `response_hook`.

### 12.2 Failure Behavior

If the destination endpoint does not exist, the packet is dropped during routing.

If the destination endpoint exists but `dst_leaf` names no local leaf, the endpoint SHOULD treat the declared `response_hook`, if present, as sufficient authority to emit a protocol fault upstream even though the requested procedure cannot be executed. If no hook exists, the endpoint MUST discard the `Call` silently.

If `procedure_id` is unknown or unsupported, the endpoint SHOULD treat the declared `response_hook`, if present, as sufficient authority to emit a protocol fault upstream even though the requested procedure cannot be executed. If no hook exists, the endpoint MUST discard the `Call` silently.

> **Rationale:** Fault reporting for an invalid call would be self-defeating if the callee first had to prove that the application procedure was valid before it could use the declared hook. The hook exists to carry either normal returned data or a protocol fault explaining why normal execution could not proceed.

## 13. Hook Definition

**Normative**

Hooks are declared only inside `CallMessage.response_hook`.

There is no standalone hook-open packet.

| Field | Meaning |
|---|---|
| `hook_id` | Identifier scoped to the calling endpoint that declared the hook. |
| `return_path` | Endpoint path to which returned `Data` or `Fault` packets are sent. |

Rules:

- `hook_id` MUST be unique within the active hook set of the calling endpoint identified by `return_path`
- `return_path` MUST name the calling endpoint that hosts the hook
- a hook is declared by `response_hook` inside a `Call`
- a hook becomes active when the destination endpoint accepts that `Call` and allocates local hook state for it
- once active, either side MAY send `Data` packets associated with that hook until the interaction ends or is canceled
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
- the receiver MUST validate that `procedure_id` matches the `procedure_id` of the `Call` that established the hook
- for hook-associated `Data`, the receiver MUST validate `src_path` against the expected hook peer recorded in local hook state

> **Rationale:** Ordinary hook traffic is part of the same procedure contract that created the hook, so the returned `procedure_id` stays anchored to the originating `Call`. This keeps hook validation simple and avoids treating a response as a separate contract lookup. Introspection therefore uses `""` on both the `Call` and the `Data` it produces. Protocol faults are separate packets and therefore do not need to overload `Data` semantics.

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

A hook becomes active when the destination endpoint accepts the `Call` and allocates local hook state for the declared `response_hook`.

Once active, either side MAY send the first `Data` packet for that hook.

If an endpoint sends hook `Data` before the peer has activated local hook state for that hook, the peer MAY discard that packet as not yet attributable to an active hook.

> **Rationale:** The protocol allows symmetric hook traffic after activation, but it does not introduce a separate readiness or acknowledgment packet just to synchronize the first `Data` frame. Allowing early packets to be discarded keeps the core protocol small while making the race explicit. Higher-layer protocols that need stricter startup guarantees are expected to define their own handshake or first-packet discipline inside the hook.

Every `Data` packet for a hook MUST:

- carry the hook's `hook_id`
- set `dst_path` to the path of the peer endpoint for that hook packet

There is no protocol-level requirement that the callee send the first `Data` packet.

`hook_id` is scoped to the calling endpoint that declared and hosts that hook.

An endpoint MUST NOT reuse an active `hook_id` within its own local hook table.

After normal completion without a `Fault` packet, the protocol does not require immediate retirement or reuse of the `hook_id`. An implementation MAY retain inactive hook records for any implementation-defined period.

When allocating a new hook, an implementation SHOULD choose the lowest available inactive `hook_id`.

> **Rationale:** The protocol needs a clear uniqueness rule for active hooks, but it does not need to over-specify local allocator policy after normal completion. Some implementations may want to retain inactive entries briefly for diagnostics, duplicate suppression, or transport reordering tolerance. Recommending the lowest available inactive `hook_id` keeps allocation predictable without forcing immediate recycling.

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

After normal completion without a `Fault` packet, the moment at which a hook becomes inactive is implementation-defined unless a higher-layer protocol carried on that hook defines a stricter rule.

> **Rationale:** `end_hook` only communicates that one sender is finished. The core protocol intentionally avoids defining a universal half-close or close-ack state machine because different applications want different shutdown behavior. A simple request-response hook can retire immediately after the final packet, while a richer bidirectional protocol can define a stricter end-of-stream sequence above this layer.

### 14.5 Fault Definition

`Fault` is a distinct packet type used for protocol-level failure reporting associated with a hook.

Protocol faults are upstream-only. An endpoint MUST NOT send a `Fault` packet to a subordinate endpoint.

The `Fault` payload is the following enum identified by fixed byte discriminants:

| Fault | Value | Meaning |
|---|---|---|
| `UnknownLeaf` | `0x01` | The addressed `dst_leaf` does not exist on the destination endpoint. |
| `UnknownProcedure` | `0x02` | The destination does not support the requested `procedure_id`. |
| `InvalidCallHeader` | `0x03` | A received `Call` header was invalid for protocol processing. |
| `InvalidDataHeader` | `0x04` | A received `Data` or `Fault` header was invalid for protocol processing. |
| `InvalidSourcePath` | `0x05` | The packet `src_path` was invalid for the connection on which it arrived. |
| `InvalidHookPeer` | `0x06` | The `Data` or `Fault` sender did not match the expected peer recorded in hook state. |
| `DestinationUnreachable` | `0x07` | The packet could not be delivered to its destination subtree. |
| `HookNotActive` | `0x08` | The referenced `hook_id` was not active at the receiving endpoint. |
| `PermissionDenied` | `0x09` | The sender was not permitted to perform the requested protocol action. |
| `InternalError` | `0x0A` | The endpoint encountered an internal protocol-processing failure. |

Example in the current Rust implementation:

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct FaultMessage {
    pub fault: ProtocolFault,
}

#[repr(u8)]
#[derive(Archive, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolFault {
    UnknownLeaf = 0x01,
    UnknownProcedure = 0x02,
    InvalidCallHeader = 0x03,
    InvalidDataHeader = 0x04,
    InvalidSourcePath = 0x05,
    InvalidHookPeer = 0x06,
    DestinationUnreachable = 0x07,
    HookNotActive = 0x08,
    PermissionDenied = 0x09,
    InternalError = 0x0A,
}
```

Rules:

- a `Fault` packet MUST carry `hook_id`
- a receiver of a `Fault` packet MUST validate `src_path` against the expected subordinate hook peer recorded in local hook state or pending call context

When an endpoint can attribute a protocol-level failure to a specific hook or declared `response_hook`, it SHOULD send a `Fault` packet upstream using:

- `dst_path` set to the path of the hook host recorded in the active hook context or pending call context
- the same `hook_id`
- a `ProtocolFault` payload describing the condition

When an endpoint rejects a `Call` for `UnknownLeaf` or `UnknownProcedure` and the `Call` declared `response_hook`, the endpoint SHOULD send a `Fault` packet upstream using that declared `hook_id` and `return_path` even if normal application execution never began.

Sending a `Fault` packet ends the hook immediately. After sending or receiving a `Fault` packet, an implementation MUST remove that hook from active state.

If an endpoint receives a fault value it does not recognize, it MUST still treat the packet as a protocol fault and close the hook.

> **Rationale:** Protocol faults are part of interoperability, so they need a fixed canonical payload contract rather than a free-form error blob. A small enum with stable byte discriminants is cheap to encode, easy to evolve, and avoids coupling core protocol behavior to human-readable messages. Receivers can make deterministic decisions from the fault kind alone.

> **Rationale:** Separating `Fault` from `Data` keeps application output and protocol failure signaling visibly distinct on the wire. It also removes the need for a reserved fault `procedure_id`, because failure is already expressed by `packet_type`.

> **Rationale:** An unrecognized protocol fault still means the application contract has failed and the hook can no longer continue safely. Requiring unknown fault values to terminate the hook preserves forward compatibility: newer peers may introduce additional fault kinds without causing older peers to accidentally keep a broken hook alive.

If an endpoint receives `Data` or `Fault` with an unknown or expired `hook_id`, it MUST discard the packet.

## 15. Introspection Payloads

**Normative**

Introspection is a machine-readable discovery mechanism for hosted leaves and supported `procedure_id` values.

Introspection MUST NOT include human-readable descriptions, parameter definitions, or serialized current state.

The caller is expected to know the meaning of each discovered `procedure_id` from the pre-shared contract identified by that `procedure_id`.

When the required blank introspection procedure is called, it MUST return one of the following payloads through the declared hook.

> **Rationale:** Introspection is intentionally narrow. It tells a caller what can be called, not how to explain or rediscover the contract. `procedure_id` already names a pre-shared callable contract, so repeating human-readable descriptions, parameter metadata, or live state inside introspection would make the protocol less canonical rather than more useful.

### 15.1 Endpoint Introspection

Returned when `procedure_id == ""` and `dst_leaf == None`.

| Field | Meaning |
|---|---|
| `leaves` | List of introspection summaries for the endpoint's hosted leaves. |

Each `LeafIntrospectionSummary` contains:

| Field | Meaning |
|---|---|
| `leaf_name` | The leaf's local name. |
| `procedures` | Full canonical `procedure_id` values supported by the leaf. |

Example in the current Rust implementation:

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct EndpointIntrospection {
    pub leaves: Vec<LeafIntrospectionSummary>,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct LeafIntrospectionSummary {
    pub leaf_name: String,
    pub procedures: Vec<String>,
}
```

### 15.2 Leaf Introspection

Returned when `procedure_id == ""` and `dst_leaf` names a specific leaf.

| Field | Meaning |
|---|---|
| `leaf_name` | The leaf's local name. |
| `procedures` | Full canonical `procedure_id` values supported by the leaf. |

Example in the current Rust implementation:

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct LeafIntrospection {
    pub leaf_name: String,
    pub procedures: Vec<String>,
}
```

Rules:

- each listed procedure MUST be identified by its full canonical `procedure_id`, not by a leaf-local short name
- endpoint introspection and leaf introspection MUST use the same per-leaf discovery shape

> **Rationale:** Returning full `procedure_id` values avoids forcing the caller to reconstruct contract names from leaf-local fragments. Endpoint introspection and leaf introspection deliberately share the same leaf record shape so the endpoint-wide form is just a list of the leaf-specific form.

## 16. Protocol Description

**Non-Normative**

The UnShell protocol has a deliberately narrow center:

- addressing by path
- one downwards packet type
- one returned-data packet type
- hooks for correlation and ongoing bidirectional interaction
- protocol faults returned through their own packet type on the same hook path

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
