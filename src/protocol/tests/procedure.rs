use alloc::{borrow::ToOwned, collections::BTreeMap, format, string::String, vec, vec::Vec};
use core::convert::Infallible;

use crate::protocol::tree::{
    Call, ChildRoute, Endpoint, EndpointOutcome, HookKey, Ingress, OutgoingData, Procedure,
    ProcedureEffect, ProcedureRuntime, ProcedureStore, ProtocolEndpoint, encode_call_reply,
};
use crate::protocol::{PacketType, decode_frame};
use crate::{Leaf, Procedure};

fn path(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}

#[derive(Default, Leaf)]
#[leaf(id = "org.example.v1.stream")]
struct StreamLeaf {
    sessions: BTreeMap<HookKey, ProcedureOpen>,
}

impl ProcedureStore<ProcedureOpen> for StreamLeaf {
    fn procedure_sessions(&mut self) -> &mut BTreeMap<HookKey, ProcedureOpen> {
        &mut self.sessions
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Procedure)]
#[procedure(leaf = StreamLeaf, name = "open")]
struct ProcedureOpen {
    prefix: String,
}

impl Procedure<StreamLeaf> for ProcedureOpen {
    type Error = Infallible;
    type Input = String;

    fn open(_leaf: &mut StreamLeaf, call: Call<Self::Input>) -> Result<Self, Self::Error> {
        Ok(Self { prefix: call.input })
    }

    fn on_data(
        _leaf: &mut StreamLeaf,
        session: &mut Self,
        data: crate::protocol::tree::IncomingData,
    ) -> Result<ProcedureEffect, Self::Error> {
        Ok(ProcedureEffect {
            outgoing: vec![OutgoingData {
                dst_path: data.hook_key.return_path,
                hook_id: data.hook_key.hook_id,
                procedure_id: ProcedureOpen::protocol_procedure_id(),
                data: format!(
                    "{}{}",
                    session.prefix,
                    String::from_utf8_lossy(&data.message.data)
                )
                .into_bytes(),
                end_hook: data.message.end_hook,
            }],
            close_session: data.message.end_hook,
        })
    }
}

#[test]
fn procedure_runtime_routes_data_to_stored_session() {
    let endpoint = ProtocolEndpoint::new(
        path(&["agent"]),
        Some(Vec::new()),
        Vec::new(),
        vec![crate::protocol::tree::LeafSpec {
            name: StreamLeaf::protocol_leaf_name(),
            procedures: vec![ProcedureOpen::protocol_procedure_id()],
        }],
    );
    let mut runtime =
        ProcedureRuntime::<StreamLeaf, ProcedureOpen>::new(endpoint, StreamLeaf::default());

    let mut controller = ProtocolEndpoint::new(
        Vec::new(),
        None,
        vec![ChildRoute {
            path: path(&["agent"]),
            registered: true,
        }],
        Vec::new(),
    );
    let hook_id = controller.allocate_hook_id();
    let open = controller
        .send_call(
            path(&["agent"]),
            Some(StreamLeaf::protocol_leaf_name()),
            ProcedureOpen::protocol_procedure_id(),
            Some(hook_id),
            encode_call_reply(&String::from("prefix:")).expect("procedure input should encode"),
        )
        .expect("open call should encode");
    let EndpointOutcome::Forward {
        frame: open_frame, ..
    } = open
    else {
        panic!("controller should forward opening call");
    };
    runtime
        .receive(&Ingress::Parent, open_frame)
        .expect("runtime should open a session");

    let data = controller
        .send_data(
            path(&["agent"]),
            hook_id,
            ProcedureOpen::protocol_procedure_id(),
            b"hello".to_vec(),
            true,
        )
        .expect("data should encode");
    let EndpointOutcome::Forward {
        frame: data_frame, ..
    } = data
    else {
        panic!("controller should forward data frame");
    };
    let outcome = runtime
        .receive(&Ingress::Parent, data_frame)
        .expect("runtime should route data to session");
    let [response_frame] = outcome.frames.as_slice() else {
        panic!("expected one response frame");
    };

    let parsed = decode_frame(response_frame.as_slice()).expect("response frame should decode");
    assert_eq!(parsed.packet_type(), PacketType::Data);
    let message = parsed.deserialize_data().expect("data should deserialize");
    assert!(message.end_hook);
    assert_eq!(String::from_utf8_lossy(&message.data), "prefix:hello");

    let forwarded = controller
        .receive(&Ingress::Child(path(&["agent"])), response_frame.clone())
        .expect("controller should receive session response");
    assert!(matches!(forwarded, EndpointOutcome::Local(_)));
    assert!(runtime.leaf_mut().procedure_sessions().is_empty());
}

