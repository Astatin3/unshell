use alloc::vec::Vec;

use crate::protocol::{Endpoint, HookID, Packet, PacketQueue};

#[cfg(feature = "interface_ratatui")]
use crate::interface::SessionView;

/// Contract implemented by one hook-backed generated session family.
///
/// A session family maps one outer `procedure_id` to many live hook instances. The
/// generated leaf owns packet grouping, retry-safe output flushing, and final cleanup;
/// the session value owns one hook's application behavior and mutable state.
///
/// # Example
///
/// ```rust,ignore
/// impl Session<MyLeafState> for MySessionState {
///     const PROCEDURE_ID: u32 = 7;
///
///     fn init(
///         leaf: &mut MyLeafState,
///         packet: Packet,
///     ) -> Result<Self, SessionInitError> {
///         Ok(MySessionState::from_open(leaf, packet))
///     }
///
///     fn update(
///         leaf: &mut MyLeafState,
///         session: &mut Self,
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
pub trait Session<L>: Sized {
    /// Outer packet procedure id used by every packet in this session family.
    const PROCEDURE_ID: u32;

    /// Creates one session value from a packet whose hook has no active session.
    ///
    /// The generated runtime derives all response routing from hook state. Session
    /// initialization therefore returns only application state or a protocol-level
    /// rejection; it never stores or receives a caller reply path.
    fn init(leaf: &mut L, packet: Packet) -> Result<Self, SessionInitError>;

    /// Advances one active hook session.
    ///
    /// The generated leaf calls this for every live session on each update tick so
    /// sessions can poll external workers even when no new packet arrived. Outbound
    /// packets must be queued through `ctx`; direct endpoint routing would bypass the
    /// generated retry rules.
    fn update(
        leaf: &mut L,
        session: &mut Self,
        incoming: &mut PacketQueue,
        ctx: &mut SessionCtx<'_>,
    ) -> SessionStatus;

    #[cfg(feature = "interface_ratatui")]
    fn render_ratatui(
        _: &L,
        _: &Self,
        _: &mut SessionView,
        _: &mut ratatui::Frame<'_>,
        _: ratatui::layout::Rect,
    ) {
    }
}

/// Error returned when a packet cannot create a new session.
pub enum SessionInitError {
    /// The packet was intentionally consumed without creating state or sending output.
    Rejected,

    /// The packet was rejected with response data that should be sent on the same hook.
    Response {
        /// Raw `Packet::data` for the response frame.
        data: Vec<u8>,

        /// Whether the response should close the hook after successful routing.
        end_hook: bool,
    },
}

impl SessionInitError {
    /// Creates a silent session rejection.
    pub fn rejected() -> Self {
        Self::Rejected
    }

    /// Creates a non-final response for a rejected session open.
    pub fn response(data: Vec<u8>) -> Self {
        Self::Response {
            data,
            end_hook: false,
        }
    }

    /// Creates a final response for a rejected session open.
    pub fn response_final(data: Vec<u8>) -> Self {
        Self::Response {
            data,
            end_hook: true,
        }
    }
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
    path: Vec<u32>,
    procedure_id: u32,
    outbox: &'a mut PacketQueue,
}

impl<'a> SessionCtx<'a> {
    /// Creates a context for one session update call.
    pub fn new(
        hook_id: HookID,
        path: Vec<u32>,
        procedure_id: u32,
        outbox: &'a mut PacketQueue,
    ) -> Self {
        Self {
            hook_id,
            path,
            procedure_id,
            outbox,
        }
    }

    /// Returns the hook id used for packets emitted through this context.
    pub fn hook_id(&self) -> HookID {
        self.hook_id
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
            path: self.path.clone(),
            procedure_id: self.procedure_id,
            data,
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

    /// Destination path for packets emitted on this hook.
    ///
    /// This is generated runtime state, not user session state. It is captured from
    /// endpoint hook routing when the session is created so leaf sessions never have
    /// to carry or understand a reply path.
    pub path: Vec<u32>,

    /// Application-owned session state.
    pub state: S,

    /// Packets delivered for this hook but not yet consumed by the session.
    pub inbox: PacketQueue,

    /// Packets emitted by the session but not yet accepted by endpoint routing.
    pub outbox: PacketQueue,

    /// Whether application logic has finished and only retry flushing may remain.
    pub closed: bool,
}

/// Generated storage for one session family.
///
/// The macro only names this field and picks the concrete `Session` type. All update,
/// retry, and cleanup behavior lives in normal Rust helpers so the template stays
/// small and readable.
pub struct SessionFamily<S> {
    /// Active hook-backed sessions for this family.
    pub entries: Vec<SessionEntry<S>>,
}

impl<S> SessionFamily<S> {
    /// Creates an empty session family.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Counts packets retained by this family for retry or future session work.
    pub fn pending_packet_count(&self) -> usize {
        let mut count = 0usize;

        for entry in &self.entries {
            count += entry.inbox.len() + entry.outbox.len();
        }

        count
    }
}

impl<S> Default for SessionFamily<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> SessionEntry<S> {
    /// Creates one active session entry for `hook_id`.
    pub fn new(hook_id: HookID, path: Vec<u32>, state: S) -> Self {
        Self {
            hook_id,
            path,
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
