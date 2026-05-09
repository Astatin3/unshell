//! Hook state for pending and active protocol flows.
//!
//! Hooks move through two phases:
//! - `PendingHook` tracks enough context to attribute faults before the callee accepts.
//! - `ActiveHook` tracks the live bidirectional flow after activation.
//!
//! The table indexes active hooks both by their host-side return path and by the remote
//! peer path so routing code can resolve whichever side of the relationship it currently has.
//! The `HookKey` already carries the host path and hook id, so the pending/active records only
//! store the extra state that actually changes across the hook lifecycle.

use alloc::{collections::BTreeMap, string::String, vec::Vec};

/// Hook table key scoped to the hook host path.
///
/// This exists because hook ids are only unique relative to the endpoint path that hosts the
/// hook state.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::HookKey;
/// let key = HookKey::new(vec!["root".into()], 7);
/// assert_eq!(key.hook_id, 7);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct HookKey {
    /// Path of the endpoint hosting the hook state.
    pub return_path: Vec<String>,
    /// Per-host hook identifier.
    pub hook_id: u64,
}

impl HookKey {
    /// Builds the canonical key for a hook hosted at `return_path`.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::HookKey;
    /// let key = HookKey::new(vec!["root".into()], 42);
    /// assert_eq!(key.return_path, vec![String::from("root")]);
    /// ```
    #[must_use]
    pub fn new(return_path: Vec<String>, hook_id: u64) -> Self {
        Self {
            return_path,
            hook_id,
        }
    }
}

/// Pending hook context used only for fault attribution before activation.
///
/// This exists so outbound calls can reserve response-hook ownership before the callee has sent
/// its first valid `Data` packet.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::PendingHook;
/// let pending = PendingHook {
///     caller_src_path: vec!["worker".into()],
///     procedure_id: "example.service.v1.invoke".into(),
///     local_ended: false,
/// };
/// assert!(!pending.local_ended);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingHook {
    /// Caller path to promote into `peer_path` once the hook becomes active.
    pub caller_src_path: Vec<String>,
    /// Procedure that created the hook.
    pub procedure_id: String,
    /// Set once the local side has already emitted its terminal message before activation.
    pub local_ended: bool,
}

/// Active hook context used for ordinary data traffic.
///
/// This exists once one peer has proven ownership of the hook stream and ordinary `Data`/`Fault`
/// routing can proceed without the pending reservation state.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::ActiveHook;
/// let active = ActiveHook {
///     peer_path: vec!["worker".into()],
///     procedure_id: "example.service.v1.invoke".into(),
///     local_ended: false,
///     peer_ended: false,
/// };
/// assert_eq!(active.peer_path[0], "worker");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveHook {
    /// Remote endpoint path currently paired with this hook.
    pub peer_path: Vec<String>,
    /// Procedure that owns the hook conversation.
    pub procedure_id: String,
    /// Set once the local side has emitted its terminal message.
    pub local_ended: bool,
    /// Set once the peer side has emitted its terminal message.
    pub peer_ended: bool,
}

/// Duplicate hook insertion error.
///
/// This exists so callers can distinguish “hook id already reserved” from other runtime errors.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::HookConflict;
/// let _conflict = HookConflict;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookConflict;

/// Durable hook state tables.
///
/// This owns both pending and active hook lifecycle state plus a peer-path index for resolving
/// inbound hook traffic from either side of the conversation.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::{HookKey, HookTable, PendingHook};
/// let mut hooks = HookTable::default();
/// let key = HookKey::new(vec!["root".into()], 1);
/// hooks.insert_pending(key.clone(), PendingHook {
///     caller_src_path: vec!["worker".into()],
///     procedure_id: "example.service.v1.invoke".into(),
///     local_ended: false,
/// }).unwrap();
/// assert_eq!(hooks.pending_len(), 1);
/// ```
#[derive(Debug, Clone, Default)]
pub struct HookTable {
    pending: BTreeMap<HookKey, PendingHook>,
    active: BTreeMap<HookKey, ActiveHook>,
    active_by_peer: BTreeMap<u64, BTreeMap<Vec<String>, HookKey>>,
    next_id: u64,
}

