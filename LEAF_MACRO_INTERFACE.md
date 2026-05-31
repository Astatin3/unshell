# Macro-Generated Leaf Interface Design

**Status:** Draft  
**Last updated:** 2026-05-28  
**Primary use case:** Remote PTY sessions over hook-backed UnShell packets

## Summary

This document proposes a generated leaf interface for UnShell. The goal is to make
stateful leaves, such as a remote PTY leaf, easy to write without forcing every leaf
author to hand-code packet draining, procedure dispatch, session lookup, hook
lifetime handling, and retry-safe final-frame cleanup.

The user writes the application logic:

- the leaf state struct
- one or more session types for long-running hook-backed conversations
- one or more procedure types for one-packet operations
- payload encoding and decoding
- OS-specific behavior, such as spawning or polling a PTY

The macro generates the plumbing:

- the `Leaf` implementation
- the generated wrapper that owns session stores and retry queues
- inbound packet filtering for the leaf's procedure ids
- per-packet procedure dispatch
- per-hook session dispatch
- retry-safe outbound flushing
- final-frame session removal only after routing succeeds

The macro should generate ordinary Rust. No runtime registry, no boxed procedure
objects, and no hidden dynamic dispatch in the hot path.

## Problem

The current `Leaf` trait is deliberately small:

```rust
pub trait Leaf {
    fn get_id(&self) -> u32;

    fn update(&mut self, endpoint: &mut Endpoint);
}
```

That makes the protocol runtime flexible, but it also means every non-trivial leaf
has to solve the same set of problems by hand:

- select only the inbound packets that belong to this leaf
- distinguish one-shot procedures from long-running sessions
- group session packets by `hook_id`
- keep per-session application state
- build response packets with the right `hook_id`, path, procedure id, and `end_hook`
- retry failed outbound packets without losing stream progress
- remove session state only after final packets route successfully
- avoid consuming packets intended for other leaves

A remote PTY leaf makes this pain obvious. A PTY session is bidirectional and long
lived. The same hook carries `Open`, `Input`, `Resize`, `StdinEof`, `Output`, `Exit`,
and errors. The endpoint already owns hook authorization, but the leaf still needs a
safe session state machine.

## Goals

- Automate the repetitive `Leaf::update` machinery.
- Keep application logic explicit and testable.
- Keep `unshell-protocol` minimal and no_std-friendly.
- Keep OS-specific PTY code outside `unshell-protocol`.
- Preserve the new endpoint hook model: downward packets pave hooks, final packets close hooks.
- Make final-frame retries hard to get wrong.
- Keep generated runtime code static and size-conscious.
- Let sessions and procedures mutate shared leaf state without directly accessing each other.
- Make multiple sessions of the same type cheap and predictable.

## Non-Goals

- Do not define a full actor framework.
- Do not add async requirements to the protocol layer.
- Do not make sessions discover or mutate other sessions directly.
- Do not introduce `Vec<Box<dyn Procedure>>` or `Vec<Box<dyn Session>>` runtime registries.
- Do not hide PTY business logic inside the macro.
- Do not add source-path fields to `Packet` just for PTY. The PTY `Open` payload can carry the reply path.

## Current Protocol Assumptions

The endpoint routing layer now owns hook lifetime:

```text
validated downward packet, end_hook=false -> open or refresh peer-bound hook
validated downward packet, end_hook=true  -> close hook after successful route or delivery
validated upward packet                  -> require matching hook
validated upward packet, end_hook=true   -> close hook after successful route or delivery
```

For PTY, this means:

- `Open` uses `end_hook = false` because it expects returned output.
- `Input`, `Resize`, `StdinEof`, and `Terminate` use `end_hook = false`.
- `StdinEof` is not `end_hook`. EOF closes stdin, not the whole PTY session.
- `Abort` may use `end_hook = true` if no acknowledgement is expected.
- `Output` uses `end_hook = false`.
- `Exit` or fatal `Error` uses `end_hook = true`.

## Crate Boundary

```text
unshell-protocol
  Endpoint
  Packet
  Leaf
  hook routing rules
  filtered inbound drain API

unshell-runtime or unshell-leaves
  real PTY worker implementation
  std-only integrations
  portable-pty adapter

unshell-macros
  tiny proc-macro shim

unshell-macros-core
  syn, quote, proc-macro2, deluxe or darling
  parser and code generator tests
```

The generated code runs in the final binary. Macro parsing dependencies do not.
That means `syn`, `quote`, `deluxe`, and `darling` are acceptable in the macro crates,
but not in the protocol runtime.

## Required Endpoint Addition

