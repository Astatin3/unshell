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

    /// Drains local-dispatch effects in FIFO order, leaving outbound sends queued.
    pub fn drain_local(&mut self) -> impl Iterator<Item = RuntimeEffect> {
        let mut drained = Vec::new();
        let mut retained = Vec::with_capacity(self.entries.len());

        for effect in self.entries.drain(..) {
            match effect {
                RuntimeEffect::Local(_) | RuntimeEffect::Dropped => drained.push(effect),
                RuntimeEffect::SendFrame { .. } => retained.push(effect),
            }
        }

        self.entries = retained;
        drained.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drain_local_leaves_outbound_sends_queued() {
        let first = ConnectionId::new(1);
        let second = ConnectionId::new(2);
        let mut queue = EffectQueue::new();

        queue.push(RuntimeEffect::SendFrame {
            connection: first,
            generation: ConnectionGeneration::INITIAL,
            frame: FrameBytes::new(),
        });
        queue.push(RuntimeEffect::Dropped);
        queue.push(RuntimeEffect::SendFrame {
            connection: second,
            generation: ConnectionGeneration::INITIAL,
            frame: FrameBytes::new(),
        });
        queue.push(RuntimeEffect::Dropped);

        let drained: Vec<_> = queue.drain_local().collect();

        assert_eq!(drained.len(), 2);
        assert!(
            drained
                .iter()
                .all(|effect| matches!(effect, RuntimeEffect::Dropped))
        );
        assert_eq!(queue.entries().len(), 2);
        assert!(matches!(
            queue.entries()[0],
            RuntimeEffect::SendFrame { connection, .. } if connection == first
        ));
        assert!(matches!(
            queue.entries()[1],
            RuntimeEffect::SendFrame { connection, .. } if connection == second
        ));
    }
}
