//! Minimal endpoint runtime for protocol tests.

use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec,
    vec::Vec,
};
use core::fmt;
use rkyv::{rancor::Error as RkyvError, to_bytes};

use crate::{
    protocol::{
        CallMessage, DataMessage, EndpointIntrospection, FaultMessage, FrameBytes, FrameError,
        HookTarget, LeafIntrospection, LeafIntrospectionSummary, PacketHeader, PacketType,
        ProtocolFault, decode_frame, encode_packet, introspection::INTROSPECTION_PROCEDURE_ID,
        validate_call, validate_header, validate_procedure_id,
    },
    tree::{ActiveHook, HookKey, HookTable, PendingHook, RouteDecision, route_destination},
};

/// Local connection state defined by the protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Connected but not routable.
    Unregistered,
    /// Admitted into local routing.
    Registered,
}

/// Registered child route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildRoute {
    /// Child endpoint path.
    pub path: Vec<String>,
    /// Local connection state.
    pub state: ConnectionState,
}

impl ChildRoute {
    /// Creates a registered child route.
    pub fn registered(path: Vec<String>) -> Self {
        Self {
            path,
            state: ConnectionState::Registered,
        }
    }
}

/// Basic leaf behavior used by the test protocol runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeafBehavior {
    /// Echoes the call data back in one `Data` packet.
    Echo,
}

/// Static leaf description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeafSpec {
    /// Local leaf name.
    pub name: String,
    /// Supported procedures.
    pub procedures: Vec<String>,
    /// Test behavior.
    pub behavior: LeafBehavior,
}

/// How a packet arrived at the endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ingress {
    /// From the direct parent.
    Parent,
    /// From a direct child path.
    Child(Vec<String>),
    /// Originated locally.
    Local,
}

/// Locally delivered events produced by protocol processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalEvent {
    /// A supported local call with no response hook.
    Call {
        header: PacketHeader,
        message: CallMessage,
    },
    /// Locally delivered data.
    Data {
        header: PacketHeader,
        message: DataMessage,
    },
    /// Locally delivered or synthesized fault.
    Fault {
        header: PacketHeader,
        message: FaultMessage,
    },
}

/// Output from processing one frame.
#[derive(Debug, Default)]
pub struct EndpointOutcome {
    /// Frames to forward. The frame bytes are moved, not cloned.
    pub forwards: Vec<(RouteDecision, FrameBytes)>,
    /// Events delivered locally.
    pub events: Vec<LocalEvent>,
    /// Whether the packet was silently dropped.
    pub dropped: bool,
}

/// Endpoint processing failure.
#[derive(Debug)]
pub enum EndpointError {
    /// Frame parsing failed.
    Frame(FrameError),
    /// Validation failed.
    Validation(crate::protocol::ValidationError),
}

