//! Stateful application-layer call runtime built on top of `ProtocolEndpoint`.

use alloc::{string::String, vec, vec::Vec};
use core::fmt;

use rkyv::{Archive, Serialize, rancor::Error, to_bytes, util::AlignedVec};

use crate::protocol::{
    CallMessage, DataMessage, FrameBytes, FrameError, HookTarget, PacketHeader, ProtocolFault,
};

use super::{
    Endpoint, EndpointError, HookKey, Ingress, LocalEvent, ProtocolEndpoint, ProtocolLeaf,
};

/// One typed incoming `Call` passed to a leaf procedure.
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingCall {
    /// Validated protocol header for the call.
    pub header: PacketHeader,
    /// Application payload for the call.
    pub message: CallMessage,
}

/// One incoming local data event tied to an active hook.
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallResult<T> {
    /// Return one reply payload to the caller.
    Reply(T),
    /// Complete the call without any response data.
    NoReply,
}

/// One hook-associated `Data` packet emitted by leaf code.
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallReply {
    /// Serialized reply bytes that should be returned upstream.
    Reply(Vec<u8>),
    /// Complete without emitting any reply packet.
    NoReply,
}

/// Error surfaced while decoding one incoming call or encoding one generated reply.
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
#[derive(Debug)]
pub struct LeafRuntime<L> {
    endpoint: ProtocolEndpoint,
    leaf: L,
}

/// Frames emitted by the runtime after one receive or poll step.
#[derive(Debug, Default)]
pub struct RuntimeOutcome {
    /// Frames emitted while processing the step.
    pub frames: Vec<FrameBytes>,
    /// Whether the endpoint dropped the incoming packet.
    pub dropped: bool,
}

impl<L> LeafRuntime<L> {
    /// Builds a runtime from one endpoint and one leaf instance.
    #[must_use]
    pub fn new(endpoint: ProtocolEndpoint, leaf: L) -> Self {
        Self { endpoint, leaf }
    }

    /// Returns the underlying protocol endpoint.
    #[must_use]
    pub fn endpoint(&self) -> &ProtocolEndpoint {
        &self.endpoint
    }

    /// Returns a mutable reference to the underlying endpoint.
    pub fn endpoint_mut(&mut self) -> &mut ProtocolEndpoint {
        &mut self.endpoint
    }

    /// Returns the hosted leaf instance.
    #[must_use]
    pub fn leaf(&self) -> &L {
        &self.leaf
    }

    /// Returns a mutable reference to the hosted leaf instance.
    pub fn leaf_mut(&mut self) -> &mut L {
        &mut self.leaf
    }
}

