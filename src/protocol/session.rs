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
///         endpoint: &mut Endpoint,
///     ) -> SessionStatus {
///         while let Some(packet) = incoming.pop_front() {
///             session.apply(leaf, packet, endpoint);
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
    /// sessions can poll external workers even when no new packet arrived. Session
    /// output is routed immediately through `endpoint`; callers that need retry
    /// semantics should keep their own compact application state and retry on a later
    /// tick.
    fn update(
        leaf: &mut L,
        session: &mut Self,
        incoming: &mut PacketQueue,
        endpoint: &mut Endpoint,
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
    /// The generated leaf removes the entry after the update tick. Final packets are
    /// routed immediately by the session before returning this status.
    Closed,
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

    /// Whether application logic has finished and should be removed after update.
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
            count += entry.inbox.len();
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
    pub fn new(hook_id: HookID, state: S) -> Self {
        Self {
            hook_id,
            state,
            inbox: PacketQueue::new(),
            closed: false,
        }
    }
}
