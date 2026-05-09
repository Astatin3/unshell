//! Runtime effects produced by packet processing.

use crate::alloc::vec::Vec;
use crate::connections::{ConnectionGeneration, ConnectionId};
use unshell_protocol::FrameBytes;
use unshell_protocol::tree::LocalEvent;

/// Side effect selected by endpoint packet processing.
#[derive(Clone, Debug)]
pub enum RuntimeEffect {
    /// Send a frame to a registered connection.
    SendFrame {
        /// Destination connection id.
        connection: ConnectionId,
        /// Generation observed when the effect was queued.
        generation: ConnectionGeneration,
        /// Encoded protocol frame.
        frame: FrameBytes,
    },
    /// Deliver a local protocol event to the future leaf/session dispatcher.
    Local(LocalEvent),
    /// The frame was intentionally dropped by protocol state.
    Dropped,
}

/// FIFO queue of runtime effects.
#[derive(Clone, Debug, Default)]
pub struct EffectQueue {
    entries: Vec<RuntimeEffect>,
}

impl EffectQueue {
    /// Creates an empty effect queue.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Queues an effect.
    pub fn push(&mut self, effect: RuntimeEffect) {
        self.entries.push(effect);
    }

    /// Returns queued effects.
    #[must_use]
    pub fn entries(&self) -> &[RuntimeEffect] {
        &self.entries
    }

    /// Drains queued effects in FIFO order.
    pub fn drain(&mut self) -> impl Iterator<Item = RuntimeEffect> + '_ {
        self.entries.drain(..)
    }
}