impl fmt::Display for EndpointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Frame(error) => write!(f, "{error}"),
            Self::Validation(error) => write!(f, "{error}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EndpointError {}

impl From<FrameError> for EndpointError {
    fn from(value: FrameError) -> Self {
        Self::Frame(value)
    }
}

impl From<crate::protocol::ValidationError> for EndpointError {
    fn from(value: crate::protocol::ValidationError) -> Self {
        Self::Validation(value)
    }
}

/// Local endpoint model suitable for tests and later integration work.
#[derive(Debug, Default)]
pub struct Endpoint {
    path: Vec<String>,
    parent_path: Option<Vec<String>>,
    children: Vec<ChildRoute>,
    leaves: BTreeMap<String, LeafSpec>,
    endpoint_procedures: BTreeSet<String>,
    hooks: HookTable,
}

impl Endpoint {
    /// Creates an endpoint with explicit path, parent, children, and leaves.
    pub fn new(
        path: Vec<String>,
        parent_path: Option<Vec<String>>,
        children: Vec<ChildRoute>,
        leaves: Vec<LeafSpec>,
    ) -> Self {
        Self {
            path,
            parent_path,
            children,
            leaves: leaves
                .into_iter()
                .map(|leaf| (leaf.name.clone(), leaf))
                .collect(),
            endpoint_procedures: BTreeSet::new(),
            hooks: HookTable::default(),
        }
    }

    /// Returns the local endpoint path.
    pub fn path(&self) -> &[String] {
        &self.path
    }

    /// Returns the hook table for assertions.
    pub fn hooks(&self) -> &HookTable {
        &self.hooks
    }

    /// Registers an endpoint-level procedure.
    ///
    /// # Errors
    ///
    /// Returns [`EndpointError`] when the procedure id is invalid.
    pub fn add_endpoint_procedure(
        &mut self,
        procedure_id: impl Into<String>,
    ) -> Result<(), EndpointError> {
        let procedure_id = procedure_id.into();
        validate_procedure_id(&procedure_id)?;
        self.endpoint_procedures.insert(procedure_id);
        Ok(())
    }

    /// Allocates a new local hook id.
    pub fn allocate_hook_id(&self) -> u64 {
        self.hooks.allocate_hook_id(&self.path)
    }

    /// Creates an outbound `Call` frame and registers host-side hook state when needed.
    ///
    /// # Errors
    ///
    /// Returns [`EndpointError`] when validation or framing fails.
    pub fn make_call(
        &mut self,
        dst_path: Vec<String>,
        dst_leaf: Option<String>,
        procedure_id: impl Into<String>,
        response_hook_id: Option<u64>,
        data: Vec<u8>,
    ) -> Result<FrameBytes, EndpointError> {
        let procedure_id = procedure_id.into();
        validate_procedure_id(&procedure_id)?;
        let response_hook = response_hook_id.map(|hook_id| HookTarget {
            hook_id,
            return_path: self.path.clone(),
        });
        let header = PacketHeader {
            packet_type: PacketType::Call,
            src_path: self.path.clone(),
            dst_path: dst_path.clone(),
            dst_leaf: dst_leaf.clone(),
            hook_id: None,
        };
        let call = CallMessage {
            procedure_id: procedure_id.clone(),
            data,
            response_hook,
        };
        validate_header(&header)?;
        validate_call(&header, &call)?;

        if let Some(hook) = &call.response_hook {
            self.hooks.insert_active(ActiveHook {
                return_path: hook.return_path.clone(),
                hook_id: hook.hook_id,
                peer_path: dst_path,
                procedure_id,
                dst_leaf,
                peer_finished: false,
            });
        }

        Ok(encode_packet(&header, &call)?)
    }

    /// Creates an outbound `Data` frame.
    ///
    /// # Errors
    ///
    /// Returns [`EndpointError`] when validation or framing fails.
    pub fn make_data(
        &self,
        dst_path: Vec<String>,
        hook_id: u64,
        procedure_id: impl Into<String>,
        data: Vec<u8>,
        end_hook: bool,
    ) -> Result<FrameBytes, EndpointError> {
        let procedure_id = procedure_id.into();
        validate_procedure_id(&procedure_id)?;
        let header = PacketHeader {
            packet_type: PacketType::Data,
            src_path: self.path.clone(),
            dst_path,
            dst_leaf: None,
            hook_id: Some(hook_id),
        };
        let message = DataMessage {
            procedure_id,
            data,
            end_hook,
        };
        validate_header(&header)?;
        Ok(encode_packet(&header, &message)?)
    }

    /// Processes one framed packet.
    ///
    /// # Errors
    ///
    /// Returns [`EndpointError`] when frame decoding or validation fails.
    pub fn receive(
        &mut self,
        ingress: &Ingress,
        frame: FrameBytes,
    ) -> Result<EndpointOutcome, EndpointError> {
        enum OwnedPayload {
            Call(PacketHeader, CallMessage),
            Data(PacketHeader, DataMessage),
            Fault(PacketHeader, FaultMessage),
        }

        let owned = {
            let parsed = decode_frame(&frame)?;
            let header = parsed.deserialize_header();
            validate_header(&header)?;
            match header.packet_type {
                PacketType::Call => OwnedPayload::Call(header, parsed.deserialize_call()?),
                PacketType::Data => OwnedPayload::Data(header, parsed.deserialize_data()?),
                PacketType::Fault => OwnedPayload::Fault(header, parsed.deserialize_fault()?),
            }
        };

        let src_path = match &owned {
            OwnedPayload::Call(header, _) => &header.src_path,
            OwnedPayload::Data(header, _) => &header.src_path,
            OwnedPayload::Fault(header, _) => &header.src_path,
        };

        if !self.valid_source_for_ingress(ingress, src_path) {
            return Ok(EndpointOutcome {
                dropped: true,
                ..EndpointOutcome::default()
            });
        }

        match owned {
            OwnedPayload::Call(header, message) => {
                self.receive_call(ingress, frame, header, message)
            }
            OwnedPayload::Data(header, message) => self.receive_data(header, message),
            OwnedPayload::Fault(header, message) => self.receive_fault(header, message),
        }
    }

    fn receive_call(
        &mut self,
        ingress: &Ingress,
        frame: FrameBytes,
        header: PacketHeader,
        message: CallMessage,
    ) -> Result<EndpointOutcome, EndpointError> {
        if !matches!(ingress, Ingress::Parent | Ingress::Local) {
            return Ok(EndpointOutcome {
                dropped: true,
                ..EndpointOutcome::default()
            });
        }

        validate_call(&header, &message)?;
        match self.decide_route(&header.dst_path) {
            RouteDecision::Child(index) => Ok(EndpointOutcome {
                forwards: vec![(RouteDecision::Child(index), frame)],
                ..EndpointOutcome::default()
            }),
            RouteDecision::Parent => Ok(EndpointOutcome {
                forwards: vec![(RouteDecision::Parent, frame)],
                ..EndpointOutcome::default()
            }),
            RouteDecision::Drop => Ok(EndpointOutcome {
                dropped: true,
                ..EndpointOutcome::default()
            }),
            RouteDecision::Local => self.handle_local_call(header, message),
        }
    }

    fn receive_data(
        &mut self,
        header: PacketHeader,
        message: DataMessage,
    ) -> Result<EndpointOutcome, EndpointError> {
        match self.decide_route(&header.dst_path) {
            RouteDecision::Child(_) | RouteDecision::Parent => Ok(EndpointOutcome {
                dropped: true,
                ..EndpointOutcome::default()
            }),
            RouteDecision::Drop => Ok(EndpointOutcome {
                dropped: true,
                ..EndpointOutcome::default()
            }),
            RouteDecision::Local => self.handle_local_data(header, message),
        }
    }

    fn receive_fault(
        &mut self,
        header: PacketHeader,
        message: FaultMessage,
    ) -> Result<EndpointOutcome, EndpointError> {
        match self.decide_route(&header.dst_path) {
            RouteDecision::Child(_) | RouteDecision::Parent => Ok(EndpointOutcome {
                dropped: true,
                ..EndpointOutcome::default()
            }),
            RouteDecision::Drop => Ok(EndpointOutcome {
                dropped: true,
                ..EndpointOutcome::default()
            }),
            RouteDecision::Local => {
                let key = HookKey::new(
                    self.path.clone(),
                    header.hook_id.expect("validated hook id"),
                );
                let matches_active = self
                    .hooks
                    .active(&key)
                    .map(|active| active.peer_path == header.src_path)
                    .unwrap_or(false);
                let matches_pending = self
                    .hooks
                    .pending(&key)
                    .map(|pending| pending.caller_src_path == header.src_path)
                    .unwrap_or(false);
                if !(matches_active || matches_pending) {
                    return Ok(EndpointOutcome {
                        dropped: true,
                        ..EndpointOutcome::default()
                    });
                }
                self.hooks.remove_active(&key);
                self.hooks.remove_pending(&key);
                Ok(EndpointOutcome {
                    events: vec![LocalEvent::Fault { header, message }],
                    ..EndpointOutcome::default()
                })
            }
        }
    }

    fn handle_local_call(
        &mut self,
        header: PacketHeader,
        message: CallMessage,
    ) -> Result<EndpointOutcome, EndpointError> {
        let key = message
            .response_hook
            .as_ref()
            .map(|hook| HookKey::new(hook.return_path.clone(), hook.hook_id));

        if let Some(hook) = &message.response_hook {
            self.hooks.insert_pending(PendingHook {
                caller_src_path: header.src_path.clone(),
                return_path: hook.return_path.clone(),
                hook_id: hook.hook_id,
                procedure_id: message.procedure_id.clone(),
                dst_leaf: header.dst_leaf.clone(),
            });
        }

        if message.procedure_id == INTROSPECTION_PROCEDURE_ID {
            return self.handle_introspection(&header, key);
        }

        let supported = match &header.dst_leaf {
            Some(leaf_name) => self
                .leaves
                .get(leaf_name)
                .map(|leaf| {
                    leaf.procedures
                        .iter()
                        .any(|candidate| candidate == &message.procedure_id)
                })
                .unwrap_or(false),
            None => self.endpoint_procedures.contains(&message.procedure_id),
        };

        if !supported {
            let fault = if header
                .dst_leaf
                .as_ref()
                .is_some_and(|leaf_name| !self.leaves.contains_key(leaf_name))
            {
                ProtocolFault::UnknownLeaf
            } else {
                ProtocolFault::UnknownProcedure
            };
            return self.emit_fault_if_possible(key, fault);
        }

        if let Some(key) = &key {
            self.hooks.activate_pending(key, header.src_path.clone());
        }

        match header
            .dst_leaf
            .as_ref()
            .and_then(|leaf_name| self.leaves.get(leaf_name))
        {
            Some(LeafSpec {
                behavior: LeafBehavior::Echo,
                ..
            }) if key.is_some() => {
                let hook = message
                    .response_hook
                    .expect("key and hook are synchronized");
                let response = DataMessage {
                    procedure_id: message.procedure_id.clone(),
                    data: message.data,
                    end_hook: true,
                };
                let response_header = PacketHeader {
                    packet_type: PacketType::Data,
                    src_path: self.path.clone(),
                    dst_path: hook.return_path.clone(),
                    dst_leaf: None,
                    hook_id: Some(hook.hook_id),
                };
                let frame = encode_packet(&response_header, &response)?;
                self.hooks
                    .remove_active(&HookKey::new(hook.return_path, hook.hook_id));
                Ok(EndpointOutcome {
                    forwards: vec![(RouteDecision::Parent, frame)],
                    ..EndpointOutcome::default()
                })
            }
            _ => Ok(EndpointOutcome {
                events: vec![LocalEvent::Call { header, message }],
                ..EndpointOutcome::default()
            }),
        }
    }

    fn handle_introspection(
        &mut self,
        header: &PacketHeader,
        key: Option<HookKey>,
    ) -> Result<EndpointOutcome, EndpointError> {
        let Some(key) = key else {
            return Ok(EndpointOutcome {
                dropped: true,
                ..EndpointOutcome::default()
            });
        };
        self.hooks.activate_pending(&key, header.src_path.clone());

        let payload = if let Some(leaf_name) = &header.dst_leaf {
            let Some(leaf) = self.leaves.get(leaf_name) else {
                return self.emit_fault_if_possible(Some(key), ProtocolFault::UnknownLeaf);
            };
            // WARNING: introspection nests one archived payload inside `DataMessage.data`.
            // This inner allocation is required because the protocol defines `data` as opaque bytes.
            to_bytes::<RkyvError>(&LeafIntrospection {
                leaf_name: leaf_name.clone(),
                procedures: leaf.procedures.clone(),
            })
            .expect("leaf introspection should serialize")
            .to_vec()
        } else {
            to_bytes::<RkyvError>(&EndpointIntrospection {
                leaves: self
                    .leaves
                    .values()
                    .map(|leaf| LeafIntrospectionSummary {
                        leaf_name: leaf.name.clone(),
                        procedures: leaf.procedures.clone(),
                    })
                    .collect(),
            })
            .expect("endpoint introspection should serialize")
            .to_vec()
        };

        let response_header = PacketHeader {
            packet_type: PacketType::Data,
            src_path: self.path.clone(),
            dst_path: key.return_path.clone(),
            dst_leaf: None,
            hook_id: Some(key.hook_id),
        };
        let response = DataMessage {
            procedure_id: String::new(),
            data: payload,
            end_hook: true,
        };
        let frame = encode_packet(&response_header, &response)?;
        self.hooks.remove_active(&key);
        Ok(EndpointOutcome {
            forwards: vec![(RouteDecision::Parent, frame)],
            ..EndpointOutcome::default()
        })
    }

    fn handle_local_data(
        &mut self,
        header: PacketHeader,
        message: DataMessage,
    ) -> Result<EndpointOutcome, EndpointError> {
        let key = HookKey::new(
            self.path.clone(),
            header.hook_id.expect("validated hook id"),
        );

        if self.hooks.active(&key).is_none() {
            let pending_matches = self
                .hooks
                .pending(&key)
                .map(|pending| {
                    pending.caller_src_path == header.src_path
                        && pending.procedure_id == message.procedure_id
                })
                .unwrap_or(false);
            if pending_matches {
                self.hooks.activate_pending(&key, header.src_path.clone());
            }
        }

        let Some(active) = self.hooks.active(&key).cloned() else {
            return Ok(EndpointOutcome {
                dropped: true,
                ..EndpointOutcome::default()
            });
        };

        if active.peer_path != header.src_path || active.procedure_id != message.procedure_id {
            self.hooks.remove_active(&key);
            self.hooks.remove_pending(&key);
            return Ok(EndpointOutcome {
                events: vec![LocalEvent::Fault {
                    header: PacketHeader {
                        packet_type: PacketType::Fault,
                        src_path: header.src_path,
                        dst_path: self.path.clone(),
                        dst_leaf: None,
                        hook_id: Some(key.hook_id),
                    },
                    message: FaultMessage {
                        fault: ProtocolFault::InvalidHookPeer,
                    },
                }],
                ..EndpointOutcome::default()
            });
        }

        if message.end_hook {
            self.hooks.remove_active(&key);
        }

        Ok(EndpointOutcome {
            events: vec![LocalEvent::Data { header, message }],
            ..EndpointOutcome::default()
        })
    }

    fn emit_fault_if_possible(
        &mut self,
        key: Option<HookKey>,
        fault: ProtocolFault,
    ) -> Result<EndpointOutcome, EndpointError> {
        let Some(key) = key else {
            return Ok(EndpointOutcome {
                dropped: true,
                ..EndpointOutcome::default()
            });
        };
        self.hooks.remove_pending(&key);
        self.hooks.remove_active(&key);
        let header = PacketHeader {
            packet_type: PacketType::Fault,
            src_path: self.path.clone(),
            dst_path: key.return_path.clone(),
            dst_leaf: None,
            hook_id: Some(key.hook_id),
        };
        let message = FaultMessage { fault };
        let frame = encode_packet(&header, &message)?;
        Ok(EndpointOutcome {
            forwards: vec![(RouteDecision::Parent, frame)],
            ..EndpointOutcome::default()
        })
    }

    fn decide_route(&self, dst_path: &[String]) -> RouteDecision {
        let child_paths: Vec<Vec<String>> = self
            .children
            .iter()
            .filter(|child| child.state == ConnectionState::Registered)
            .map(|child| child.path.clone())
            .collect();
        route_destination(
            &self.path,
            &child_paths,
            self.parent_path.is_some(),
            dst_path,
        )
    }

    fn valid_source_for_ingress(&self, ingress: &Ingress, src_path: &[String]) -> bool {
        match ingress {
            Ingress::Parent => self
                .parent_path
                .as_ref()
                .map_or(self.path.is_empty(), |path| path == src_path),
            Ingress::Child(path) => path == src_path,
            Ingress::Local => src_path == self.path,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::introspection::ArchivedEndpointIntrospection;
    use crate::protocol::{HookTarget, deserialize_archived_bytes};

    fn echo_leaf() -> LeafSpec {
        LeafSpec {
            name: String::from("echo"),
            procedures: vec![String::from("org.product.v1.echo.roundtrip")],
            behavior: LeafBehavior::Echo,
        }
    }

    #[test]
    fn introspection_returns_payload_and_clears_hook() {
        let mut child = Endpoint::new(
            vec![String::from("child")],
            Some(Vec::new()),
            Vec::new(),
            vec![echo_leaf()],
        );
        let header = PacketHeader {
            packet_type: PacketType::Call,
            src_path: Vec::new(),
            dst_path: vec![String::from("child")],
            dst_leaf: None,
            hook_id: None,
        };
        let call = CallMessage {
            procedure_id: String::new(),
            data: Vec::new(),
            response_hook: Some(HookTarget {
                hook_id: 1,
                return_path: Vec::new(),
            }),
        };

        let outcome = child
            .receive(
                &Ingress::Parent,
                encode_packet(&header, &call).expect("frame"),
            )
            .expect("receive should succeed");
        let (_, frame) = outcome
            .forwards
            .first()
            .expect("forwarded frame should exist");
        let parsed = decode_frame(frame).expect("data frame");
        let data = parsed.deserialize_data().expect("data payload");
        let payload = deserialize_archived_bytes::<
            ArchivedEndpointIntrospection,
            EndpointIntrospection,
        >(&data.data)
        .expect("introspection payload");
        assert_eq!(payload.leaves.len(), 1);
        assert_eq!(child.hooks().active_len(), 0);
    }

    #[test]
    fn invalid_peer_generates_local_fault_event() {
        let mut root = Endpoint::new(Vec::new(), None, Vec::new(), Vec::new());
        let _call = root
            .make_call(
                vec![String::from("child")],
                None,
                String::from("org.product.v1.echo.roundtrip"),
                Some(7),
                Vec::new(),
            )
            .expect("call should encode");
        let frame = root
            .make_data(
                Vec::new(),
                7,
                String::from("org.product.v1.echo.roundtrip"),
                b"bad".to_vec(),
                false,
            )
            .expect("data should encode");
        let parsed = decode_frame(&frame).expect("frame should decode");
        let mut header = parsed.deserialize_header();
        header.src_path = vec![String::from("other")];
        let bad_frame = encode_packet(
            &header,
            &parsed.deserialize_data().expect("data should decode"),
        )
        .expect("bad frame should encode");
        let outcome = root
            .receive(&Ingress::Child(vec![String::from("other")]), bad_frame)
            .expect("receive should work");
        assert!(matches!(
            outcome.events.first(),
            Some(LocalEvent::Fault { .. })
        ));
    }
}