The generated leaf must be able to drain only the packets it owns. The current
`take_inbound_clear` drains a whole local queue, which is unsafe once multiple
application leaves share an endpoint.

Add a filtered drain API:

```rust
impl Endpoint {
    /// Drains packets from `local_id` that match `predicate`, preserving all other
    /// packets in their original relative order.
    pub fn take_inbound_matching<P, F>(&mut self, local_id: u32, predicate: P, f: F)
    where
        P: FnMut(&Packet) -> bool,
        F: FnMut(&Packet);
}
```

For generated leaves, matching usually means:

```rust
packet.procedure_id == PROC_PTY
    || packet.procedure_id == PROC_PING
    || packet.procedure_id == PROC_CAPABILITIES
```

This is the one endpoint API the macro needs before it can be safe in mixed-leaf endpoints.

## User-Facing Macro

Use an attribute macro that wraps a user-owned state struct and generates a leaf type.
A derive macro alone cannot add storage fields to the original struct, so the macro
must generate a companion wrapper.

```rust
#[unshell_leaf(
    leaf = RemotePtyLeaf,
    id = LEAF_REMOTE_PTY,
    sessions(PtySession),
    procedures(PingProcedure, CapabilitiesProcedure)
)]
pub struct RemotePtyState {
    max_sessions: usize,
    default_rows: u16,
    default_cols: u16,
}
```

The macro emits roughly:

```rust
pub struct RemotePtyLeaf {
    state: RemotePtyState,
    pty_sessions: SessionStore<PtySession>,
    retry: PacketQueue,
}

impl RemotePtyLeaf {
    pub fn new(state: RemotePtyState) -> Self { ... }
}

impl Leaf for RemotePtyLeaf {
    fn get_id(&self) -> u32 {
        LEAF_REMOTE_PTY
    }

    fn update(&mut self, endpoint: &mut Endpoint) {
        ... generated dispatch ...
    }
}
```

The leaf wrapper is the object stored in `Endpoint::leaves`. The state struct stays
small and owned by the user.

## Core Model

```text
Endpoint inbound queue
        |
        v
+-------------------------------+
| generated RemotePtyLeaf       |
|                               |
|  state: RemotePtyState        |
|  sessions: SessionStore       |
|  retry: PacketQueue           |
+-------------------------------+
        |
        +--> Procedure packet -> Procedure::handle(...)
        |
        +--> Session packet   -> by hook_id
                                  create or update session
                                  session queues outbound frames
                                  generated flush handles retry
```

Procedures and sessions can both mutate `RemotePtyState`. They cannot directly
borrow or mutate each other. Communication between them must happen through leaf
state or through packets.

## Sessions

A session is a long-running hook-backed conversation. PTY is the main example.

```rust
pub trait Session<L> {
    /// All packets for this session type use this outer procedure id.
    const PROCEDURE_ID: u32;

    /// State owned for one active hook/session.
    type State;

    /// Attempts to create a new session from an incoming packet.
    fn init(
        leaf: &mut L,
        packet: Packet,
        ctx: &mut SessionInit,
    ) -> SessionInitResult<Self::State>;

    /// Advances one session. The generated leaf passes all queued packets for this
    /// hook and one context that can enqueue outbound frames.
    fn update(
        leaf: &mut L,
        session: &mut Self::State,
        incoming: &mut PacketQueue,
        ctx: &mut SessionCtx,
    ) -> SessionStatus;
}
```

`L` is the user state type, for example `RemotePtyState`.

### Session Init Context

```rust
pub struct SessionInit {
    hook_id: HookID,
    packet_path: Vec<u32>,
}

pub enum SessionInitResult<S> {
    Created(S),
    Rejected,
    RejectedWith(Packet),
}
```

`RejectedWith(Packet)` is intended for cases where the initializer can build a
protocol-level failure response, such as "too many PTY sessions". The generated leaf
still owns routing and retry for that packet.

The PTY `Open` payload should include the caller reply path because `Packet` does
not currently carry source path.

```text
Open payload:
  opcode
  reply_path_len
  reply_path segments
  rows
  cols
  command/env/options
```

The session stores that reply path and uses it for upward output packets.

### Session Update Context

Sessions should use a context wrapper rather than directly constructing packets.
The context can still carry restricted endpoint access when absolutely necessary,
but the normal output path should be helper methods.

```rust
pub struct SessionCtx<'a> {
    endpoint: &'a mut Endpoint,
    hook_id: HookID,
    reply_path: &'a [u32],
    procedure_id: u32,
    outbox: &'a mut PacketQueue,
}
```

Helpers:

