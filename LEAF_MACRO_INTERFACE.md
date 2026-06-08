# Template Leaf Interface Design

**Status:** Implemented draft  
**Last updated:** 2026-05-31  
**Primary use case:** Small generated leaf wrappers without proc-macro machinery

## Summary

Leaf generation now uses a declarative `unshell_leaf!` template instead of the old
`#[unshell_leaf]` proc macro. The goal is to make generated code obvious, closer to
an HTML template than an AST transformation.

The macro only fills slots:

- wrapper name
- user state type
- leaf id
- interface metadata
- named session families
- named procedure families

All real behavior lives in normal Rust helpers under `src/protocol/runtime.rs`.
Those helpers are testable without macro parsing, `syn`, `quote`, or generated name
inference.

## User Shape

```rust
pub struct FakePtyState {
    pub active_count: usize,
    pub total_opened: u64,
}

unshell_leaf! {
    pub leaf FakePtyLeaf for FakePtyState {
        id: LEAF_FAKE_PTY,
        meta: unshell::protocol::LeafMeta {
            name: "Fake PTY Leaf",
            identifier: "dev.unshell.v1.pty",
            version: "v0",
            authors: unshell::alloc::vec!["ASTATIN3"],
        },
        sessions {
            pty: PtySessionState,
        }
        procedures {}
    }
}
```

The field name before each session type is explicit. The macro does not invent a
field name from the Rust type.

## Generated Shape

The example above expands to the equivalent of:

```rust
pub struct FakePtyLeaf {
    state: FakePtyState,
    outbox: LeafOutbox,
    pty: SessionFamily<PtySessionState>,
}
```

Session types are the per-hook state values themselves. There is no separate
zero-sized handler struct; a type like `PtySessionState` implements `Session` and is
stored directly in the generated `SessionFamily`.

The wrapper implements:

- `new(state)`
- `state()`
- `state_mut()`
- `active_session_count()`
- `pending_packet_count()`
- `Leaf::get_id()`
- `Leaf::update()`
- feature-gated `Leaf::get_meta()`
- feature-gated `Leaf::render_ratatui()`

## Runtime Helpers

The macro delegates behavior to small helpers:

- `dispatch_session`
- `update_session_family`
- `dispatch_procedure`
- `flush_leaf_outbox`

This keeps the macro readable. The helper functions own the mechanics of session
lookup, initialization, and procedure response flushing. Sessions route their own
output immediately through `Endpoint` helpers to avoid a per-session output context
and retry queue in small implant builds.

## Interface Direction

The old caller-owned interface store has been removed. It mixed event tracing,
session/procedure render buckets, timestamps, and retry ownership into one global
object, which made the frontend path more complicated than the leaf state it was
trying to expose.

The replacement direction is direct backend-specific rendering from the state that
already owns the behavior:

```text
Leaf wrapper
  leaf state
  generated session families
  generated procedure families
        |
        v
feature-gated render method for the selected frontend backend
```

TODO(interface-ratatui): add a small Ratatui render context under `src/interface`
and pass it through `Leaf::render_ratatui`, `Session::render_ratatui`, and
`Procedure::render_ratatui`. The context should describe render target metadata; it
must not become another packet-flow database.

## Ratatui Rendering

Ratatui rendering is a plain feature-gated pass:

```rust
leaf.render_ratatui(frame, area);
```

Session rendering is an associated function on the stored session state type:

```rust
fn render_ratatui(
    leaf: &LeafState,
    session: &Self,
    frame: &mut ratatui::Frame<'_>,
    area: ratatui::layout::Rect,
) {
}
```

Procedure rendering is also associated and renders from leaf state. The current
signature is intentionally minimal until the replacement Ratatui context is added.

## Why This Replaced The Proc Macro

The old proc macro had to parse attributes, infer names, generate many code paths,
and duplicate runtime logic inside codegen. That made the generator harder to reason
about than the leaf behavior it was trying to simplify.

The new design is intentionally boring:

```text
macro template -> named fields and loops
runtime helpers -> behavior
feature-gated render methods -> UI adapters
```

That is the whole game.
