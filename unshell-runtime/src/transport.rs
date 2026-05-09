//! Nonblocking transport contract for the single-threaded runtime.
//!
//! Transports move already-framed protocol packets. They do not know tree paths,
//! leaf names, hook state, admission policy, or route decisions.

use crate::connections::ConnectionId;
use unshell_protocol::FrameBytes;

/// Nonblocking frame transport used by [`crate::node::NodeRuntime`].
pub trait Transport {
    /// Transport-specific error.
    type Error;

    /// Polls for one inbound frame.
    ///
    /// `Ok(None)` means no frame is currently ready. Implementations must not
    /// block inside this method; callers drive progress by calling `tick` again.
    fn poll_recv(&mut self) -> Result<Option<(ConnectionId, FrameBytes)>, Self::Error>;

    /// Sends one framed packet on a registered connection.
    fn send_frame(
        &mut self,
        connection: ConnectionId,
        frame: &FrameBytes,
    ) -> Result<(), Self::Error>;

    /// Flushes buffered outbound transport data, if the transport has any.
    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
