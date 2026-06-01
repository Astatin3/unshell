use crate::protocol::{Endpoint, EndpointError, EndpointName};

/// Compact identifier for one routed return channel.
///
/// Hook ids are local endpoint state, not globally unique session ids. A downward
/// packet with `end_hook = false` reserves the id at each endpoint it crosses so
/// later upward packets can prove that the route was paved by trusted downward
/// traffic first.
pub type HookID = u16;

impl Endpoint {
    /// Allocates a hook id that is not currently active on this endpoint.
    ///
    /// The first id is still deterministic (`0`) for the protocol tests, but the
    /// allocator now skips active hooks so long-lived streams cannot accidentally
    /// reuse an id before the previous route has closed. If every `u16` id is active
    /// the function panics; that is a hard local resource exhaustion condition, not a
    /// recoverable packet error.
    ///
    /// TODO: Reevaluate this method of allocation checking. It can be quite slow
    pub fn allocate_hook_id(&mut self) -> HookID {
        for _ in 0..=HookID::MAX {
            let candidate = self.last_hook.next();

            if !self.has_hook(candidate) {
                return candidate;
            }
        }

        // Avoid a panic message here: this crate is optimized for small binaries,
        // and exhausting every `u16` hook id is unrecoverable local state corruption.
        panic!();
    }

    /// Backwards-compatible name for [`Self::allocate_hook_id`].
    ///
    /// Existing leaves and tests still call `get_hook_id`; new code should prefer
    /// `allocate_hook_id` because it describes the reservation semantics more clearly.
    pub fn get_hook_id(&mut self) -> HookID {
        self.allocate_hook_id()
    }

    /// Explicitly records that `peer` may use `hook_id` as this endpoint's return channel.
    ///
    /// Routing calls this automatically for successful downward packets whose
    /// `end_hook` flag is false. The public method exists for trusted local setup and
    /// tests; ordinary leaf procedures should usually let packet routing pave hooks
    /// instead of mutating hook state by hand.
    pub fn accept_hook(&mut self, hook_id: HookID, peer: u32) -> Option<u32> {
        self.hook_insert(hook_id, peer)
    }

    /// Returns true when `hook_id` is currently active.
    pub fn has_hook(&self, hook_id: HookID) -> bool {
        self.hooks
            .iter()
            .any(|(existing_hook, _)| *existing_hook == hook_id)
    }

    /// Returns the adjacent peer currently associated with `hook_id`.
    ///
    /// The peer is the next endpoint expected to participate in the return channel:
    /// a child for downward calls that will reply upward, or a parent for a local
    /// callee that will emit an upward response.
    pub fn hook_peer(&self, hook_id: HookID) -> Option<u32> {
        self.hooks
            .iter()
            .find(|(existing_hook, _)| *existing_hook == hook_id)
            .map(|(_, peer)| *peer)
    }

    /// Returns the number of active hooks on this endpoint.
    pub fn hook_count(&self) -> usize {
        self.hooks.len()
    }

    /// Locally forgets a hook without sending protocol traffic.
    ///
    /// Graceful shutdown should use a packet with `end_hook = true` so every endpoint
    /// along the route cleans up after successful delivery. This method is for local
    /// emergency cleanup such as a crashed PTY process, a timed-out stream, or a lost
    /// transport where no final packet can be delivered.
    pub fn forget_hook(&mut self, hook_id: HookID) -> bool {
        self.close_hook(hook_id)
    }

    /// Validates that `actual_peer` is the peer allowed to use `hook_id`.
    pub(crate) fn ensure_hook_peer(
        &self,
        hook_id: HookID,
        actual_peer: EndpointName,
    ) -> Result<(), EndpointError> {
        let expected_peer = self
            .hook_peer(hook_id)
            .ok_or(EndpointError::UnknownHook { hook_id })?;

        if expected_peer == actual_peer {
            Ok(())
        } else {
            Err(EndpointError::HookPeerMismatch {
                hook_id,
                expected_peer,
                actual_peer,
            })
        }
    }

    /// Opens or refreshes `hook_id` for the adjacent `peer` after downward routing succeeds.
    pub(crate) fn open_hook(&mut self, hook_id: HookID, peer: EndpointName) {
        self.hook_insert(hook_id, peer);
    }

    /// Removes `hook_id` and reports whether it existed.
    pub(crate) fn close_hook(&mut self, hook_id: HookID) -> bool {
        self.hook_remove(hook_id).is_some()
    }

    /// Inserts or updates a hook and returns the previously associated peer.
    pub(crate) fn hook_insert(
        &mut self,
        hook_id: HookID,
        peer: EndpointName,
    ) -> Option<EndpointName> {
        if let Some((_, existing_peer)) = self
            .hooks
            .iter_mut()
            .find(|(existing_hook, _)| *existing_hook == hook_id)
        {
            let previous = *existing_peer;
            *existing_peer = peer;
            Some(previous)
        } else {
            self.hooks.push((hook_id, peer));
            None
        }
    }

    /// Removes a hook and returns the peer it pointed at.
    pub(crate) fn hook_remove(&mut self, hook_id: HookID) -> Option<EndpointName> {
        let index = self
            .hooks
            .iter()
            .position(|(existing_hook, _)| *existing_hook == hook_id)?;

        Some(self.hooks.remove(index).1)
    }
}
