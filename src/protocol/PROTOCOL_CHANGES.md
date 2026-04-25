# Protocol Change Pressure

This document records protocol-spec changes that are worth considering after the
runtime rewrite in `src/protocol`.

The current rewrite intentionally keeps the existing wire model from
`/home/astatin3/Documents/GitHub/unshell/PROTOCOL.md` wherever possible. The main
goal was to remove avoidable runtime work without silently drifting the protocol.

The implementation now does the following:

- compiles child routing prefixes once instead of scanning child paths on every packet
- routes from the header first, then decodes payloads only on local delivery
- keeps pending hook state minimal and active hook state directly indexed
- separates local typed send paths from framed transport-facing send paths

Those are implementation changes. They do not require a protocol update.

## No Immediate Wire Change Required

The current runtime rewrite does **not** require a wire-format break.

The following parts of `PROTOCOL.md` remain worth keeping as-is:

- path-based routing remains the canonical behavior
- pending call context remains distinct from active hook state
- `Fault` remains upstream-only
- unknown or expired `hook_id` still drops returned traffic
- hook closure still requires both sides to send `end_hook = true`, or one `Fault`

Those rules keep the protocol boring and interoperable.

## Change 1: Framing That Guarantees Archive Alignment

### Current problem

`PROTOCOL.md` Section 8 fixes a framed format with a 4-byte big-endian length
prefix before each archived section.

That is simple, but it has one hard performance downside in the current Rust
implementation:

- the start of the archived section is not guaranteed to satisfy `rkyv` alignment
- the decoder therefore has to copy header bytes into an `AlignedVec` before safe access
- local payload decode also copies the payload bytes into another `AlignedVec`

This means the runtime still performs unavoidable memory copies during decode even
after the architectural cleanup.

### Recommended protocol change

Revise the framing rules so each archived section begins at a guaranteed aligned
offset.

Two viable options:

1. Add explicit padding after each length field so the archived section begins at
   the required alignment boundary.
2. Replace the current two-section frame with one canonical aligned envelope type
   whose internal layout already satisfies the archive alignment rules.

### Why this is objectively better

- removes the forced alignment-copy step on decode
- makes zero-copy or near-zero-copy archived access actually achievable
- reduces local delivery latency for all packet types
- reduces transient allocation pressure in the decoder

### Tradeoff

This is a wire-format change. Every compliant implementation would need to adopt
the new framing.

### Recommendation

This is the strongest protocol-level change to consider first, because the current
framing directly blocks further copy removal.

## Change 2: Compact Path Representation for a Future v2

### Current problem

`PROTOCOL.md` Sections 5, 6, 10, 11, and 13 make paths canonical on the wire as
`Vec<String>` values.

That is easy to understand and debug, but it imposes real cost:

- path routing requires segment-wise string comparison
- hook state keys carry owned path vectors
- packets repeat full path strings over and over
- the runtime must repeatedly compare or clone path structures at boundaries

The new implementation minimizes those costs internally, but it cannot eliminate
them while the wire format remains path-string based.

### Recommended protocol change

For a future protocol version, consider separating:

- the canonical human-readable control/discovery layer
- the compact transport/runtime layer

The compact transport/runtime layer would use stable numeric endpoint IDs instead
of repeated `Vec<String>` path payloads.

### Why this is objectively better

- routing becomes integer-based instead of string-prefix based
- hook keys become compact and cheap to index
- packets shrink
- path comparisons and many path clones disappear from the hot path

### Tradeoff

This is a full protocol-versioning decision, not a local cleanup.

It adds coordination costs:

- peers must agree on endpoint IDs
- topology updates become more structured
- the protocol becomes less self-describing on the wire

### Recommendation

Do **not** make this change as a silent update to the current protocol.

If pursued, it should be introduced explicitly as a `v2` protocol, because it is
no longer behaviorally equivalent to the current path-based wire model.

## Change 3: Clarify Caller-Side Hook Activation Semantics

### Current problem

`PROTOCOL.md` Section 13 is explicit about callee-side pending call context, but
it leaves more room for interpretation on the caller side after a `Call` is sent.

The current runtime keeps caller-side hook state available immediately after send
so it can validate returned traffic efficiently.

That is practical, but the spec could be clearer about whether the caller's local
hook record is considered active immediately, or merely reserved until the callee
accepts.

### Recommended protocol change

Clarify caller-side wording in Section 13 so implementations know whether the
caller may allocate directly into active host state after sending a `Call`, as
long as early returned `Data` for an actually inactive hook is still discarded per
Section 14.1.

### Why this is objectively better

- removes ambiguity for optimized runtimes
- makes caller-side hook bookkeeping more consistent across implementations
- avoids accidental spec drift through inference

### Tradeoff

This is a clarification change, not necessarily a wire-format change.

## Summary

The runtime rewrite shows that most of the original performance problems were
architectural, not inherent to the protocol.

The current protocol can support a much lower-loop implementation than before.

The main remaining protocol-level blocker is the framing/alignment rule. That is
the one change most worth making if the next goal is to reduce unavoidable memory
copies further.
