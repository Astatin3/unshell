pub mod tree {
    //! The 'tree' protocol is a transport layer protocol
    //! designed to be stacked on others, and to be built
    //! on with emergent complexity and minimal overhead.
    //!
    //! Here are the core principles:
    //! 1) The root node is the most trusted
    //! 2) Packets can never be spontaneously sent from a lower level
    //! 3) 'Streams' are freeform, they exist if a downwards packet
    //!    doesn't have the 'close' bit set.
    //! 4) There's very little intermediary structure required,
    //!    error responses aren't handled, the handshake process
    //!    isn't defined. Nodes are expected to handle their
    //!    own security.

    mod packet;
    mod router;

    pub use packet::TreePacketHeader;
    pub use router::TreeRouter;
}

pub mod api {
    pub mod concurrency;
    pub mod node;
    pub mod procedure;
    pub mod procedure_table;
    pub mod router;
    pub mod session;

    /// Node IDs are the identifiers of each node
    /// They must be unique among the direct children
    /// of any given node, but are not required to be unique
    /// in any other circumstance.
    ///
    /// Since the path is defined explicitly, this isn't a problem.
    #[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, rkyv::Archive)]
    pub struct NodeId(pub u32);

    /// The Hook ID is the unique identifier of any given
    /// bidirectional channel. It targets a specific procedure
    /// on a child.
    ///
    /// It must be unique among a pair of node ids,
    /// and a procedure ID on the child
    ///
    /// This can be used both as a bidirectional stream and as
    /// a producer/subscriber channel, or anything else that
    /// can be defined in this format.
    #[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, rkyv::Archive)]
    pub struct HookId(pub u32);

    /// A procedure is an identifier to any given component
    /// on a node. It can be a static remote proc method,
    /// another protocol stacked on top of the tree protocol,
    /// whatever.
    #[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, rkyv::Archive)]
    pub struct ProcedureId(pub u32);
}

pub mod api_impl {
    mod nodeimpl;
    mod procedure_table_dynamic;
    mod procedure_table_static;
    mod session_store_slab;

    pub use nodeimpl::NodeImpl;
    pub use procedure_table_dynamic::DynamicProcedureTable;
    pub use procedure_table_static::StaticProcedureTable;
    pub use session_store_slab::SlabSessionStore;
}