```rust
impl<'a> SessionCtx<'a> {
    pub fn send(&mut self, opcode: u8, data: &[u8]);

    pub fn send_final(&mut self, opcode: u8, data: &[u8]);

    pub fn error(&mut self, code: u8, data: &[u8]);

    pub fn error_final(&mut self, code: u8, data: &[u8]);

}
```

These helpers build packets like:

```rust
Packet {
    hook_id: self.hook_id,
    end_hook,
    path: self.reply_path.to_vec(),
    procedure_id: self.procedure_id,
    data: encode_frame(opcode, payload),
}
```

The helper only queues packets. It does not route them immediately. The generated
leaf owns flushing and retry.

### Session Status

```rust
pub enum SessionStatus {
    Running,
    Closing,
    Closed,
}
```

`Closed` means the session has no more application work. The generated leaf still
must keep the session until any final packet has routed successfully.

## Procedures

A procedure is a one-packet operation. It is appropriate for introspection, ping,
capabilities, and simple state queries.

```rust
pub trait Procedure<L> {
    const PROCEDURE_ID: u32;

    fn handle(
        leaf: &mut L,
        endpoint: &mut Endpoint,
        packet: Packet,
        out: &mut ProcedureOut,
    );
}
```

Procedure helpers mirror session helpers but operate on one incoming packet:

```rust
pub struct ProcedureOut {
    hook_id: HookID,
    reply_path: Vec<u32>,
    procedure_id: u32,
    outbox: PacketQueue,
}

impl ProcedureOut {
    pub fn send(&mut self, data: &[u8]);

    pub fn send_final(&mut self, data: &[u8]);

}
```

Procedures do not directly access sessions. If a procedure needs information about
sessions, that information should be mirrored into leaf state by the session code.

## PTY Binary Protocol

One PTY session uses one hook and one outer procedure id. The inner PTY protocol is
a tiny binary frame in `Packet::data`.

If API names use `end_state` at the session layer, it maps directly to
`Packet::end_hook` at the protocol layer.

```text
Packet {
    hook_id: session hook,
    procedure_id: PROC_PTY,
    data: [opcode, payload...],
    end_hook: session lifetime marker,
}
```

Suggested opcodes:

| Opcode | Direction | Meaning | end_hook |
|---:|---|---|---|
| 0 | Downward | Open PTY | false |
| 1 | Upward | Opened | false |
| 2 | Downward | Input bytes | false |
| 3 | Downward | Resize | false |
| 4 | Downward | Stdin EOF | false |
| 5 | Downward | Terminate process | false |
| 6 | Downward | Abort session without acknowledgement | true |
| 7 | Upward | Output bytes | false |
| 8 | Upward | Exit status | true |
| 9 | Upward | Fatal error | true |

`StdinEof` must not set `end_hook = true`. The remote process may still emit output
after stdin closes.

## Generated Update Loop

The generated `Leaf::update` should follow this shape:

```text
update(endpoint)
  1. flush retry queue first
  2. drain matching inbound packets only
  3. dispatch procedure packets
  4. group session packets by hook_id
  5. create sessions for open packets
  6. update active sessions
  7. flush outbound frames
  8. remove sessions whose final packet routed successfully
```

More concrete flow:

```text
+----------------------------+
| generated update           |
+----------------------------+
       |
       v
flush retry packets
       |
       v
take_inbound_matching(local_id, owns_packet)
       |
       +--> procedure id match -> Procedure::handle
       |
       +--> session id match   -> session inbox by hook_id
       |
       v
for each session inbox
       |
       +--> existing hook -> Session::update
       |
       +--> no hook       -> Session::init, then Session::update
       |
       v
flush session/procedure outbox
       |
       +--> route success -> advance/remove when safe
       |
       +--> route failure -> keep retry packet and keep state
```

The retry rule is the most important generated invariant:

```text
If endpoint.add_outbound(packet) fails, the generated leaf must not:
  - drop the packet
  - advance a final frame
  - remove the session
  - consume state that cannot be reconstructed
```

## Generated Dispatch

The macro should emit static matches, not runtime registries.

Good:

```rust
match packet.procedure_id {
    PtySession::PROCEDURE_ID => self.dispatch_pty_session(packet),
    PingProcedure::PROCEDURE_ID => PingProcedure::handle(...),
    CapabilitiesProcedure::PROCEDURE_ID => CapabilitiesProcedure::handle(...),
    _ => self.handle_unknown(packet),
}
```

Avoid:

```rust
Vec<Box<dyn Procedure>>
Vec<Box<dyn Session>>
```

Static dispatch keeps the generated code visible to LLVM and avoids registry setup
costs in constrained binaries.

