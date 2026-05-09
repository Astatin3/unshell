//! Node lifecycle state.

/// Lifecycle state for a runtime node.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NodeState {
    /// The node has been constructed but has not started transport activity.
    #[default]
    Created,
    /// The node is accepting local work and transport events.
    Running,
    /// The node is draining work before shutdown.
    Stopping,
    /// The node has stopped and should not accept new work.
    Stopped,
}
