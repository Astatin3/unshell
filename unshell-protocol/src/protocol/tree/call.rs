//! Stateful application-layer call runtime built on top of `ProtocolEndpoint`.

use alloc::{string::String, vec, vec::Vec};
use core::fmt;

use rkyv::{Archive, Serialize, rancor::Error, to_bytes, util::AlignedVec};

use crate::protocol::{
    CallMessage, DataMessage, FrameBytes, FrameError, HookTarget, PacketHeader, ProtocolFault,
};

use super::{
    Endpoint, EndpointError, HookKey, Ingress, LocalEvent, ProtocolEndpoint, ProtocolLeaf,
    RouteDecision, RouterLeaf,
};
use super::endpoint::ForwardedFrame;

/// One typed incoming `Call` passed to a leaf procedure.
///
/// This exists so application code can work with a decoded request type plus the protocol context
/// that matters for authorization, routing, or replies.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::{Call, HookKey};
/// let call = Call {
///     input: String::from("hello"),
///     caller_path: vec!["root".into()],
///     procedure_id: "org.example.v1.echo.invoke".into(),
///     dst_leaf: Some("echo".into()),
///     response_hook: Some(HookKey::new(vec!["root".into()], 7)),
/// };
/// assert_eq!(call.input, "hello");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call<T> {
    /// Decoded application input payload.
    pub input: T,
    /// Endpoint path of the caller that opened this call.
    pub caller_path: Vec<String>,
    /// Canonical procedure identifier chosen by the caller.
    pub procedure_id: String,
    /// Optional destination leaf targeted by the call.
    pub dst_leaf: Option<String>,
    /// Hook key declared by the caller when it expects a response.
    pub response_hook: Option<HookKey>,
}

/// One incoming local call event that already passed protocol validation.
///
/// This exists for dispatch layers that still want direct access to the raw protocol payload
/// before converting it into a typed [`Call<T>`].
///
/// # Example
/// ```rust
/// use unshell::protocol::{CallMessage, PacketHeader, PacketType};
/// use unshell::protocol::tree::IncomingCall;
/// let call = IncomingCall {
///     header: PacketHeader {
///         packet_type: PacketType::Call,
///         src_path: vec!["root".into()],
///         dst_path: vec!["worker".into()],
///         dst_leaf: None,
///         hook_id: None,
///     },
///     message: CallMessage {
///         procedure_id: "example.invoke".into(),
///         data: vec![],
///         response_hook: None,
///     },
/// };
/// assert_eq!(call.message.procedure_id, "example.invoke");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingCall {
    /// Validated protocol header for the call.
    pub header: PacketHeader,
    /// Application payload for the call.
    pub message: CallMessage,
}

/// One incoming local data event tied to an active hook.
///
/// This exists so hook-aware leaf code receives both the payload and the resolved hook identity
/// that owns the stream.
///
/// # Example
/// ```rust
/// use unshell::protocol::{DataMessage, PacketHeader, PacketType};
/// use unshell::protocol::tree::{HookKey, IncomingData};
/// let data = IncomingData {
///     header: PacketHeader {
///         packet_type: PacketType::Data,
///         src_path: vec!["worker".into()],
///         dst_path: vec!["root".into()],
///         dst_leaf: None,
///         hook_id: Some(7),
///     },
///     message: DataMessage {
///         procedure_id: "example.invoke".into(),
///         data: vec![1],
///         end_hook: false,
///     },
///     hook_key: HookKey::new(vec!["root".into()], 7),
/// };
/// assert_eq!(data.hook_key.hook_id, 7);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingData {
    /// Validated protocol header for the data packet.
    pub header: PacketHeader,
    /// Hook-associated data payload.
    pub message: DataMessage,
    /// Resolved hook key for the active session.
    pub hook_key: HookKey,
}