## Session Store

The first implementation can use a small `Vec` or `VecDeque` store.

```rust
pub struct SessionEntry<S> {
    hook_id: HookID,
    state: S,
    inbox: PacketQueue,
    outbox: PacketQueue,
    retry: Option<Packet>,
    closing: bool,
}
```

For minimal binaries, a later implementation can replace this with a fixed-capacity
table under a feature flag.

Required operations:

- find by `hook_id`
- insert if capacity allows
- push incoming packet to inbox
- enqueue outbound packet
- retain session while retry packet exists
- remove only after session is closed and outbox/retry are empty

## Unknown Packets

The generated leaf should have configurable unknown-packet behavior.

Default behavior:

- unknown procedure id remains in the endpoint queue because `take_inbound_matching` does not drain it
- unknown session opcode for a known session produces a fatal error frame and closes the session
- packet for unknown hook under a session procedure produces a fatal error frame if the payload is not a valid `Open`

For PTY:

```text
unknown hook + Open opcode       -> create session
unknown hook + non-Open opcode   -> Error end_hook=true
known hook   + known opcode      -> session update
known hook   + unknown opcode    -> Error end_hook=true
```

## Access Boundaries

Sessions and procedures can mutate leaf state:

```rust
fn update(leaf: &mut RemotePtyState, ...)
```

They should not directly access each other:

```text
Procedure -> no direct SessionStore access
Session   -> no direct Procedure access
```

If cross-cutting data is needed, mirror it into `RemotePtyState`:

```rust
pub struct RemotePtyState {
    active_count: usize,
    total_spawned: u64,
    max_sessions: usize,
}
```

This keeps borrowing simple and avoids turning the generated leaf into a shared
mutable object graph.

## Endpoint Access

The user proposed passing `&mut Endpoint` to sessions and procedures. The safest
design is slightly narrower:

- procedures may receive `&mut Endpoint` because they handle one packet and return immediately
- sessions should receive `SessionCtx`, which can expose narrow endpoint helpers
- a raw endpoint escape hatch can be added later if a real leaf proves it needs one

Rationale: sessions are long-lived and retry-sensitive. If session code calls
`endpoint.add_outbound` directly, it can bypass generated retry handling and lose a
final packet. The helper path keeps the dangerous part centralized.

## Remote PTY Example

User code:

```rust
#[unshell_leaf(
    leaf = RemotePtyLeaf,
    id = LEAF_REMOTE_PTY,
    sessions(PtySession),
    procedures(PtyCapabilities)
)]
pub struct RemotePtyState {
    max_sessions: usize,
    default_rows: u16,
    default_cols: u16,
}

pub struct PtySession;

pub struct PtyState {
    hook_id: HookID,
    reply_path: Vec<u32>,
    worker: PtyWorker,
    saw_stdin_eof: bool,
}

impl Session<RemotePtyState> for PtySession {
    const PROCEDURE_ID: u32 = PROC_PTY;

    type State = PtyState;

    fn init(
        leaf: &mut RemotePtyState,
        packet: Packet,
        ctx: &mut SessionInit,
    ) -> SessionInitResult<Self::State> {
        let open = decode_open(&packet.data)?;

        if leaf.active_count >= leaf.max_sessions {
            return SessionInitResult::RejectedWith(pty_error_busy(packet.hook_id));
        }

        let worker = PtyWorker::spawn(open.command, open.rows, open.cols)?;

        SessionInitResult::Created(PtyState {
            hook_id: ctx.hook_id(),
            reply_path: open.reply_path,
            worker,
            saw_stdin_eof: false,
        })
    }

    fn update(
        leaf: &mut RemotePtyState,
        session: &mut Self::State,
        incoming: &mut PacketQueue,
        ctx: &mut SessionCtx,
    ) -> SessionStatus {
        while let Some(packet) = incoming.pop_front() {
            match decode_pty_frame(&packet.data) {
                PtyFrame::Input(bytes) => session.worker.write(&bytes),
                PtyFrame::Resize { rows, cols } => session.worker.resize(rows, cols),
                PtyFrame::StdinEof => {
                    session.saw_stdin_eof = true;
                    session.worker.close_stdin();
                }
                PtyFrame::Terminate => session.worker.terminate(),
                PtyFrame::Abort => return SessionStatus::Closed,
                _ => ctx.error_final(ERR_BAD_OPCODE, b"bad pty opcode"),
            }
        }

        while let Some(bytes) = session.worker.poll_output() {
            ctx.send(OP_OUTPUT, &bytes);
        }

        if let Some(status) = session.worker.poll_exit() {
            ctx.send_final(OP_EXIT, &encode_exit(status));
            leaf.active_count -= 1;
            return SessionStatus::Closed;
        }

        SessionStatus::Running
    }
}
```

