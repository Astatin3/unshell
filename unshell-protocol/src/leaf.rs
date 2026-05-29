use crate::{Endpoint, HookID, Packet, PacketQueue};

use alloc::vec::Vec;

/// Application extension point hosted by an [`Endpoint`].
///
/// A leaf owns product-specific state and reacts to packets that endpoint routing has
/// already delivered locally. The trait intentionally stays small so handwritten
/// leaves, generated leaves, and test leaves can all share the same endpoint loop.
pub trait Leaf {
    /// Returns the stable local identifier for this leaf implementation.
    fn get_id(&self) -> u32;

    /// Advances the leaf by one endpoint update tick.
    ///
    /// Implementations normally drain matching inbound packets, mutate leaf-owned
    /// state, then enqueue outbound packets with [`Endpoint::add_outbound`].
    fn update(&mut self, _: &mut Endpoint);
}

/// Contract implemented by one hook-backed generated session family.
///
/// A session family maps one outer `procedure_id` to many live hook instances. The
/// generated leaf owns packet grouping, retry-safe output flushing, and final cleanup;
/// the session implementation owns only application behavior.
///
/// # Example
///
/// ```rust,ignore
/// impl Session<MyLeafState> for MySession {
///     const PROCEDURE_ID: u32 = 7;
///     type State = MySessionState;
///
///     fn reply_path(state: &Self::State) -> &[u32] {
///         &state.reply_path
///     }
///
///     fn init(
///         leaf: &mut MyLeafState,
///         packet: Packet,
///         ctx: &mut SessionInit,
///     ) -> SessionInitResult<Self::State> {
///         SessionInitResult::Created(MySessionState::from_open(leaf, packet, ctx))
///     }
///
///     fn update(
///         leaf: &mut MyLeafState,
///         session: &mut Self::State,
///         incoming: &mut PacketQueue,
///         ctx: &mut SessionCtx<'_>,
///     ) -> SessionStatus {
///         while let Some(packet) = incoming.pop_front() {
///             session.apply(leaf, packet, ctx);
///         }
///         SessionStatus::Running
///     }
/// }
/// ```
pub trait Session<L> {
    /// Outer packet procedure id used by every packet in this session family.
    const PROCEDURE_ID: u32;

    /// Application state stored for one live hook.
    type State;

    /// Returns the destination path for responses emitted by this session.
    ///
    /// `Packet` currently carries only a destination path, so protocols that need to
    /// reply to a caller should capture a reply path during [`Self::init`]. The
    /// generated leaf clones this path into [`SessionCtx`] before calling update so
    /// session code can mutably borrow its state while emitting frames.
    fn reply_path(session: &Self::State) -> &[u32];

    /// Creates one session state from a packet whose hook has no active session.
    ///
    /// Returning [`SessionInitResult::RejectedWith`] lets the generated leaf route a
    /// protocol-level failure response with the same retry guarantees as normal
    /// output. Returning [`SessionInitResult::Rejected`] silently consumes the packet.
    fn init(leaf: &mut L, packet: Packet, ctx: &mut SessionInit) -> SessionInitResult<Self::State>;

    /// Advances one active hook session.
    ///
    /// The generated leaf calls this for every live session on each update tick so
    /// sessions can poll external workers even when no new packet arrived. Outbound
    /// packets must be queued through `ctx`; direct endpoint routing would bypass the
    /// generated retry rules.
    fn update(
        leaf: &mut L,
        session: &mut Self::State,
        incoming: &mut PacketQueue,
        ctx: &mut SessionCtx<'_>,
    ) -> SessionStatus;
}

/// Contract implemented by one generated one-packet procedure handler.
///
/// Procedures are for stateless or short-lived operations such as ping, capabilities,
/// or health checks. Long-running conversations should use [`Session`] so final
/// packet cleanup and retries remain tied to hook state.
pub trait Procedure<L> {
    /// Outer packet procedure id handled by this procedure.
    const PROCEDURE_ID: u32;