/// One incoming local fault event tied to a pending or active hook.
///
/// This exists so leaf code can observe upstream protocol termination and release any
/// application-level resources associated with the hook.
///
/// # Example
/// ```rust
/// use unshell::protocol::{FaultMessage, PacketHeader, PacketType, ProtocolFault};
/// use unshell::protocol::tree::{HookKey, IncomingFault};
/// let fault = IncomingFault {
///     header: PacketHeader {
///         packet_type: PacketType::Fault,
///         src_path: vec!["worker".into()],
///         dst_path: vec!["root".into()],
///         dst_leaf: None,
///         hook_id: Some(7),
///     },
///     fault: FaultMessage { fault: ProtocolFault::INTERNAL_ERROR },
///     hook_key: HookKey::new(vec!["root".into()], 7),
/// };
/// assert_eq!(fault.hook_key.hook_id, 7);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingFault {
    /// Validated protocol header for the fault packet.
    pub header: PacketHeader,
    /// Fault payload emitted by the peer.
    pub fault: crate::protocol::FaultMessage,
    /// Hook key for the pending or active session that faulted.
    pub hook_key: HookKey,
}

/// Outcome of one generated initial call procedure.
///
/// This exists for generated one-shot leaf procedures that either emit one reply payload or
/// intentionally complete without any returned hook traffic.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::CallResult;
/// let reply: CallResult<String> = CallResult::Reply("hello".into());
/// assert!(matches!(reply, CallResult::Reply(_)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallResult<T> {
    /// Return one reply payload to the caller.
    Reply(T),
    /// Complete the call without any response data.
    NoReply,
}

/// One hook-associated `Data` packet emitted by leaf code.
///
/// This exists as the normalized outbound unit produced by leaf code before the runtime turns it
/// into framed protocol traffic.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::OutgoingData;
/// let packet = OutgoingData {
///     dst_path: vec!["root".into()],
///     hook_id: 7,
///     procedure_id: "example.invoke".into(),
///     data: vec![1, 2, 3],
///     end_hook: true,
/// };
/// assert!(packet.end_hook);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingData {
    /// Destination endpoint path for the hook packet.
    pub dst_path: Vec<String>,
    /// Hook identifier scoped to the receiving endpoint.
    pub hook_id: u64,
    /// Procedure identifier that owns this hook stream.
    pub procedure_id: String,
    /// Serialized application data to send.
    pub data: Vec<u8>,
    /// Whether this packet closes the local side of the hook.
    pub end_hook: bool,
}

/// One runtime-normalized reply produced by generated call dispatch.
///
/// This exists because generated call dispatch always normalizes leaf return values into either
/// serialized reply bytes or an explicit “no reply” outcome.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::CallReply;
/// let reply = CallReply::Reply(vec![1, 2, 3]);
/// assert!(matches!(reply, CallReply::Reply(_)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallReply {
    /// Serialized reply bytes that should be returned upstream.
    Reply(Vec<u8>),
    /// Complete without emitting any reply packet.
    NoReply,
}

/// Error surfaced while decoding one incoming call or encoding one generated reply.
///
/// This exists so generated dispatch can keep decode, encode, and handler failures distinct while
/// still using one error channel.
///
/// # Example
/// ```rust
/// use unshell::protocol::{FrameError};
/// use unshell::protocol::tree::DispatchError;
/// let error: DispatchError<core::convert::Infallible> = DispatchError::Decode(FrameError::Truncated);
/// assert!(matches!(error, DispatchError::Decode(_)));
/// ```
#[derive(Debug)]
pub enum DispatchError<E> {
    /// Failed to decode the typed call input.
    Decode(FrameError),
    /// Failed to encode the typed call output.
    Encode(FrameError),
    /// The leaf-specific call handler returned an error.
    Handler(E),
}

impl<E> fmt::Display for DispatchError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(error) => write!(f, "call decode failed: {error}"),
            Self::Encode(error) => write!(f, "call reply encode failed: {error}"),
            Self::Handler(error) => write!(f, "call handler failed: {error}"),
        }
    }
}