impl<L> LeafRuntime<L>
where
    L: CallLeaf + super::CallProcedures<Error = <L as CallLeaf>::Error>,
{
    /// Delivers one inbound frame into the stateful leaf runtime.
    pub fn receive(
        &mut self,
        ingress: &Ingress,
        frame: FrameBytes,
    ) -> Result<RuntimeOutcome, LeafRuntimeError<<L as CallLeaf>::Error>> {
        let outcome = self.endpoint.receive(ingress, frame)?;
        self.process_endpoint_outcome(outcome)
    }

    /// Polls the leaf for locally-generated hook traffic and routes any emitted frames.
    pub fn poll(&mut self) -> Result<RuntimeOutcome, LeafRuntimeError<<L as CallLeaf>::Error>> {
        let outgoing = self.leaf.poll().map_err(LeafRuntimeError::Leaf)?;
        self.emit_outgoing(outgoing)
    }

    fn process_endpoint_outcome(
        &mut self,
        outcome: crate::protocol::tree::EndpointOutcome,
    ) -> Result<RuntimeOutcome, LeafRuntimeError<<L as CallLeaf>::Error>> {
        match outcome {
            crate::protocol::tree::EndpointOutcome::Forward { frame, .. } => Ok(RuntimeOutcome {
                frames: vec![frame],
                dropped: false,
            }),
            crate::protocol::tree::EndpointOutcome::Dropped => Ok(RuntimeOutcome {
                frames: Vec::new(),
                dropped: true,
            }),
            crate::protocol::tree::EndpointOutcome::Local(event) => self.process_local_event(event),
        }
    }

    fn process_local_event(
        &mut self,
        event: LocalEvent,
    ) -> Result<RuntimeOutcome, LeafRuntimeError<<L as CallLeaf>::Error>> {
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
    ) -> Result<RuntimeOutcome, LeafRuntimeError<<L as CallLeaf>::Error>> {
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

        match self.leaf.dispatch_call(incoming) {
            Ok(CallReply::Reply(bytes)) => {
                let frames = if let Some(hook) = response_hook {
                    self.send_reply_data(hook, procedure_id, bytes, true)?
                } else {
                    Vec::new()
                };
                Ok(RuntimeOutcome {
                    frames,
                    dropped: false,
                })
            }
            Ok(CallReply::NoReply) => Ok(RuntimeOutcome::default()),
            Err(error) => {
                let frames = self.emit_internal_fault_if_possible(fault_hook)?;
                let _ = frames;
                Err(LeafRuntimeError::Dispatch(error))
            }
        }
    }

    fn process_local_data(
        &mut self,
        header: PacketHeader,
        message: DataMessage,
        hook_key: HookKey,
    ) -> Result<RuntimeOutcome, LeafRuntimeError<<L as CallLeaf>::Error>> {
        let outgoing = self
            .leaf
            .on_data(IncomingData {
                header,
                message,
                hook_key,
            })
            .map_err(LeafRuntimeError::Leaf)?;
        self.emit_outgoing(outgoing)
    }

    fn process_local_fault(
        &mut self,
        header: PacketHeader,
        message: crate::protocol::FaultMessage,
        hook_key: HookKey,
    ) -> Result<RuntimeOutcome, LeafRuntimeError<<L as CallLeaf>::Error>> {
        self.leaf
            .on_fault(IncomingFault {
                header,
                fault: message,
                hook_key,
            })
            .map_err(LeafRuntimeError::Leaf)?;
        Ok(RuntimeOutcome::default())
    }

    fn emit_outgoing(
        &mut self,
        outgoing: Vec<OutgoingData>,
    ) -> Result<RuntimeOutcome, LeafRuntimeError<<L as CallLeaf>::Error>> {
        let mut runtime = RuntimeOutcome::default();
        for packet in outgoing {
            let endpoint_outcome = self.endpoint.send_data(
                packet.dst_path,
                packet.hook_id,
                packet.procedure_id,
                packet.data,
                packet.end_hook,
            )?;
            runtime
                .frames
                .extend(self.process_endpoint_outcome(endpoint_outcome)?.frames);
        }
        Ok(runtime)
    }

    fn send_reply_data(
        &mut self,
        hook: HookTarget,
        procedure_id: String,
        bytes: Vec<u8>,
        end_hook: bool,
    ) -> Result<Vec<FrameBytes>, LeafRuntimeError<<L as CallLeaf>::Error>> {
        let endpoint_outcome = self.endpoint.send_data(
            hook.return_path,
            hook.hook_id,
            procedure_id,
            bytes,
            end_hook,
        )?;
        Ok(self.process_endpoint_outcome(endpoint_outcome)?.frames)
    }

    fn emit_internal_fault_if_possible(
        &mut self,
        hook: Option<&HookTarget>,
    ) -> Result<Vec<FrameBytes>, LeafRuntimeError<<L as CallLeaf>::Error>> {
        let Some(hook) = hook else {
            return Ok(Vec::new());
        };
        let key = HookKey::new(hook.return_path.clone(), hook.hook_id);
        let outcome = self
            .endpoint
            .emit_fault_if_possible(Some(key), ProtocolFault::INTERNAL_ERROR)?;
        Ok(self.process_endpoint_outcome(outcome)?.frames)
    }
}

/// Decodes one archived call payload into a typed application request.
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
pub fn encode_call_reply<T>(value: &T) -> Result<Vec<u8>, FrameError>
where
    T: for<'a> Serialize<
        rkyv::api::high::HighSerializer<AlignedVec, rkyv::ser::allocator::ArenaHandle<'a>, Error>,
    >,
{
    let bytes = to_bytes::<Error>(value).map_err(FrameError::Serialize)?;
    Ok(bytes.as_slice().to_vec())
}
