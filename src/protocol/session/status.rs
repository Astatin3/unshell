/// Session lifecycle status returned from [`Session::update`](super::Session::update).
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