impl<E> core::error::Error for DispatchError<E> where E: core::error::Error + 'static {}

/// Error surfaced by the stateful leaf runtime.
///
/// This exists so callers can distinguish transport/runtime failures from leaf-local business
/// logic failures.
///
/// # Example
/// ```rust
/// use unshell::protocol::{FrameError};
/// use unshell::protocol::tree::{DispatchError, LeafRuntimeError};
/// let error: LeafRuntimeError<core::convert::Infallible> = LeafRuntimeError::Dispatch(DispatchError::Decode(FrameError::Truncated));
/// assert!(matches!(error, LeafRuntimeError::Dispatch(_)));
/// ```
#[derive(Debug)]
pub enum LeafRuntimeError<E> {
    /// Protocol endpoint routing or framing failed.
    Endpoint(EndpointError),
    /// Typed call dispatch failed.
    Dispatch(DispatchError<E>),
    /// Leaf-local data or fault handling failed.
    Leaf(E),
}

impl<E> fmt::Display for LeafRuntimeError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Endpoint(error) => write!(f, "{error}"),
            Self::Dispatch(error) => write!(f, "{error}"),
            Self::Leaf(error) => write!(f, "{error}"),
        }
    }
}

impl<E> core::error::Error for LeafRuntimeError<E> where E: core::error::Error + 'static {}

impl<E> From<EndpointError> for LeafRuntimeError<E> {
    fn from(value: EndpointError) -> Self {
        Self::Endpoint(value)
    }
}

/// High-level leaf behavior layered on top of validated protocol events.
///
/// This exists for leaves that want validated call/data/fault delivery without managing endpoint
/// routing details themselves.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::CallLeaf;
/// struct ExampleLeaf;
/// impl unshell::protocol::tree::ProtocolLeaf for ExampleLeaf {
///     fn leaf_name() -> String { "org.example.v1.echo".into() }
/// }
/// impl CallLeaf for ExampleLeaf {
///     type Error = core::convert::Infallible;
/// }
/// ```
pub trait CallLeaf: ProtocolLeaf {
    /// Leaf-specific error surfaced by call, data, or fault handling.
    type Error;

    /// Handles hook-associated inbound `Data` after protocol validation.
    fn on_data(&mut self, _data: IncomingData) -> Result<Vec<OutgoingData>, Self::Error> {
        Ok(Vec::new())
    }

    /// Observes one inbound `Fault` after protocol validation.
    fn on_fault(&mut self, _fault: IncomingFault) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Polls the leaf for locally-generated hook traffic.
    fn poll(&mut self) -> Result<Vec<OutgoingData>, Self::Error> {
        Ok(Vec::new())
    }
}

/// Stateful runtime that combines a protocol endpoint with one leaf instance.
///
/// This exists as the high-level runtime for simple one-shot call procedures plus hook data/fault
/// handling.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::LeafRuntime;
/// # struct Leaf;
/// # let _ = core::marker::PhantomData::<LeafRuntime<Leaf>>;
/// ```
#[derive(Debug)]
pub struct LeafRuntime<L> {
    endpoint: ProtocolEndpoint,
    leaf: L,
}

/// Frames emitted by the runtime after one receive or poll step.
///
/// This exists so callers can flush emitted frames to transport while also learning whether the
/// inbound packet was intentionally dropped.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::RuntimeOutcome;
/// let outcome = RuntimeOutcome::default();
/// assert!(outcome.frames.is_empty());
/// ```
#[derive(Debug, Default)]
pub struct RuntimeOutcome {
    /// Frames emitted while processing the step.
    pub frames: Vec<FrameBytes>,
    /// Whether the endpoint dropped the incoming packet.
    pub dropped: bool,
}