impl HookTable {
    /// Allocates a non-zero hook id for a hook hosted at `return_path`.
    ///
    /// Hook ids are scoped by host path, so this only needs to guarantee uniqueness within the
    /// local table. The wrapped increment keeps allocation infallible for long-lived runtimes.
    ///
    /// The table currently uses one counter shared across all host paths. The `return_path`
    /// parameter remains in the API because hook ids are still interpreted as host-scoped by the
    /// rest of the protocol surface.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::HookTable;
    /// let mut hooks = HookTable::default();
    /// let id = hooks.allocate_hook_id(&[String::from("root")]);
    /// assert_ne!(id, 0);
    /// ```
    #[must_use]
    pub fn allocate_hook_id(&mut self, _return_path: &[String]) -> u64 {
        let id = self.next_id.max(1);
        self.next_id = id.wrapping_add(1);
        id
    }

    /// Inserts a hook that has been announced but not yet accepted by the callee.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{HookKey, HookTable, PendingHook};
    /// let mut hooks = HookTable::default();
    /// hooks.insert_pending(HookKey::new(vec!["root".into()], 1), PendingHook {
    ///     caller_src_path: vec!["worker".into()],
    ///     procedure_id: "example.service.v1.invoke".into(),
    ///     local_ended: false,
    /// })?;
    /// # Ok::<(), unshell::protocol::tree::HookConflict>(())
    /// ```
    pub fn insert_pending(
        &mut self,
        key: HookKey,
        pending: PendingHook,
    ) -> Result<(), HookConflict> {
        if self.pending.contains_key(&key) || self.active.contains_key(&key) {
            return Err(HookConflict);
        }
        self.pending.insert(key, pending);
        Ok(())
    }

    /// Promotes a pending hook into the active table.
    ///
    /// Activation intentionally reuses the original hook id and host path, but swaps the
    /// pending caller attribution into the active peer path used for data routing.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{HookKey, HookTable, PendingHook};
    /// let mut hooks = HookTable::default();
    /// let key = HookKey::new(vec!["root".into()], 1);
    /// hooks.insert_pending(key.clone(), PendingHook {
    ///     caller_src_path: vec!["worker".into()],
    ///     procedure_id: "example.service.v1.invoke".into(),
    ///     local_ended: false,
    /// })?;
    /// hooks.activate_pending(&key);
    /// assert_eq!(hooks.active_len(), 1);
    /// # Ok::<(), unshell::protocol::tree::HookConflict>(())
    /// ```
    pub fn activate_pending(&mut self, key: &HookKey) -> Option<()> {
        let pending = self.pending.remove(key)?;
        self.insert_active(
            key.clone(),
            ActiveHook {
                peer_path: pending.caller_src_path,
                procedure_id: pending.procedure_id,
                local_ended: pending.local_ended,
                peer_ended: false,
            },
        )
        .ok()?;
        Some(())
    }

    /// Inserts a live hook and its peer-path lookup entry.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{ActiveHook, HookKey, HookTable};
    /// let mut hooks = HookTable::default();
    /// hooks.insert_active(HookKey::new(vec!["root".into()], 1), ActiveHook {
    ///     peer_path: vec!["worker".into()],
    ///     procedure_id: "example.service.v1.invoke".into(),
    ///     local_ended: false,
    ///     peer_ended: false,
    /// })?;
    /// assert_eq!(hooks.active_len(), 1);
    /// # Ok::<(), unshell::protocol::tree::HookConflict>(())
    /// ```
    pub fn insert_active(&mut self, key: HookKey, active: ActiveHook) -> Result<(), HookConflict> {
        // Reject both duplicate host-scoped keys and duplicate peer ownership claims. Either one
        // would make later inbound hook traffic ambiguous.
        if self.pending.contains_key(&key)
            || self.active.contains_key(&key)
            || self
                .active_by_peer
                .get(&key.hook_id)
                .is_some_and(|peer_paths| peer_paths.contains_key(active.peer_path.as_slice()))
        {
            return Err(HookConflict);
        }
        self.active_by_peer
            .entry(key.hook_id)
            .or_default()
            .insert(active.peer_path.clone(), key.clone());
        self.active.insert(key, active);
        Ok(())
    }

