use alloc::vec::Vec;

use crate::protocol::{Endpoint, Packet, PacketQueue, SessionInitError, SessionStatus};

#[cfg(feature = "interface_ratatui")]
use crate::interface::InterfaceContext;

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

    /// Serializes this session for interface history storage.
    ///
    /// Returning `false` means the session does not expose historical state. Returning
    /// `true` means `out` contains a complete session-owned representation that
    /// [`Self::deserialize_interface_state`] can later rebuild. The generated leaf
    /// stores these bytes as opaque database blobs; no packet log or record schema is
    /// created around them.
    fn serialize_interface_state(&self, _: &mut Vec<u8>) -> bool {
        false
    }

    /// Rebuilds a session object from bytes previously written by this session type.
    ///
    /// This keeps active and historical UI unified: generated leaves can deserialize a
    /// stored session blob and pass it to the same Ratatui helper used for live state.
    fn deserialize_interface_state(_: &[u8]) -> Option<Self> {
        None
    }

    /// Renders one active or historical session inside its owning leaf UI.
    ///
    /// This is not a public lifecycle method. It is a typed helper that generated or
    /// handwritten leaves may call from their single leaf-level interface pass.
    #[cfg(feature = "interface_ratatui")]
    fn render_interface_ratatui(
        _: &L,
        _: &Self,
        _: &mut InterfaceContext<'_>,
        _: &mut ratatui::Frame<'_>,
        _: ratatui::layout::Rect,
    ) {
    }
}