/// Frames emitted by the runtime together with their chosen next hops.
///
/// What it is: the router-oriented variant of [`RuntimeOutcome`], preserving the
/// `RouteDecision` for every emitted frame.
///
/// Why it exists: transport-owning leaves need to know whether each frame should
/// go to the parent or to a specific child, not just the encoded bytes.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::RoutedRuntimeOutcome;
/// let outcome = RoutedRuntimeOutcome::default();
/// assert!(outcome.forwarded.is_empty());
/// ```
#[derive(Debug, Default, Clone)]
pub struct RoutedRuntimeOutcome {
    /// Forwarded frames paired with the route chosen by the endpoint runtime.
    pub forwarded: Vec<ForwardedFrame>,
    /// Whether the endpoint dropped the incoming packet.
    pub dropped: bool,
}

impl<L> LeafRuntime<L> {
    /// Builds a runtime from one endpoint and one leaf instance.
    #[must_use]
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{LeafRuntime, ProtocolEndpoint};
    /// struct ExampleLeaf;
    /// let runtime = LeafRuntime::new(ProtocolEndpoint::new(Vec::new(), None, Vec::new(), Vec::new()), ExampleLeaf);
    /// let _ = runtime;
    /// ```
    pub fn new(endpoint: ProtocolEndpoint, leaf: L) -> Self {
        Self { endpoint, leaf }
    }

    /// Returns the underlying protocol endpoint.
    #[must_use]
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{LeafRuntime, ProtocolEndpoint};
    /// struct ExampleLeaf;
    /// let runtime = LeafRuntime::new(ProtocolEndpoint::new(Vec::new(), None, Vec::new(), Vec::new()), ExampleLeaf);
    /// let _endpoint = runtime.endpoint();
    /// ```
    pub fn endpoint(&self) -> &ProtocolEndpoint {
        &self.endpoint
    }

    /// Returns a mutable reference to the underlying endpoint.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{LeafRuntime, ProtocolEndpoint};
    /// struct ExampleLeaf;
    /// let mut runtime = LeafRuntime::new(ProtocolEndpoint::new(Vec::new(), None, Vec::new(), Vec::new()), ExampleLeaf);
    /// let _endpoint = runtime.endpoint_mut();
    /// ```
    pub fn endpoint_mut(&mut self) -> &mut ProtocolEndpoint {
        &mut self.endpoint
    }

    /// Returns the hosted leaf instance.
    #[must_use]
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{LeafRuntime, ProtocolEndpoint};
    /// struct ExampleLeaf;
    /// let runtime = LeafRuntime::new(ProtocolEndpoint::new(Vec::new(), None, Vec::new(), Vec::new()), ExampleLeaf);
    /// let _leaf = runtime.leaf();
    /// ```
    pub fn leaf(&self) -> &L {
        &self.leaf
    }

    /// Returns a mutable reference to the hosted leaf instance.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{LeafRuntime, ProtocolEndpoint};
    /// struct ExampleLeaf;
    /// let mut runtime = LeafRuntime::new(ProtocolEndpoint::new(Vec::new(), None, Vec::new(), Vec::new()), ExampleLeaf);
    /// let _leaf = runtime.leaf_mut();
    /// ```
    pub fn leaf_mut(&mut self) -> &mut L {
        &mut self.leaf
    }
}

