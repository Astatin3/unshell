//! Explicit tree declaration, routing, and a small endpoint runtime.
//!
//! This module keeps the protocol tree machinery split by concern:
//! - `routing` contains static path declarations and longest-prefix routing helpers.
//! - `hook` contains the pending/active hook lifecycle tables used by endpoint runtime code.
//! - `endpoint` ties those pieces together into the runtime-facing protocol endpoint API.

mod endpoint;
mod hook;
mod leaf;
mod routing;

pub use endpoint::{
    ChildRoute, ConnectionState, Endpoint, EndpointError, EndpointOutcome, Ingress, LeafSpec,
    LocalEvent, ProtocolEndpoint,
};
pub use hook::{ActiveHook, HookConflict, HookKey, HookTable, PendingHook};
pub use leaf::{ProtocolLeaf, derive_leaf_name};
pub use routing::{
    CompiledRoutes, DefaultRouteProvider, LeafNode, RouteDecision, RouteProvider, TreeNode,
    is_prefix, route_destination,
};
