// ===========================================================================
// 7. The generic engine + the two concrete Nodes users actually name
// ===========================================================================

use std::marker::PhantomData;

use crate::api::{
    HookId, NodeId, ProcedureId,
    concurrency::Concurrency,
    node::{Node, NodeError},
    procedure::ProcedureContext,
    procedure_table::{ProcedureManifest, ProcedureTable},
    router::Router,
    session::{SessionData, SessionState, SessionStore},
};

pub struct NodeImpl<R, T, S, C, P, K>
where
    // What type of router is this using
    R: Router<HeaderType = P>,
    // How the procedures are stored
    T: ProcedureTable,
    // How the sessions are stored
    S: SessionStore<Key = K>,
    // What wraps the data to allow for multithreaded actions
    C: Concurrency,
    // P = type of packet header
    // K = type of session id (key)
{
    router: R,

    table: C::Cell<T>,
    sessions: C::Cell<S>,

    // The parameters must be used
    _marker_p: PhantomData<P>,
    _marker_k: PhantomData<K>,
}

impl<R: Router<HeaderType = P>, T: ProcedureTable, S: SessionStore<Key = K>, C: Concurrency, P, K>
    Node for NodeImpl<R, T, S, C, P, K>
{
    type RouterHandle = R;
    type SessionId = K;

    fn router(&self) -> &R {
        &self.router
    }

    fn open_session(&mut self, peer: NodeId, procedure: ProcedureId) -> K {
        // TODO: properly allocate hooks
        let hook = HookId(0);

        let data = SessionData {
            peer,
            procedure,
            hook,
            state: SessionState::None,
        };

        C::borrow_mut(&self.sessions).insert(data)
    }

    fn close_session(&mut self, id: &K) -> Option<SessionData> {
        C::borrow_mut(&self.sessions).remove(id)
    }

    fn dispatch(&mut self, session: &K, payload: &[u8]) -> Result<Vec<u8>, NodeError> {
        let mut store = C::borrow_mut(&self.sessions);

        let session_data = store.get_mut(session).ok_or(NodeError::UnknownSession)?;

        let table_guard = C::borrow(&self.table);
        let proc = table_guard
            .lookup(session_data.procedure)
            .ok_or(NodeError::UnknownProcedure(session_data.procedure))?;

        let mut ctx = NodeProcedureContext {
            session: session_data,
        };

        proc.call(&mut ctx, payload).map_err(NodeError::Procedure)
    }

    fn manifest(&self) -> ProcedureManifest {
        C::borrow(&self.table).manifest()
    }
}

/// Concrete context handed to procedures during `dispatch`.
///
/// Holds the live `&mut SessionData` (so typed state access goes straight
/// into the slab) and the session's id. No router reference: with `R`
/// unconstrained, `reply` is a stub.
struct NodeProcedureContext<'a> {
    session: &'a mut SessionData,
}

impl<'a> ProcedureContext<'a> for NodeProcedureContext<'a> {
    fn peer(&self) -> NodeId {
        self.session.peer
    }

    fn session_state<T: 'static>(&mut self) -> Option<&mut T> {
        match &mut self.session.state {
            SessionState::Boxed(b) => b.downcast_mut::<T>(),
            _ => None,
        }
    }

    fn set_session_state<T: 'static + Send>(&mut self, state: T) {
        self.session.state = SessionState::Boxed(Box::new(state));
    }

    fn reply(&mut self, _payload: &[u8]) -> Result<(), NodeError> {
        Err(NodeError::Router(
            "reply is not wired to a router outbound path yet",
        ))
    }
}
