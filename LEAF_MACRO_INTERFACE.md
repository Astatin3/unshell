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
- feature-gated `Leaf::update_interface()`
- feature-gated `Leaf::get_meta()`
- feature-gated `Leaf::render_ratatui()`

## Runtime Helpers

The macro delegates behavior to small helpers:

- `dispatch_session`
- `update_session_family`
- `dispatch_procedure`
- `flush_leaf_outbox`

This keeps the macro readable. The helper functions own the mechanics of session
lookup, initialization, procedure response flushing, and optional interface logging.
Sessions route their own output immediately through `Endpoint` helpers to avoid a
per-session output context and retry queue in small implant builds.

## Interface Store

`InterfaceStore` is caller-owned. It records packet flow and timing without putting
UI state inside `Endpoint` or the leaf wrapper.

```text
InterfaceStore
  events: Vec<InterfaceEvent>
  sessions: BTreeMap<SessionKey, SessionView>
  procedures: BTreeMap<ProcedureKey, ProcedureView>
```

Generated leaves receive an optional mutable store during `update_interface`. The
helpers create and update the appropriate session/procedure views when packets are
dispatched, sessions update, and queued procedure outbound routes succeed or fail.

Internally, interface events are target-driven:

```text
generated runtime
  knows packet owner
        |
        v
InterfaceTarget::Session(SessionKey)
InterfaceTarget::Procedure(ProcedureKey)
        |
        v
InterfaceStore::record(...)
  append InterfaceEvent
  link event index to exactly one view
  update SessionViewStatus when applicable
```

This is deliberately not inferred from `Packet`. A PTY session packet and a one-shot
procedure packet both have `procedure_id` and `hook_id`, but they should not both
create session views. The runtime already knows which dispatch branch handled the
packet, so that answer is carried into the store.

Leaf-level retry queues carry the same owner metadata for procedure responses.
Session responses bypass this queue and use `Endpoint::send_hook_raw` or
`Endpoint::send_hook_frame` directly.

Time remains caller-supplied:

```rust
interface.set_now_ns(Some(now_ns));
leaf.update_interface(endpoint, &mut interface);
```

No clock is embedded in the no_std protocol layer.

## Ratatui Rendering

Ratatui rendering is a plain feature-gated pass:

```rust
leaf.render_ratatui(frame, area, &mut interface);
```

Session rendering is an associated function on the stored session state type:

```rust
fn render_ratatui(
    leaf: &LeafState,
    session: &Self,
    view: &mut SessionView,
    frame: &mut ratatui::Frame<'_>,
    area: ratatui::layout::Rect,
) {
}
```

Procedure rendering is also associated and renders from leaf state plus the caller
owned procedure view.

## Why This Replaced The Proc Macro

The old proc macro had to parse attributes, infer names, generate many code paths,
and duplicate runtime logic inside codegen. That made the generator harder to reason
about than the leaf behavior it was trying to simplify.

The new design is intentionally boring:

```text
macro template -> named fields and loops
runtime helpers -> behavior
caller InterfaceStore -> UI/log state
```

That is the whole game.