The exact error handling syntax above is illustrative. The final API should avoid
`?` unless `SessionInitResult` has a clear conversion story.

## Generated Remote PTY Flow

```text
Caller                         Remote endpoint                 RemotePtyLeaf
------                         ---------------                 -------------
allocate hook
Open end=false  -------------> downward route opens hook ----> init PtyState
Input false    -------------> hook remains active -----------> write PTY stdin
Resize false   -------------> hook remains active -----------> resize PTY
StdinEof false -------------> hook remains active -----------> close PTY stdin

Output false   <------------- upward route requires hook <---- ctx.send
Output false   <------------- upward route requires hook <---- ctx.send
Exit true      <------------- upward route closes hook   <---- ctx.send_final

caller cleanup                                                   remove session
```

## Proc Macro Implementation

Rust still requires a proc-macro crate for real attribute macros. Keep that crate
thin and put the real logic in a normal testable crate.

```text
unshell-macros
  #[proc_macro_attribute]
  pub fn unshell_leaf(attr, item) -> TokenStream

unshell-macros-core
  parse UnshellLeafArgs
  parse state struct
  generate wrapper
  generate Leaf impl
  generate dispatch methods
  tests over proc_macro2::TokenStream
```

Recommended parser helper:

- `deluxe` for attribute parsing if the desired syntax uses richer Rust tokens
- `darling` if the syntax stays close to normal Rust meta attributes

Current recommendation: use `deluxe` for attribute parsing and `syn`/`quote` for
code generation.

## Generated Code Requirements

- Must compile without requiring std in `unshell-protocol`.
- Must not allocate handler trait objects.
- Must preserve generic parameters and where clauses on the state struct.
- Must emit useful compile errors for duplicate procedure ids.
- Must emit useful compile errors for duplicate session procedure ids.
- Must reject session and procedure id collisions unless explicitly allowed.
- Must preserve user attributes that are not consumed by the macro.
- Must not require users to manually implement `Leaf` for the generated wrapper.

## Testing Strategy

Phase 1 tests should use a fake PTY session, not `portable-pty`.

Protocol tests:

- `open_pty_paves_hook_and_creates_session`
- `input_and_output_share_one_hook`
- `stdin_eof_keeps_hook_until_exit`
- `exit_end_hook_cleans_route_and_session`
- `failed_final_exit_route_retries_without_losing_session`
- `abort_downward_end_hook_closes_without_ack`
- `unknown_session_input_returns_error_end_hook`
- `two_pty_sessions_interleave_without_crossing_hooks`
- `pty_leaf_does_not_consume_other_leaf_packets`

Macro tests:

- generated leaf implements `Leaf`
- duplicate procedure ids fail at compile time
- duplicate session procedure ids fail at compile time
- generic state structs expand correctly
- generated code routes final frames retry-safely
- generated code preserves unmatched inbound packets

Use `trybuild` for compile-fail macro tests once `unshell-macros` exists.

## Rollout Plan

1. Add `Endpoint::take_inbound_matching`.
2. Write a manual fake PTY leaf that follows the exact generated shape.
3. Add PTY session tests against the manual fake leaf.
4. Create `unshell-macros` and `unshell-macros-core`.
5. Generate the same shape as the manual fake leaf.
6. Port fake PTY tests to the generated leaf.
7. Add compile-fail macro tests.
8. Implement the real std-only PTY worker in `unshell-leaves` or `unshell-runtime`.
9. Add integration tests for the real worker where the platform supports PTYs.

This avoids writing a macro against an unproven design. First make the generated
shape real by hand, then teach the macro to emit it.

## Open Questions

- Should `SessionCtx` expose any raw `Endpoint` access, or only narrow helpers?
- Should session stores be `Vec` first, or should fixed-capacity storage be designed immediately?
- Should unknown opcodes produce an error packet by default, or should each session type decide?
- Should `Open` always carry `reply_path`, or should the packet format eventually add source path?
- Should `ProcedureOut` be retry-safe like session output, or are procedures allowed to fail fast?
- Should macro-generated leaves expose counters for active sessions and retry queue depth?

## Recommendation

Implement this as static generated code with a narrow context API.

Do not build a runtime plugin system. Do not give sessions raw access to session
stores. Do not let session code bypass generated output flushing. The macro should
make the correct routing and hook behavior the easiest path, especially for final
frames.

The first proof point should be fake PTY. If fake PTY works cleanly, real PTY is
mostly OS plumbing.
