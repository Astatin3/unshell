//! Endpoint runtime and traits.

use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec,
    vec::Vec,
};
use core::fmt;
use rkyv::{rancor::Error as RkyvError, to_bytes};

use crate::protocol::{
    CallMessage, DataMessage, EndpointIntrospection, FaultMessage, FrameBytes, FrameError,
    HookTarget, LeafIntrospection, LeafIntrospectionSummary, PacketHeader, PacketType,
    ProtocolFault, ValidationError, decode_frame, encode_packet,
    introspection::INTROSPECTION_PROCEDURE_ID, validate_call, validate_header,
    validate_procedure_id,
};

use super::{ActiveHook, HookKey, HookTable, PendingHook, RouteDecision, route_destination};

/// Local connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Unregistered,
    Registered,
}

/// Registered child route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildRoute {
    pub path: Vec<String>,
    pub state: ConnectionState,
}

impl ChildRoute {
    pub fn registered(path: Vec<String>) -> Self {
        Self {
            path,
            state: ConnectionState::Registered,
        }
    }
}

/// Leaf behavior for test runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeafBehavior {
    Echo,
}

/// Static leaf description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeafSpec {
    pub name: String,
    pub procedures: Vec<String>,
    pub behavior: LeafBehavior,
}

/// Arrival side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ingress {
    Parent,
    Child(Vec<String>),
    Local,
}

/// Local events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalEvent {
    Call {
        header: PacketHeader,
        message: CallMessage,
    },
    Data {
        header: PacketHeader,
        message: DataMessage,
    },
    Fault {
        header: PacketHeader,
        message: FaultMessage,
    },
}

/// Processing outcome.
#[derive(Debug, Default)]
pub struct EndpointOutcome {
    pub forwards: Vec<(RouteDecision, FrameBytes)>,
    pub events: Vec<LocalEvent>,
    pub dropped: bool,
}

/// Processing error.
#[derive(Debug)]
pub enum EndpointError {
    Frame(FrameError),
    Validation(ValidationError),
}

impl fmt::Display for EndpointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Frame(error) => write!(f, "{error}"),
            Self::Validation(error) => write!(f, "{error}"),
        }
    }
}

impl core::error::Error for EndpointError {}

impl From<FrameError> for EndpointError {
    fn from(value: FrameError) -> Self {
        Self::Frame(value)
    }
}

impl From<ValidationError> for EndpointError {
    fn from(value: ValidationError) -> Self {
        Self::Validation(value)
    }
}

/// Core trait for a protocol endpoint.
pub trait Endpoint {
    fn path(&self) -> &[String];
    fn receive(
        &mut self,
        ingress: &Ingress,
        frame: FrameBytes,
    ) -> Result<EndpointOutcome, EndpointError>;
}

/// Default endpoint implementation.
#[derive(Debug, Default)]
pub struct ProtocolEndpoint {
    path: Vec<String>,
    parent_path: Option<Vec<String>>,
    children: Vec<ChildRoute>,
    leaves: BTreeMap<String, LeafSpec>,
    endpoint_procedures: BTreeSet<String>,
    hooks: HookTable,
}

impl ProtocolEndpoint {
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

    pub fn add_endpoint_procedure(
        &mut self,
        procedure_id: impl Into<String>,
    ) -> Result<(), EndpointError> {
        let procedure_id = procedure_id.into();
        validate_procedure_id(&procedure_id)?;
        self.endpoint_procedures.insert(procedure_id);
        Ok(())
    }

    pub fn allocate_hook_id(&self) -> u64 {
        self.hooks.allocate_hook_id(&self.path)
    }

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
                .map(|leaf| leaf.procedures.iter().any(|p| p == &message.procedure_id))
                .unwrap_or(false),
            None => self.endpoint_procedures.contains(&message.procedure_id),
        };

        if !supported {
            let fault = if header
                .dst_leaf
                .as_ref()
                .is_some_and(|name| !self.leaves.contains_key(name))
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
            .and_then(|name| self.leaves.get(name))
        {
            Some(leaf) if leaf.behavior == LeafBehavior::Echo && key.is_some() => {
                let hook = message.response_hook.expect("synchronized");
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
            to_bytes::<RkyvError>(&LeafIntrospection {
                leaf_name: leaf_name.clone(),
                procedures: leaf.procedures.clone(),
            })
            .expect("serialize")
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
            .expect("serialize")
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
        let key = HookKey::new(self.path.clone(), header.hook_id.expect("validated"));

        if self.hooks.active(&key).is_none() {
            let matches = self.hooks.pending(&key).is_some_and(|p| {
                p.caller_src_path == header.src_path && p.procedure_id == message.procedure_id
            });
            if matches {
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

    fn handle_local_fault(
        &mut self,
        header: PacketHeader,
        message: FaultMessage,
    ) -> Result<EndpointOutcome, EndpointError> {
        let key = HookKey::new(self.path.clone(), header.hook_id.expect("validated"));
        let matches = self
            .hooks
            .active(&key)
            .is_some_and(|a| a.peer_path == header.src_path)
            || self
                .hooks
                .pending(&key)
                .is_some_and(|p| p.caller_src_path == header.src_path);
        if !matches {
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
        let frame = encode_packet(&header, &FaultMessage { fault })?;
        Ok(EndpointOutcome {
            forwards: vec![(RouteDecision::Parent, frame)],
            ..EndpointOutcome::default()
        })
    }

    fn decide_route(&self, dst_path: &[String]) -> RouteDecision {
        let child_paths: Vec<Vec<String>> = self
            .children
            .iter()
            .filter(|c| c.state == ConnectionState::Registered)
            .map(|c| c.path.clone())
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
                .map_or(self.path.is_empty(), |p| p == src_path),
            Ingress::Child(path) => path == src_path,
            Ingress::Local => src_path == self.path,
        }
    }
}

impl Endpoint for ProtocolEndpoint {
    fn path(&self) -> &[String] {
        &self.path
    }

    fn receive(
        &mut self,
        ingress: &Ingress,
        frame: FrameBytes,
    ) -> Result<EndpointOutcome, EndpointError> {
        let parsed = decode_frame(&frame)?;
        let header = parsed.deserialize_header();
        validate_header(&header)?;
        if !self.valid_source_for_ingress(ingress, &header.src_path) {
            return Ok(EndpointOutcome {
                dropped: true,
                ..EndpointOutcome::default()
            });
        }

        match header.packet_type {
            PacketType::Call => {
                let message = parsed.deserialize_call()?;
                if !matches!(ingress, Ingress::Parent | Ingress::Local) {
                    return Ok(EndpointOutcome {
                        dropped: true,
                        ..EndpointOutcome::default()
                    });
                }
                validate_call(&header, &message)?;
                match self.decide_route(&header.dst_path) {
                    RouteDecision::Child(idx) => Ok(EndpointOutcome {
                        forwards: vec![(RouteDecision::Child(idx), frame)],
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
            PacketType::Data => {
                let message = parsed.deserialize_data()?;
                match self.decide_route(&header.dst_path) {
                    RouteDecision::Local => self.handle_local_data(header, message),
                    _ => Ok(EndpointOutcome {
                        dropped: true,
                        ..EndpointOutcome::default()
                    }),
                }
            }
            PacketType::Fault => {
                let message = parsed.deserialize_fault()?;
                match self.decide_route(&header.dst_path) {
                    RouteDecision::Local => self.handle_local_fault(header, message),
                    _ => Ok(EndpointOutcome {
                        dropped: true,
                        ..EndpointOutcome::default()
                    }),
                }
            }
        }
    }
}
