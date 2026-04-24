//! Smaller onboarding scenarios.
//!
//! These scenarios are intentionally compact. Each one isolates one major part
//! of the protocol so users can learn the tree, hook, and fault mechanics before
//! switching to the larger sandbox topology.

use crate::model::{
    EndpointProcedureKind, EndpointProcedureSpec, LeafKind, LeafSpec, NodeId, NodeSpec,
    ScenarioDefinition, Selection,
};

/// Single-response endpoint procedure used in small scenarios.
pub(super) const PROC_PING: &str = "demo.endpoint.v1.control.ping";
/// Multi-packet endpoint procedure used to visualize chunked responses.
pub(super) const PROC_CHUNKED: &str = "demo.endpoint.v1.stream.chunked_greeting";
/// Long-lived endpoint procedure used for bidirectional hook traffic.
pub(super) const PROC_CHAT: &str = "demo.endpoint.v1.chat.session";
/// Leaf echo contract used throughout the demos.
pub(super) const PROC_ECHO: &str = "demo.leaf.v1.echo.invoke";

/// Returns the onboarding scenarios in the order they should be explored.
pub(super) fn scenarios() -> Vec<ScenarioDefinition> {
    vec![
        local_introspection(),
        echo_leaf(),
        branch_routing(),
        bidirectional_chat(),
        fault_showcase(),
    ]
}

/// Minimal introspection walkthrough.
fn local_introspection() -> ScenarioDefinition {
    ScenarioDefinition {
        name: "Local Introspection".to_owned(),
        description:
            "Inspect the root and its immediate child using the required empty procedure id."
                .to_owned(),
        highlights: vec![
            "Blank procedure calls map to protocol introspection.".to_owned(),
            "Leaf introspection uses the same hook path as endpoint introspection.".to_owned(),
        ],
        root: NodeSpec {
            segment: String::new(),
            title: "Root".to_owned(),
            description: "The operator-controlled root endpoint.".to_owned(),
            leaves: Vec::new(),
            endpoint_procedures: vec![EndpointProcedureSpec {
                procedure_id: PROC_PING.to_owned(),
                description: "Single-packet endpoint response for baseline testing.".to_owned(),
                kind: EndpointProcedureKind::Ping,
            }],
            children: vec![NodeSpec {
                segment: "alpha".to_owned(),
                title: "Alpha".to_owned(),
                description: "A minimal child endpoint with one echo leaf.".to_owned(),
                leaves: vec![LeafSpec {
                    name: "echo".to_owned(),
                    description: "Echoes bytes back through the declared hook.".to_owned(),
                    kind: LeafKind::Echo,
                    procedures: vec![PROC_ECHO.to_owned()],
                }],
                endpoint_procedures: Vec::new(),
                children: Vec::new(),
            }],
        },
        initial_selection: Selection::Node(NodeId(0)),
    }
}

/// Simple leaf-call scenario.
fn echo_leaf() -> ScenarioDefinition {
    ScenarioDefinition {
        name: "Echo Leaf".to_owned(),
        description: "Call a concrete leaf and watch the hook finish normally.".to_owned(),
        highlights: vec![
            "The leaf uses the built-in `Echo` behavior from the core runtime.".to_owned(),
            "The final response sets `end_hook = true`.".to_owned(),
        ],
        root: NodeSpec {
            segment: String::new(),
            title: "Root".to_owned(),
            description: "The operator origin for all demo calls.".to_owned(),
            leaves: Vec::new(),
            endpoint_procedures: Vec::new(),
            children: vec![NodeSpec {
                segment: "services".to_owned(),
                title: "Services".to_owned(),
                description: "Hosts protocol-visible demo leaves.".to_owned(),
                leaves: vec![LeafSpec {
                    name: "echo".to_owned(),
                    description: "Simple echo leaf.".to_owned(),
                    kind: LeafKind::Echo,
                    procedures: vec![PROC_ECHO.to_owned()],
                }],
                endpoint_procedures: vec![EndpointProcedureSpec {
                    procedure_id: PROC_CHUNKED.to_owned(),
                    description: "Three response packets with a clear final chunk.".to_owned(),
                    kind: EndpointProcedureKind::ChunkedGreeting,
                }],
                children: Vec::new(),
            }],
        },
        initial_selection: Selection::Leaf {
            node_id: NodeId(1),
            leaf_name: "echo".to_owned(),
        },
    }
}