impl<L> LeafRuntime<L>
where
    L: CallLeaf + super::CallProcedures<Error = <L as CallLeaf>::Error>,
{
    /// Delivers one inbound frame into the stateful leaf runtime.
    ///
    /// # Example
    /// ```rust
    /// # use unshell::protocol::tree::{LeafRuntime, ProtocolEndpoint};
    /// # struct ExampleLeaf;
    /// # let _ = core::marker::PhantomData::<LeafRuntime<ExampleLeaf>>;
    /// ```
    pub fn receive(
        &mut self,
        ingress: &Ingress,
        frame: FrameBytes,
    ) -> Result<RuntimeOutcome, LeafRuntimeError<<L as CallLeaf>::Error>> {
        let routed = self.receive_routed(ingress, frame)?;
        Ok(RuntimeOutcome {
            frames: routed
                .forwarded
                .into_iter()
                .map(|forwarded| forwarded.frame)
                .collect(),
            dropped: routed.dropped,
        })
    }

    /// Delivers one inbound frame while preserving route decisions for emitted traffic.
    ///
    /// # Example
    /// ```rust
    /// # use unshell::protocol::tree::{LeafRuntime, ProtocolEndpoint};
    /// # struct ExampleLeaf;
    /// # let _ = core::marker::PhantomData::<LeafRuntime<ExampleLeaf>>;
    /// ```
    pub fn receive_routed(
        &mut self,
        ingress: &Ingress,
        frame: FrameBytes,
    ) -> Result<RoutedRuntimeOutcome, LeafRuntimeError<<L as CallLeaf>::Error>> {
        let outcome = self.endpoint.receive(ingress, frame)?;
        self.process_endpoint_outcome_routed(outcome)
    }

    /// Polls the leaf for locally-generated hook traffic and routes any emitted frames.
    ///
    /// # Example
    /// ```rust
    /// # use unshell::protocol::tree::{LeafRuntime, ProtocolEndpoint};
    /// # struct ExampleLeaf;
    /// # let _ = core::marker::PhantomData::<LeafRuntime<ExampleLeaf>>;
    /// ```
    pub fn poll(&mut self) -> Result<RuntimeOutcome, LeafRuntimeError<<L as CallLeaf>::Error>> {
        let routed = self.poll_routed()?;
        Ok(RuntimeOutcome {
            frames: routed
                .forwarded
                .into_iter()
                .map(|forwarded| forwarded.frame)
                .collect(),
            dropped: routed.dropped,
        })
    }

    /// Polls the leaf while preserving route decisions for emitted traffic.
    ///
    /// # Example
    /// ```rust
    /// # use unshell::protocol::tree::{LeafRuntime, ProtocolEndpoint};
    /// # struct ExampleLeaf;
    /// # let _ = core::marker::PhantomData::<LeafRuntime<ExampleLeaf>>;
    /// ```
    pub fn poll_routed(
        &mut self,
    ) -> Result<RoutedRuntimeOutcome, LeafRuntimeError<<L as CallLeaf>::Error>> {
        let outgoing = self.leaf.poll().map_err(LeafRuntimeError::Leaf)?;
        self.emit_outgoing_routed(outgoing)
    }

    fn process_endpoint_outcome_routed(
        &mut self,
        outcome: crate::protocol::tree::EndpointOutcome,
    ) -> Result<RoutedRuntimeOutcome, LeafRuntimeError<<L as CallLeaf>::Error>> {
        match outcome {
            crate::protocol::tree::EndpointOutcome::Forward { route, frame } => {
                Ok(RoutedRuntimeOutcome {
                    forwarded: vec![ForwardedFrame { route, frame }],
                    dropped: false,
                })
            }
            crate::protocol::tree::EndpointOutcome::Dropped => Ok(RoutedRuntimeOutcome {
                forwarded: Vec::new(),
                dropped: true,
            }),
            crate::protocol::tree::EndpointOutcome::Local(event) => self.process_local_event(event),
        }
    }

    fn process_local_event(
        &mut self,
        event: LocalEvent,
    ) -> Result<RoutedRuntimeOutcome, LeafRuntimeError<<L as CallLeaf>::Error>> {
        match event {
            LocalEvent::Call { header, message } => self.process_local_call(header, message),
            LocalEvent::Data {
                header,
                message,
                hook_key,
            } => self.process_local_data(header, message, hook_key),
            LocalEvent::Fault {
                header,
                message,
                hook_key,
            } => self.process_local_fault(header, message, hook_key),
        }
    }

    fn process_local_call(
        &mut self,
        header: PacketHeader,
        message: CallMessage,
    ) -> Result<RoutedRuntimeOutcome, LeafRuntimeError<<L as CallLeaf>::Error>> {
        let CallMessage {
            procedure_id,
            data,
            response_hook,
        } = message;
        let fault_hook = response_hook.as_ref();
        let incoming = IncomingCall {
            header,
            // Split the payload apart so the reply path can reuse the owned procedure id and
            // response hook without re-decoding the incoming bytes.
            message: CallMessage {
                procedure_id: procedure_id.clone(),
                data,
                response_hook: response_hook.clone(),
            },
        };

        match self.leaf.dispatch_call(&mut self.endpoint, incoming) {
            Ok(CallReply::Reply(bytes)) => {
                let frames = if let Some(hook) = response_hook {
                    self.send_reply_data(hook, procedure_id, bytes, true)?
                } else {
                    RoutedRuntimeOutcome::default()
                };
                Ok(frames)
            }
            Ok(CallReply::NoReply) => Ok(RoutedRuntimeOutcome::default()),
            Err(error) => {
                // Dispatch failures still emit a protocol fault for the remote caller when a
                // response hook exists, even though the local runtime also surfaces the error.
                let _ = self.emit_internal_fault_if_possible(fault_hook)?;
                Err(LeafRuntimeError::Dispatch(error))
            }
        }
    }

    fn process_local_data(
        &mut self,
        header: PacketHeader,
        message: DataMessage,
        hook_key: HookKey,
    ) -> Result<RoutedRuntimeOutcome, LeafRuntimeError<<L as CallLeaf>::Error>> {
        let outgoing = self
            .leaf
            .on_data(IncomingData {
                header,
                message,
                hook_key,
            })
            .map_err(LeafRuntimeError::Leaf)?;
        self.emit_outgoing_routed(outgoing)
    }

    fn process_local_fault(
        &mut self,
        header: PacketHeader,
        message: crate::protocol::FaultMessage,
        hook_key: HookKey,
    ) -> Result<RoutedRuntimeOutcome, LeafRuntimeError<<L as CallLeaf>::Error>> {
        self.leaf
            .on_fault(IncomingFault {
                header,
                fault: message,
                hook_key,
            })
            .map_err(LeafRuntimeError::Leaf)?;
        Ok(RoutedRuntimeOutcome::default())
    }

    fn emit_outgoing_routed(
        &mut self,
        outgoing: Vec<OutgoingData>,
    ) -> Result<RoutedRuntimeOutcome, LeafRuntimeError<<L as CallLeaf>::Error>> {
        let mut runtime = RoutedRuntimeOutcome::default();
        for packet in outgoing {
            let endpoint_outcome = self.endpoint.send_data(
                packet.dst_path,
                packet.hook_id,
                packet.procedure_id,
                packet.data,
                packet.end_hook,
            )?;
            runtime
                .forwarded
                .extend(self.process_endpoint_outcome_routed(endpoint_outcome)?.forwarded);
        }
        Ok(runtime)
    }

    fn send_reply_data(
        &mut self,
        hook: HookTarget,
        procedure_id: String,
        bytes: Vec<u8>,
        end_hook: bool,
    ) -> Result<RoutedRuntimeOutcome, LeafRuntimeError<<L as CallLeaf>::Error>> {
        let endpoint_outcome = self.endpoint.send_data(
            hook.return_path,
            hook.hook_id,
            procedure_id,
            bytes,
            end_hook,
        )?;
        self.process_endpoint_outcome_routed(endpoint_outcome)
    }

    fn emit_internal_fault_if_possible(
        &mut self,
        hook: Option<&HookTarget>,
    ) -> Result<RoutedRuntimeOutcome, LeafRuntimeError<<L as CallLeaf>::Error>> {
        let Some(hook) = hook else {
            return Ok(RoutedRuntimeOutcome::default());
        };
        let key = HookKey::new(hook.return_path.clone(), hook.hook_id);
        let outcome = self
            .endpoint
            .emit_fault_if_possible(Some(key), ProtocolFault::INTERNAL_ERROR)?;
        self.process_endpoint_outcome_routed(outcome)
    }
}

