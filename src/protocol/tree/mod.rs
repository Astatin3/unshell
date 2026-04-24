//! Explicit tree declaration, routing, and a small endpoint runtime.

mod endpoint;
mod hook;
mod routing;

pub use endpoint::{
    ChildRoute, ConnectionState, Endpoint, EndpointError, EndpointOutcome, Ingress, LeafBehavior,
    LeafSpec, LocalEvent, ProtocolEndpoint,
};
pub use hook::{ActiveHook, HookKey, HookTable, PendingHook};
pub use routing::{
    DefaultRouteProvider, LeafNode, RouteDecision, RouteProvider, TreeNode, is_prefix,
    route_destination,
};
