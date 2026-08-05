// ===========================================================================
// 3. Procedures
// ===========================================================================

use crate::api::{NodeId, ProcedureId, node::NodeError};

#[derive(Debug)]
pub enum ProcedureError {
    NotFound(ProcedureId),
    BadInput,
    Rejected(&'static str),
    Internal(&'static str),
}

/// What a procedure author implements. Kept sync and `&[u8] -> Vec<u8>`
/// at this layer; if a procedure needs to await I/O it should hand off
/// to whatever the Node's async story is via `ctx.router()`, rather than
/// this trait itself being async -- that keeps `ErasedProcedure` object
/// safe and keeps the static table a plain function-pointer-shaped map.
pub trait Procedure: Send + Sync {
    /// Compile-time id, used by the static-table macro. Dynamic
    /// registration supplies an id explicitly instead (see
    /// `DynamicRegistry::register`), so a procedure can be reused in
    /// both worlds.
    const ID: ProcedureId;

    fn call(&self, ctx: &mut dyn ProcedureContext, input: &[u8])
    -> Result<Vec<u8>, ProcedureError>;
}

/// Object-safe counterpart to `Procedure`, without the associated
/// const. Both the static (phf map of `&'static dyn ErasedProcedure`)
/// and dynamic (`HashMap<_, Box<dyn ErasedProcedure>>`) tables store
/// this same trait object, so a `Procedure` impl works in either table
/// without being written twice.
pub trait ErasedProcedure: Send + Sync {
    fn id(&self) -> ProcedureId;

    fn call(&self, ctx: &mut dyn ProcedureContext, input: &[u8])
    -> Result<Vec<u8>, ProcedureError>;
}

/// Blanket impl so every `Procedure` is automatically an
/// `ErasedProcedure`.
impl<P: Procedure> ErasedProcedure for P {
    fn id(&self) -> ProcedureId {
        Self::ID
    }

    fn call(
        &self,
        ctx: &mut dyn ProcedureContext,
        input: &[u8],
    ) -> Result<Vec<u8>, ProcedureError> {
        Procedure::call(self, ctx, input)
    }
}

/// What a `Procedure::call` implementation is handed. A trait object
/// (not a concrete struct) so the same procedure code compiles once
/// and runs unmodified against `NodeSingleThreaded` or
/// `NodeMultiThreaded` -- the concurrency strategy is hidden behind
/// this interface rather than leaking into every procedure.
pub trait ProcedureContext<'a> {
    // type SessionId;
    // fn session_id(&self) -> Self::SessionId;

    fn peer(&self) -> NodeId;

    /// Typed access into `SessionState`. Returns `None` if no state is
    /// set or the stored type doesn't match `T` -- callers that need
    /// state should treat absence as "not initialized yet", not as an
    /// error.
    fn session_state<T: 'static>(&mut self) -> Option<&mut T>
    where
        Self: Sized;
    fn set_session_state<T: 'static + Send>(&mut self, state: T)
    where
        Self: Sized;

    /// Send a reply on this session without terminating it (streaming
    /// responses / progress updates).
    fn reply(&mut self, payload: &[u8]) -> Result<(), NodeError>;
}
