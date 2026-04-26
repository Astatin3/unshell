//! Explicit tree declaration, routing, and a small endpoint runtime.
//!
//! This module keeps the protocol tree machinery split by concern:
//! - `routing` contains static path declarations and longest-prefix routing helpers.
//! - `hook` contains the pending/active hook lifecycle tables used by endpoint runtime code.
//! - `endpoint` ties those pieces together into the runtime-facing protocol endpoint API.
//! - `leaf` defines application-facing metadata and generated call-dispatch traits.
//! - `call` and `procedure` layer higher-level runtimes on top of validated endpoint events.

mod call;
mod endpoint;
mod hook;
mod leaf;
mod procedure;
mod routing;

pub use call::{
    Call, CallLeaf, CallReply, CallResult, DispatchError, IncomingCall, IncomingData,
    IncomingFault, LeafRuntime, LeafRuntimeError, OutgoingData, RuntimeOutcome, decode_call_input,
    encode_call_reply,
};
pub use endpoint::{
    ChildRoute, Endpoint, EndpointError, EndpointOutcome, Ingress, LeafSpec, LocalEvent,
    ProtocolEndpoint,
};
pub use hook::{ActiveHook, HookConflict, HookKey, HookTable, PendingHook};
pub use leaf::{CallProcedures, ProtocolLeaf, derive_leaf_name};
pub use procedure::{
    Procedure, ProcedureEffect, ProcedureRuntime, ProcedureRuntimeError, ProcedureRuntimeOutcome,
    ProcedureStore, StatefulProcedureMetadata,
};
pub use routing::{
    CompiledRoutes, DefaultRouteProvider, LeafNode, RouteDecision, RouteProvider, TreeNode,
    is_prefix, route_destination,
};