/// Multi-branch routing scenario.
fn branch_routing() -> ScenarioDefinition {
    ScenarioDefinition {
        name: "Branch Routing".to_owned(),
        description: "Demonstrates longest-prefix routing across sibling branches.".to_owned(),
        highlights: vec![
            "Packets descend through the most specific child path.".to_owned(),
            "Responses route back upward and then down into the hook host subtree.".to_owned(),
        ],
        root: NodeSpec {
            segment: String::new(),
            title: "Root".to_owned(),
            description: "The routing apex.".to_owned(),
            leaves: Vec::new(),
            endpoint_procedures: Vec::new(),
            children: vec![
                NodeSpec {
                    segment: "alpha".to_owned(),
                    title: "Alpha".to_owned(),
                    description: "Intermediate branch.".to_owned(),
                    leaves: Vec::new(),
                    endpoint_procedures: Vec::new(),
                    children: vec![NodeSpec {
                        segment: "beta".to_owned(),
                        title: "Beta".to_owned(),
                        description: "Nested endpoint for longest-prefix routing.".to_owned(),
                        leaves: vec![LeafSpec {
                            name: "echo".to_owned(),
                            description: "Nested echo leaf.".to_owned(),
                            kind: LeafKind::Echo,
                            procedures: vec![PROC_ECHO.to_owned()],
                        }],
                        endpoint_procedures: vec![EndpointProcedureSpec {
                            procedure_id: PROC_PING.to_owned(),
                            description: "Checks routed endpoint procedures.".to_owned(),
                            kind: EndpointProcedureKind::Ping,
                        }],
                        children: Vec::new(),
                    }],
                },
                NodeSpec {
                    segment: "gamma".to_owned(),
                    title: "Gamma".to_owned(),
                    description: "Sibling endpoint used to make the route tree non-trivial."
                        .to_owned(),
                    leaves: vec![LeafSpec {
                        name: "echo".to_owned(),
                        description: "Sibling echo leaf.".to_owned(),
                        kind: LeafKind::Echo,
                        procedures: vec![PROC_ECHO.to_owned()],
                    }],
                    endpoint_procedures: Vec::new(),
                    children: Vec::new(),
                },
            ],
        },
        initial_selection: Selection::Node(NodeId(2)),
    }
}

/// Long-lived hook scenario.
fn bidirectional_chat() -> ScenarioDefinition {
    ScenarioDefinition {
        name: "Bidirectional Chat".to_owned(),
        description: "Keeps a hook active so the root can continue sending `Data` packets."
            .to_owned(),
        highlights: vec![
            "After activation, either side may send hook data first.".to_owned(),
            "The chat handler exists outside the core runtime so the demo can show application-level behavior without changing the protocol.".to_owned(),
        ],
        root: NodeSpec {
            segment: String::new(),
            title: "Root".to_owned(),
            description: "The operator-controlled hook host.".to_owned(),
            leaves: Vec::new(),
            endpoint_procedures: Vec::new(),
            children: vec![NodeSpec {
                segment: "chat".to_owned(),
                title: "Chat Host".to_owned(),
                description: "Endpoint with a long-lived hook-backed chat procedure.".to_owned(),
                leaves: Vec::new(),
                endpoint_procedures: vec![EndpointProcedureSpec {
                    procedure_id: PROC_CHAT.to_owned(),
                    description: "Bidirectional hook that replies until it sees `bye`.".to_owned(),
                    kind: EndpointProcedureKind::Chat,
                }],
                children: Vec::new(),
            }],
        },
        initial_selection: Selection::Node(NodeId(1)),
    }
}

/// Protocol-fault walkthrough.
fn fault_showcase() -> ScenarioDefinition {
    ScenarioDefinition {
        name: "Fault Showcase".to_owned(),
        description: "Use valid and invalid calls to trigger protocol-level faults.".to_owned(),
        highlights: vec![
            "Unknown leaf and unknown procedure faults are attributed to the declared hook."
                .to_owned(),
            "Packets with an invalid hook peer are rejected and faulted locally.".to_owned(),
        ],
        root: NodeSpec {
            segment: String::new(),
            title: "Root".to_owned(),
            description: "Runs fault-focused experiments.".to_owned(),
            leaves: Vec::new(),
            endpoint_procedures: Vec::new(),
            children: vec![NodeSpec {
                segment: "faults".to_owned(),
                title: "Fault Lab".to_owned(),
                description: "One endpoint with one known leaf and one known procedure.".to_owned(),
                leaves: vec![LeafSpec {
                    name: "echo".to_owned(),
                    description: "Valid leaf used to contrast unknown-leaf failures.".to_owned(),
                    kind: LeafKind::Echo,
                    procedures: vec![PROC_ECHO.to_owned()],
                }],
                endpoint_procedures: vec![EndpointProcedureSpec {
                    procedure_id: PROC_PING.to_owned(),
                    description: "Known procedure for contrast against unknown procedures."
                        .to_owned(),
                    kind: EndpointProcedureKind::Ping,
                }],
                children: Vec::new(),
            }],
        },
        initial_selection: Selection::Node(NodeId(1)),
    }
}
