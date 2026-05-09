//! # UnShell Runtime
//!
//! Single-threaded runtime scaffolding for hosting UnShell protocol nodes. This
//! crate currently bridges the existing protocol endpoint state while defining
//! the concrete transport, connection, and leaf-action APIs the redesign will use.

#![no_std]

pub extern crate alloc;

pub mod connections;
pub mod context;
pub mod effects;
pub mod leaf;
pub mod node;
pub mod transport;

pub use connections::{
    Connection, ConnectionDirection, ConnectionGeneration, ConnectionId, ConnectionState,
    ConnectionTable, Connections, RegisteredConnection,
};
pub use context::{
    ConnectionAction, LeafAction, LeafContext, OutboundCall, OutboundHookData, RequestDenied,
    RuntimeCapability,
};
pub use effects::{EffectQueue, RuntimeEffect};
pub use leaf::{Leaf, LeafCapabilities, LeafId, LeafPermissions, RegisteredLeaf};
pub use node::{
    EndpointState, LeafDispatchError, Node, NodeId, NodeRuntime, NodeRuntimeError, NodeState,
    TickBudget, TickOutcome,
};
pub use transport::Transport;

#[cfg(test)]
mod tests {
    use crate::alloc::string::String;
    use crate::alloc::vec;
    use crate::alloc::vec::Vec;

    use super::{
        Connection, ConnectionDirection, ConnectionGeneration, ConnectionId, ConnectionState,
        Connections, LeafAction, LeafCapabilities, LeafContext, LeafId, LeafPermissions,
        OutboundCall, OutboundHookData, RequestDenied, RuntimeCapability,
    };

    #[test]
    fn connection_generation_advances_without_wrapping() {
        assert_eq!(ConnectionGeneration::INITIAL.get(), 0);
        assert_eq!(ConnectionGeneration::new(41).next().get(), 42);
        assert_eq!(ConnectionGeneration::new(u64::MAX).next().get(), u64::MAX);
    }

    #[test]
    fn connection_table_reports_registered_connection_metadata() {
        let id = ConnectionId::new(7);
        let mut connections = Connections::new();
        connections.push(Connection::registered(
            id,
            ConnectionDirection::Child,
            vec![String::from("root"), String::from("child")],
            ConnectionGeneration::new(3),
        ));

        let registered = connections
            .registered(id)
            .expect("connection is registered");
        assert_eq!(registered.direction(), ConnectionDirection::Child);
        assert_eq!(registered.generation().get(), 3);
        assert_eq!(registered.peer_path(), ["root", "child"]);
    }

    #[test]
    fn connected_connections_are_not_routable() {
        let id = ConnectionId::new(9);
        let mut connections = Connections::new();
        connections.push(Connection::connected(id, ConnectionGeneration::INITIAL));

        assert!(connections.registered(id).is_none());
        assert!(matches!(
            connections.get(id).unwrap().state(),
            ConnectionState::Connected { .. }
        ));
    }

    #[test]
    fn leaf_context_queues_only_capability_checked_actions() {
        let id = LeafId::new(String::from("org.example.v1.echo"));
        let capabilities = LeafCapabilities {
            leaf_name: String::from("org.example.v1.echo"),
            procedures: vec![String::from("org.example.v1.echo.invoke")],
            permissions: LeafPermissions::REPLY_ONLY,
        };
        let connections = Connections::new();
        let local_path = vec![String::from("root")];
        let mut ctx = LeafContext::new(&local_path, &id, &capabilities, &connections);

        ctx.hook_data(OutboundHookData {
            dst_path: vec![String::from("root")],
            hook_id: 7,
            procedure_id: String::from("org.example.v1.echo.invoke"),
            payload: vec![1, 2, 3],
            end_hook: true,
        })
        .expect("reply-only leaf can send hook data");

        let denied = ctx.call(OutboundCall {
            dst_path: vec![String::from("root"), String::from("child")],
            dst_leaf: None,
            procedure_id: String::from("org.example.v1.echo.invoke"),
            payload: Vec::new(),
            expects_response: false,
        });

        assert_eq!(ctx.local_path(), ["root"]);
        assert!(matches!(ctx.actions()[0], LeafAction::SendHookData(_)));
        assert_eq!(
            denied,
            Err(RequestDenied::MissingCapability(
                RuntimeCapability::SendCalls
            ))
        );
    }
}