impl<L> LeafRuntime<L>
where
    L: CallLeaf + super::CallProcedures<Error = <L as CallLeaf>::Error> + RouterLeaf,
{
    /// Sends previously forwarded frames through the router leaf's parent/child links.
    ///
    /// What it is: a small transport bridge from endpoint route decisions to the
    /// leaf-owned connections.
    ///
    /// Why it exists: router leaves should be able to reuse the normal protocol
    /// runtime and still own the concrete forwarding mechanism.
    ///
    /// # Example
    /// ```rust
    /// # use unshell::protocol::tree::{LeafRuntime, ProtocolEndpoint};
    /// # struct ExampleLeaf;
    /// # let _ = core::marker::PhantomData::<LeafRuntime<ExampleLeaf>>;
    /// ```
    pub fn route_forwarded(
        &mut self,
        forwarded: Vec<ForwardedFrame>,
    ) -> Result<(), <L as RouterLeaf>::RouteError> {
        for forwarded in forwarded {
            match forwarded.route {
                RouteDecision::Parent => {
                    self.leaf
                        .route_to_parent(self.endpoint.path(), forwarded.frame)?;
                }
                RouteDecision::Child(index) => {
                    let child_path = self
                        .endpoint
                        .child_routes()
                        .get(index)
                        .map(|child| child.path.clone())
                        .unwrap_or_default();
                    self.leaf.route_to_child(&child_path, forwarded.frame)?;
                }
                RouteDecision::Local | RouteDecision::Drop => {}
            }
        }
        Ok(())
    }
}

