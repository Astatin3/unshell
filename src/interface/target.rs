use crate::{
    interface::{ProcedureKey, SessionKey},
    protocol::HookID,
};

/// Internal owner for one interface event.
///
/// The runtime already knows whether a packet belongs to a hook-backed session or a
/// one-shot procedure. Keeping that answer explicit avoids reconstructing ownership
/// from packet fields later, which is what made procedure packet flow look like fake
/// session activity in the previous store implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InterfaceTarget {
    /// Event belongs to one hook-backed session instance.
    Session(SessionKey),

    /// Event belongs to one one-shot procedure family.
    Procedure(ProcedureKey),
}

impl InterfaceTarget {
    /// Builds a session target from the same pieces exposed by [`SessionKey`].
    pub(crate) fn session(leaf_id: u32, procedure_id: u32, hook_id: HookID) -> Self {
        Self::Session(SessionKey {
            leaf_id,
            procedure_id,
            hook_id,
        })
    }

    /// Builds a procedure target from the same pieces exposed by [`ProcedureKey`].
    pub(crate) fn procedure(leaf_id: u32, procedure_id: u32) -> Self {
        Self::Procedure(ProcedureKey {
            leaf_id,
            procedure_id,
        })
    }

    /// Returns the leaf id used on the append-only event record.
    pub(crate) fn leaf_id(self) -> u32 {
        match self {
            Self::Session(key) => key.leaf_id,
            Self::Procedure(key) => key.leaf_id,
        }
    }
}
