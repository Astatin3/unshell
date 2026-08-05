// ===========================================================================
// 4. Procedure tables: static vs dynamic, same shape
// ===========================================================================

use crate::api::{
    ProcedureId,
    node::NodeError,
    procedure::{ErasedProcedure, Procedure},
};

/// What Node needs from a procedure table, regardless of how it's
/// populated. This is the seam that makes static/dynamic swappable
/// without touching dispatch logic.
pub trait ProcedureTable {
    fn lookup(&self, id: ProcedureId) -> Option<&dyn ErasedProcedure>;
    fn manifest(&self) -> ProcedureManifest;
}

/// Advertises what a Node exposes, e.g. during a handshake.
pub struct ProcedureManifest {
    pub ids: Vec<ProcedureId>,
}

/// Only implemented for tables that support runtime mutation, so a
/// `NodeStatic` never even exposes a `register` method -- attempting
/// runtime registration on a static-only Node is a compile error
/// (`DynamicRegistry` not implemented), not a runtime panic or an
/// ignored no-op.
pub trait DynamicRegistry {
    fn register<P: Procedure + 'static>(&mut self, proc: P) -> Result<(), NodeError>;
    fn register_boxed(
        &mut self,
        id: ProcedureId,
        proc: Box<dyn ErasedProcedure>,
    ) -> Result<(), NodeError>;
    fn unregister(&mut self, id: ProcedureId) -> Option<Box<dyn ErasedProcedure>>;
}
