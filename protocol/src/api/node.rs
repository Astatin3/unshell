// ===========================================================================
// 5. The Node contract
// ===========================================================================

use crate::api::{
    NodeId, ProcedureId, procedure::ProcedureError, procedure_table::ProcedureManifest,
    session::SessionData,
};

#[derive(Debug)]
pub enum NodeError {
    UnknownSession,
    UnknownProcedure(ProcedureId),
    Procedure(ProcedureError),
    Router(&'static str),
}

/// The wrapper around Router. Threading-agnostic: this trait says
/// nothing about Rc vs Arc, RefCell vs RwLock -- that's decided by
/// whichever concrete type implements it (`NodeSingleThreaded`,
/// `NodeMultiThreaded`). Every method here is something both variants
/// must be able to do.
pub trait Node {
    /// Whatever Router's public handle type is -- kept as an assoc type
    /// so this trait doesn't hard-depend on Router's concrete type,
    /// which keeps this module testable against a fake Router.
    type RouterHandle;

    type SessionId;

    fn router(&self) -> &Self::RouterHandle;

    // -- session lifecycle -------------------------------------------------
    fn open_session(&mut self, peer: NodeId, procedure: ProcedureId) -> Self::SessionId;
    fn close_session(&mut self, id: &Self::SessionId) -> Option<SessionData>;

    // -- dispatch ------------------------------------------------------------
    /// Router hands Node a decoded envelope (session + payload); Node
    /// resolves the session, looks up the procedure, builds a Context,
    /// and invokes the handler. Returns the response bytes to send back
    /// via Router, or an error Router can turn into a protocol-level
    /// error reply.
    fn dispatch(&mut self, session: &Self::SessionId, payload: &[u8])
    -> Result<Vec<u8>, NodeError>;

    /// What this Node currently exposes -- used to answer a peer's
    /// handshake / capability query.
    fn manifest(&self) -> ProcedureManifest;
}