#[derive(Default, Leaf)]
#[leaf(id = "org.example.v1.duplex")]
struct DuplexLeaf {
    sessions: BTreeMap<HookKey, DuplexProcedure>,
}

impl ProcedureStore<DuplexProcedure> for DuplexLeaf {
    fn procedure_sessions(&mut self) -> &mut BTreeMap<HookKey, DuplexProcedure> {
        &mut self.sessions
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Procedure)]
#[procedure(leaf = DuplexLeaf, name = "open")]
struct DuplexProcedure {
    saw_peer_close: bool,
}

impl Procedure<DuplexLeaf> for DuplexProcedure {
    type Error = Infallible;
    type Input = ();

    fn open(_leaf: &mut DuplexLeaf, _call: Call<Self::Input>) -> Result<Self, Self::Error> {
        Ok(Self {
            saw_peer_close: false,
        })
    }

    fn on_data(
        _leaf: &mut DuplexLeaf,
        session: &mut Self,
        data: crate::protocol::tree::IncomingData,
    ) -> Result<ProcedureEffect, Self::Error> {
        if data.message.data == b"local-end" {
            return Ok(ProcedureEffect::outgoing(vec![OutgoingData {
                dst_path: data.hook_key.return_path,
                hook_id: data.hook_key.hook_id,
                procedure_id: DuplexProcedure::protocol_procedure_id(),
                data: Vec::new(),
                end_hook: true,
            }]));
        }

        if data.message.end_hook {
            session.saw_peer_close = true;
            return Ok(ProcedureEffect::close(Vec::new()));
        }

        Ok(ProcedureEffect::default())
    }
}

#[test]
fn procedure_runtime_keeps_session_after_local_end_until_explicit_close() {
    let endpoint = ProtocolEndpoint::new(
        path(&["agent"]),
        Some(Vec::new()),
        Vec::new(),
        vec![crate::protocol::tree::LeafSpec {
            name: DuplexLeaf::protocol_leaf_name(),
            procedures: vec![DuplexProcedure::protocol_procedure_id()],
        }],
    );
    let mut runtime =
        ProcedureRuntime::<DuplexLeaf, DuplexProcedure>::new(endpoint, DuplexLeaf::default());

    let mut controller = ProtocolEndpoint::new(
        Vec::new(),
        None,
        vec![ChildRoute {
            path: path(&["agent"]),
            registered: true,
        }],
        Vec::new(),
    );
    let hook_id = controller.allocate_hook_id();
    let open = controller
        .send_call(
            path(&["agent"]),
            Some(DuplexLeaf::protocol_leaf_name()),
            DuplexProcedure::protocol_procedure_id(),
            Some(hook_id),
            encode_call_reply(&()).expect("unit call should encode"),
        )
        .expect("open call should encode");
    let EndpointOutcome::Forward {
        frame: open_frame, ..
    } = open
    else {
        panic!("controller should forward opening call");
    };
    runtime
        .receive(&Ingress::Parent, open_frame)
        .expect("runtime should open duplex session");

    let local_end = controller
        .send_data(
            path(&["agent"]),
            hook_id,
            DuplexProcedure::protocol_procedure_id(),
            b"local-end".to_vec(),
            false,
        )
        .expect("local end trigger should encode");
    let EndpointOutcome::Forward {
        frame: local_end_frame,
        ..
    } = local_end
    else {
        panic!("controller should forward local end trigger");
    };
    let outcome = runtime
        .receive(&Ingress::Parent, local_end_frame)
        .expect("runtime should emit a local end packet");
    assert_eq!(outcome.frames.len(), 1);
    assert_eq!(runtime.leaf_mut().procedure_sessions().len(), 1);

    let peer_end = encode_call_reply(&()).expect("unit value is just a placeholder");
    let peer_end = crate::protocol::encode_packet(
        &crate::protocol::PacketHeader {
            packet_type: PacketType::Data,
            src_path: Vec::new(),
            dst_path: path(&["agent"]),
            dst_leaf: None,
            hook_id: Some(hook_id),
        },
        &crate::protocol::DataMessage {
            procedure_id: DuplexProcedure::protocol_procedure_id(),
            data: peer_end,
            end_hook: true,
        },
    )
    .expect("peer end frame should encode");
    let peer_end_outcome = runtime
        .receive(&Ingress::Parent, peer_end)
        .expect("runtime should accept peer end after local end");
    assert!(peer_end_outcome.frames.is_empty());
    assert!(runtime.leaf_mut().procedure_sessions().is_empty());
}