    /// Handles one packet and optionally queues response packets in `out`.
    fn handle(leaf: &mut L, endpoint: &mut Endpoint, packet: Packet, out: &mut ProcedureOut);
}

/// Context passed to [`Session::init`].
///
/// This carries routing metadata that the generated leaf already knows before the
/// session state exists. Protocols that need source paths should encode them in the
/// packet payload; `packet_path` is the destination path that routed the packet here.
pub struct SessionInit {
    hook_id: HookID,
    packet_path: Vec<u32>,
}

impl SessionInit {
    /// Creates initialization metadata for a delivered packet.
    pub fn new(hook_id: HookID, packet_path: Vec<u32>) -> Self {
        Self {
            hook_id,
            packet_path,
        }
    }

    /// Returns the hook id that will identify the new session.
    pub fn hook_id(&self) -> HookID {
        self.hook_id
    }

    /// Returns the destination path from the packet that reached this leaf.
    pub fn packet_path(&self) -> &[u32] {
        &self.packet_path
    }
}

/// Result of trying to create a session from a packet without an active hook entry.
pub enum SessionInitResult<S> {
    /// A new session was created and should be stored by the generated leaf.
    Created(S),

    /// The packet was intentionally consumed without creating state or a response.
    Rejected,

    /// The packet was rejected with a response that the generated leaf must route.
    RejectedWith(Packet),
}

/// Session lifecycle status returned from [`Session::update`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    /// The session is active and should receive future update ticks.
    Running,

    /// The session is winding down but still needs future update ticks.
    Closing,

    /// The session has finished application work.
    ///
    /// The generated leaf still retains the entry until every queued packet routes
    /// successfully, which prevents a failed final frame from losing session cleanup.
    Closed,
}

/// Mutable output context passed to [`Session::update`].
///
/// The context queues packets only; it never routes them immediately. Centralizing
/// routing in generated code is what makes final-frame retries reliable.
pub struct SessionCtx<'a> {
    hook_id: HookID,
    reply_path: Vec<u32>,
    procedure_id: u32,
    outbox: &'a mut PacketQueue,
}

impl<'a> SessionCtx<'a> {
    /// Creates a context for one session update call.
    pub fn new(
        hook_id: HookID,
        reply_path: Vec<u32>,
        procedure_id: u32,
        outbox: &'a mut PacketQueue,
    ) -> Self {
        Self {
            hook_id,
            reply_path,
            procedure_id,
            outbox,
        }
    }

    /// Returns the hook id used for packets emitted through this context.
    pub fn hook_id(&self) -> HookID {
        self.hook_id
    }

    /// Returns the destination path used for packets emitted through this context.
    pub fn reply_path(&self) -> &[u32] {
        &self.reply_path
    }

    /// Queues a one-byte-opcode frame without closing the hook.
    pub fn send(&mut self, opcode: u8, data: &[u8]) {
        self.send_frame(opcode, data, false);
    }

    /// Queues a one-byte-opcode frame that closes the hook after successful routing.
    pub fn send_final(&mut self, opcode: u8, data: &[u8]) {
        self.send_frame(opcode, data, true);
    }

    /// Queues a protocol-specific error frame without closing the hook.
    ///
    /// The `code` is used as the frame opcode because the protocol layer does not
    /// reserve a universal error opcode. Leaves that have a dedicated error opcode can
    /// pass that value here or call [`Self::send`] directly.
    pub fn error(&mut self, code: u8, data: &[u8]) {
        self.send(code, data);
    }

    /// Queues a protocol-specific error frame that closes the hook after routing.
    pub fn error_final(&mut self, code: u8, data: &[u8]) {
        self.send_final(code, data);
    }

    /// Queues raw packet data without adding an opcode byte.
    pub fn send_raw(&mut self, data: &[u8]) {
        self.send_raw_with_end(data, false);
    }

