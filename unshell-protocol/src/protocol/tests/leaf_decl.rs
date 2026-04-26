use alloc::{string::String, vec};

use crate::leaf;
use crate::protocol::tree::{LeafBinding, LeafDeclaration, ProcedureMetadata, ProtocolLeaf};

struct EndpointHost;
struct Open;
struct Reset;

impl ProcedureMetadata for Open {
    type Leaf = EndpointHost;
    const PROCEDURE_SUFFIX: &'static str = "open";
}

impl ProcedureMetadata for Reset {
    type Leaf = EndpointHost;
    const PROCEDURE_SUFFIX: &'static str = "reset";
}

leaf! {
    id = "org.example.v1.demo",
    procedures = [Open, Reset],
    endpoint_struct = EndpointHost,
}

struct EndpointHalf;
struct TuiHalf;
struct Connect;

impl ProcedureMetadata for Connect {
    type Leaf = EndpointHalf;
    const PROCEDURE_SUFFIX: &'static str = "connect";
}

leaf! {
    name = "chat",
    org = "org",
    product = "example",
    version = "v2",
    procedures = [Connect],
    endpoint_struct = EndpointHalf,
    tui_struct = TuiHalf,
}

struct TuiOnly;
struct Tail;

impl ProcedureMetadata for Tail {
    type Leaf = TuiOnly;
    const PROCEDURE_SUFFIX: &'static str = "tail";
}

leaf! {
    id = "org.example.v1.transcript",
    procedures = [Tail],
    tui_struct = TuiOnly,
}

#[test]
fn leaf_declaration_generates_endpoint_host_metadata() {
    assert_eq!(EndpointHost::protocol_leaf_name(), "org.example.v1.demo");
    assert_eq!(
        EndpointHost::protocol_leaf_spec().procedures,
        vec![
            String::from("org.example.v1.demo.open"),
            String::from("org.example.v1.demo.reset"),
        ]
    );
    assert_eq!(
        <EndpointHost as LeafBinding>::Declaration::leaf_name(),
        "org.example.v1.demo"
    );
}

#[test]
fn leaf_declaration_shares_metadata_between_endpoint_and_tui_hosts() {
    assert_eq!(
        EndpointHalf::protocol_leaf_name(),
        TuiHalf::protocol_leaf_name()
    );
    assert_eq!(
        EndpointHalf::protocol_leaf_spec().procedures,
        TuiHalf::protocol_leaf_spec().procedures
    );
    assert_eq!(
        <EndpointHalf as LeafBinding>::Declaration::procedure_id("connect"),
        Some(String::from("org.example.v2.chat.connect"))
    );
}

#[test]
fn leaf_declaration_supports_tui_only_hosts() {
    assert_eq!(TuiOnly::protocol_leaf_name(), "org.example.v1.transcript");
    assert_eq!(
        <TuiOnly as LeafDeclaration>::procedure_id("tail"),
        Some(String::from("org.example.v1.transcript.tail"))
    );
}
