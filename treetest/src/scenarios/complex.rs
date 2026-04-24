//! Larger sandbox scenarios.

use crate::model::{
    EndpointProcedureKind, EndpointProcedureSpec, LeafKind, LeafSpec, NodeId, NodeSpec,
    ScenarioDefinition, Selection,
};

use super::simple::{PROC_CHAT, PROC_CHUNKED, PROC_ECHO, PROC_PING};

pub(super) fn scenarios() -> Vec<ScenarioDefinition> {
    vec![complex_tree()]
}

fn complex_tree() -> ScenarioDefinition {
    ScenarioDefinition {
        name: "Complex Tree".to_owned(),
        description: "A larger topology that combines leaf calls, endpoint procedures, and nested routing.".to_owned(),
        highlights: vec![
            "Use this as a sandbox after learning the smaller scenarios.".to_owned(),
            "The tree contains both leaf and endpoint interactions so the UI inspector stays interesting.".to_owned(),
        ],
        root: NodeSpec {
            segment: String::new(),
            title: "Root".to_owned(),
            description: "Primary operator endpoint.".to_owned(),
            leaves: Vec::new(),
            endpoint_procedures: vec![EndpointProcedureSpec {
                procedure_id: PROC_PING.to_owned(),
                description: "Root-local endpoint procedure for comparison with remote calls.".to_owned(),
                kind: EndpointProcedureKind::Ping,
            }],
            children: vec![
                NodeSpec {
                    segment: "alpha".to_owned(),
                    title: "Alpha".to_owned(),
                    description: "Left branch.".to_owned(),
                    leaves: vec![LeafSpec {
                        name: "echo".to_owned(),
                        description: "Echo leaf on alpha.".to_owned(),
                        kind: LeafKind::Echo,
                        procedures: vec![PROC_ECHO.to_owned()],
                    }],
                    endpoint_procedures: vec![EndpointProcedureSpec {
                        procedure_id: PROC_CHUNKED.to_owned(),
                        description: "Chunked endpoint response.".to_owned(),
                        kind: EndpointProcedureKind::ChunkedGreeting,
                    }],
                    children: vec![NodeSpec {
                        segment: "deep".to_owned(),
                        title: "Alpha Deep".to_owned(),
                        description: "Nested node for multi-hop traffic.".to_owned(),
                        leaves: vec![LeafSpec {
                            name: "echo".to_owned(),
                            description: "Deep nested echo leaf.".to_owned(),
                            kind: LeafKind::Echo,
                            procedures: vec![PROC_ECHO.to_owned()],
                        }],
                        endpoint_procedures: Vec::new(),
                        children: Vec::new(),
                    }],
                },
                NodeSpec {
                    segment: "beta".to_owned(),
                    title: "Beta".to_owned(),
                    description: "Right branch.".to_owned(),
                    leaves: Vec::new(),
                    endpoint_procedures: vec![EndpointProcedureSpec {
                        procedure_id: PROC_CHAT.to_owned(),
                        description: "Long-lived chat procedure.".to_owned(),
                        kind: EndpointProcedureKind::Chat,
                    }],
                    children: vec![NodeSpec {
                        segment: "gamma".to_owned(),
                        title: "Gamma".to_owned(),
                        description: "Nested branch with its own ping procedure.".to_owned(),
                        leaves: vec![LeafSpec {
                            name: "echo".to_owned(),
                            description: "Gamma echo leaf.".to_owned(),
                            kind: LeafKind::Echo,
                            procedures: vec![PROC_ECHO.to_owned()],
                        }],
                        endpoint_procedures: vec![EndpointProcedureSpec {
                            procedure_id: PROC_PING.to_owned(),
                            description: "Nested ping procedure.".to_owned(),
                            kind: EndpointProcedureKind::Ping,
                        }],
                        children: Vec::new(),
                    }],
                },
            ],
        },
        initial_selection: Selection::Node(NodeId(0)),
    }
}