    /// Removes a pending hook without affecting active state.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{HookKey, HookTable, PendingHook};
    /// let mut hooks = HookTable::default();
    /// let key = HookKey::new(vec!["root".into()], 1);
    /// hooks.insert_pending(key.clone(), PendingHook {
    ///     caller_src_path: vec!["worker".into()],
    ///     procedure_id: "example.service.v1.invoke".into(),
    ///     local_ended: false,
    /// })?;
    /// assert!(hooks.remove_pending(&key).is_some());
    /// # Ok::<(), unshell::protocol::tree::HookConflict>(())
    /// ```
    pub fn remove_pending(&mut self, key: &HookKey) -> Option<PendingHook> {
        self.pending.remove(key)
    }

    /// Marks the local side finished before the hook becomes active.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{HookKey, HookTable, PendingHook};
    /// let mut hooks = HookTable::default();
    /// let key = HookKey::new(vec!["root".into()], 1);
    /// hooks.insert_pending(key.clone(), PendingHook {
    ///     caller_src_path: vec!["worker".into()],
    ///     procedure_id: "example.service.v1.invoke".into(),
    ///     local_ended: false,
    /// })?;
    /// hooks.mark_pending_local_end(&key);
    /// assert!(hooks.pending(&key).unwrap().local_ended);
    /// # Ok::<(), unshell::protocol::tree::HookConflict>(())
    /// ```
    pub fn mark_pending_local_end(&mut self, key: &HookKey) {
        if let Some(pending) = self.pending.get_mut(key) {
            pending.local_ended = true;
        }
    }

    /// Removes an active hook and its secondary peer-path index entry.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{ActiveHook, HookKey, HookTable};
    /// let mut hooks = HookTable::default();
    /// let key = HookKey::new(vec!["root".into()], 1);
    /// hooks.insert_active(key.clone(), ActiveHook {
    ///     peer_path: vec!["worker".into()],
    ///     procedure_id: "example.service.v1.invoke".into(),
    ///     local_ended: false,
    ///     peer_ended: false,
    /// })?;
    /// assert!(hooks.remove_active(&key).is_some());
    /// # Ok::<(), unshell::protocol::tree::HookConflict>(())
    /// ```
    pub fn remove_active(&mut self, key: &HookKey) -> Option<ActiveHook> {
        let active = self.active.remove(key)?;
        if let Some(peer_paths) = self.active_by_peer.get_mut(&key.hook_id) {
            peer_paths.remove(active.peer_path.as_slice());
            if peer_paths.is_empty() {
                self.active_by_peer.remove(&key.hook_id);
            }
        }
        Some(active)
    }

    /// Returns the pending hook for `key`, if present.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{HookKey, HookTable, PendingHook};
    /// let mut hooks = HookTable::default();
    /// let key = HookKey::new(vec!["root".into()], 1);
    /// hooks.insert_pending(key.clone(), PendingHook {
    ///     caller_src_path: vec!["worker".into()],
    ///     procedure_id: "example.service.v1.invoke".into(),
    ///     local_ended: false,
    /// })?;
    /// assert!(hooks.pending(&key).is_some());
    /// # Ok::<(), unshell::protocol::tree::HookConflict>(())
    /// ```
    #[must_use]
    pub fn pending(&self, key: &HookKey) -> Option<&PendingHook> {
        self.pending.get(key)
    }

    /// Returns the active hook for `key`, if present.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{ActiveHook, HookKey, HookTable};
    /// let mut hooks = HookTable::default();
    /// let key = HookKey::new(vec!["root".into()], 1);
    /// hooks.insert_active(key.clone(), ActiveHook {
    ///     peer_path: vec!["worker".into()],
    ///     procedure_id: "example.service.v1.invoke".into(),
    ///     local_ended: false,
    ///     peer_ended: false,
    /// })?;
    /// assert!(hooks.active(&key).is_some());
    /// # Ok::<(), unshell::protocol::tree::HookConflict>(())
    /// ```
    #[must_use]
    pub fn active(&self, key: &HookKey) -> Option<&ActiveHook> {
        self.active.get(key)
    }