/// Decodes one archived call payload into a typed application request.
///
/// This exists for generated and manual leaf code that stores its own typed `rkyv` payload inside
/// protocol `CallMessage::data` bytes.
///
/// # Example
/// ```rust
/// use rkyv::{Archive, Deserialize, Serialize};
/// use unshell::protocol::tree::{decode_call_input, encode_call_reply};
/// #[derive(Archive, Serialize, Deserialize, Debug, PartialEq)]
/// struct Example { value: u32 }
/// let bytes = encode_call_reply(&Example { value: 7 })?;
/// let decoded = decode_call_input::<Example>(&bytes)?;
/// assert_eq!(decoded, Example { value: 7 });
/// # Ok::<(), unshell::protocol::FrameError>(())
/// ```
pub fn decode_call_input<T>(bytes: &[u8]) -> Result<T, FrameError>
where
    T: Archive,
    <T as Archive>::Archived: rkyv::Portable
        + for<'b> rkyv::bytecheck::CheckBytes<rkyv::api::high::HighValidator<'b, Error>>
        + rkyv::Deserialize<T, rkyv::api::high::HighDeserializer<Error>>,
{
    crate::protocol::deserialize_archived_bytes::<<T as Archive>::Archived, T>(bytes)
}

/// Encodes one typed application reply into hook `Data` bytes.
///
/// This exists for generated and manual leaf code that wants to place one typed `rkyv` payload in
/// the `data` field of a returned hook packet.
///
/// # Example
/// ```rust
/// use rkyv::{Archive, Deserialize, Serialize};
/// use unshell::protocol::tree::encode_call_reply;
/// #[derive(Archive, Serialize, Deserialize, Debug, PartialEq)]
/// struct Example { value: u32 }
/// let bytes = encode_call_reply(&Example { value: 7 })?;
/// assert!(!bytes.is_empty());
/// # Ok::<(), unshell::protocol::FrameError>(())
/// ```
pub fn encode_call_reply<T>(value: &T) -> Result<Vec<u8>, FrameError>
where
    T: for<'a> Serialize<
        rkyv::api::high::HighSerializer<AlignedVec, rkyv::ser::allocator::ArenaHandle<'a>, Error>,
    >,
{
    let bytes = to_bytes::<Error>(value).map_err(FrameError::Serialize)?;
    Ok(bytes.as_slice().to_vec())
}