    /// Queues raw packet data and closes the hook after successful routing.
    pub fn send_raw_final(&mut self, data: &[u8]) {
        self.send_raw_with_end(data, true);
    }

    fn send_frame(&mut self, opcode: u8, data: &[u8], end_hook: bool) {
        let mut frame = Vec::with_capacity(data.len() + 1);
        frame.push(opcode);
        frame.extend_from_slice(data);
        self.enqueue_data(frame, end_hook);
    }

    fn send_raw_with_end(&mut self, data: &[u8], end_hook: bool) {
        self.enqueue_data(data.to_vec(), end_hook);
    }

    fn enqueue_data(&mut self, data: Vec<u8>, end_hook: bool) {
        self.outbox.push_back(Packet {
            hook_id: self.hook_id,
            end_hook,
            path: self.reply_path.clone(),
            procedure_id: self.procedure_id,
            data,
        });
    }
}

/// Output accumulator passed to [`Procedure::handle`].
pub struct ProcedureOut {
    hook_id: HookID,
    reply_path: Vec<u32>,
    procedure_id: u32,
    outbox: PacketQueue,
}

impl ProcedureOut {
    /// Creates an empty procedure output queue.
    pub fn new(hook_id: HookID, reply_path: Vec<u32>, procedure_id: u32) -> Self {
        Self {
            hook_id,
            reply_path,
            procedure_id,
            outbox: PacketQueue::new(),
        }
    }

    /// Replaces the response path used by later [`Self::send`] calls.
    pub fn set_reply_path(&mut self, reply_path: Vec<u32>) {
        self.reply_path = reply_path;
    }

    /// Queues raw response data without closing the hook.
    pub fn send(&mut self, data: &[u8]) {
        self.send_with_end(data, false);
    }

    /// Queues raw response data that closes the hook after successful routing.
    pub fn send_final(&mut self, data: &[u8]) {
        self.send_with_end(data, true);
    }

    /// Consumes the output accumulator and returns packets for generated retry logic.
    pub fn into_packets(self) -> PacketQueue {
        self.outbox
    }

    fn send_with_end(&mut self, data: &[u8], end_hook: bool) {
        self.outbox.push_back(Packet {
            hook_id: self.hook_id,
            end_hook,
            path: self.reply_path.clone(),
            procedure_id: self.procedure_id,
            data: data.to_vec(),
        });
    }
}

/// Storage entry used by macro-generated session stores.
///
/// The fields are public so generated code in downstream crates can keep the update
/// loop straightforward and static. Handwritten leaves may also use this type, but it
/// is intentionally small rather than a full session framework.
pub struct SessionEntry<S> {
    /// Hook id associated with this live session.
    pub hook_id: HookID,

    /// Application-owned session state.
    pub state: S,

    /// Packets delivered for this hook but not yet consumed by the session.
    pub inbox: PacketQueue,

    /// Packets emitted by the session but not yet accepted by endpoint routing.
    pub outbox: PacketQueue,

    /// Whether application logic has finished and only retry flushing may remain.
    pub closed: bool,
}

impl<S> SessionEntry<S> {
    /// Creates one active session entry for `hook_id`.
    pub fn new(hook_id: HookID, state: S) -> Self {
        Self {
            hook_id,
            state,
            inbox: PacketQueue::new(),
            outbox: PacketQueue::new(),
            closed: false,
        }
    }
}

/// Flushes a retry queue through [`Endpoint::add_outbound`].
///
/// The packet at the front is cloned for each attempt and removed only after routing
/// succeeds. This preserves final frames when a route is temporarily unavailable.
/// The return value is true when the queue was fully drained.
pub fn flush_packet_queue(endpoint: &mut Endpoint, outbox: &mut PacketQueue) -> bool {
    while let Some(packet) = outbox.front().cloned() {
        if endpoint.add_outbound(packet).is_err() {
            return false;
        }

        outbox.pop_front();
    }

    true
}
