use crate::protocol::HookID;

/// Stable identity for one generated session view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SessionKey {
    /// Leaf id that owns the generated session family.
    pub leaf_id: u32,

    /// Procedure id shared by every packet in the session family.
    pub procedure_id: u32,

    /// Hook id for the live session instance.
    pub hook_id: HookID,
}

/// Stable identity for one generated procedure view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcedureKey {
    /// Leaf id that owns the generated procedure family.
    pub leaf_id: u32,

    /// Procedure id handled by this one-shot procedure family.
    pub procedure_id: u32,
}