    /// Resolves an active hook from either side of the conversation.
    ///
    /// The host side addresses hooks directly by `(return_path, hook_id)`. Peer-originated
    /// traffic only has `(hook_id, peer_path)`, so the secondary index maps that back to the
    /// canonical host-scoped key.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{ActiveHook, HookKey, HookTable};
    /// let mut hooks = HookTable::default();
    /// let key = HookKey::new(vec!["root".into()], 1);
    /// hooks.insert_active(key.clone(), ActiveHook {
    ///     peer_path: vec!["worker".into()],
    ///     procedure_id: "example.service.v1.invoke".into(),
    ///     local_ended: false,
    ///     peer_ended: false,
    /// })?;
    /// assert_eq!(hooks.resolve_active_key(&["root".into()], 1, &["worker".into()]), Some(key));
    /// # Ok::<(), unshell::protocol::tree::HookConflict>(())
    /// ```
    #[must_use]
    pub fn resolve_active_key(
        &self,
        return_path: &[String],
        hook_id: u64,
        peer_path: &[String],
    ) -> Option<HookKey> {
        // Prefer peer-originated resolution first because inbound hook traffic normally arrives
        // from the far side with only `(hook_id, peer_path)` available.
        if let Some(key) = self
            .active_by_peer
            .get(&hook_id)
            .and_then(|peer_paths| peer_paths.get(peer_path))
        {
            return Some(key.clone());
        }

        let host_key = HookKey::new(return_path.to_vec(), hook_id);
        self.active.contains_key(&host_key).then_some(host_key)
    }

    /// Marks the local side finished and returns `true` once both sides are finished.
    ///
    /// This does not remove the hook. Callers use the boolean to decide whether cleanup should
    /// happen immediately or whether the peer side is still expected to send more traffic.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{ActiveHook, HookKey, HookTable};
    /// let mut hooks = HookTable::default();
    /// let key = HookKey::new(vec!["root".into()], 1);
    /// hooks.insert_active(key.clone(), ActiveHook {
    ///     peer_path: vec!["worker".into()],
    ///     procedure_id: "example.service.v1.invoke".into(),
    ///     local_ended: false,
    ///     peer_ended: true,
    /// })?;
    /// assert!(hooks.mark_local_end(&key));
    /// # Ok::<(), unshell::protocol::tree::HookConflict>(())
    /// ```
    pub fn mark_local_end(&mut self, key: &HookKey) -> bool {
        let Some(active) = self.active.get_mut(key) else {
            return false;
        };
        active.local_ended = true;
        active.peer_ended
    }

    /// Marks the peer side finished and returns `true` once both sides are finished.
    ///
    /// This mirrors [`mark_local_end`](Self::mark_local_end): it only reports completion, leaving
    /// final removal to the caller so higher layers can decide when to tear down hook state.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{ActiveHook, HookKey, HookTable};
    /// let mut hooks = HookTable::default();
    /// let key = HookKey::new(vec!["root".into()], 1);
    /// hooks.insert_active(key.clone(), ActiveHook {
    ///     peer_path: vec!["worker".into()],
    ///     procedure_id: "example.service.v1.invoke".into(),
    ///     local_ended: true,
    ///     peer_ended: false,
    /// })?;
    /// assert!(hooks.mark_peer_end(&key));
    /// # Ok::<(), unshell::protocol::tree::HookConflict>(())
    /// ```
    pub fn mark_peer_end(&mut self, key: &HookKey) -> bool {
        let Some(active) = self.active.get_mut(key) else {
            return false;
        };
        active.peer_ended = true;
        active.local_ended
    }

    /// Returns the number of active hooks.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::HookTable;
    /// let hooks = HookTable::default();
    /// assert_eq!(hooks.active_len(), 0);
    /// ```
    #[must_use]
    pub fn active_len(&self) -> usize {
        self.active.len()
    }

    /// Returns the number of pending hooks.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::HookTable;
    /// let hooks = HookTable::default();
    /// assert_eq!(hooks.pending_len(), 0);
    /// ```
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }
}
