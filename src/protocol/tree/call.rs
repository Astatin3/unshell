//! Stateful application-layer call runtime built on top of `ProtocolEndpoint`.

use alloc::{string::String, vec::Vec};
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
    pub input: T,
    pub caller_path: Vec<String>,
    pub procedure_id: String,
    pub dst_leaf: Option<String>,
    pub response_hook: Option<HookKey>,
}

/// One incoming local call event that already passed protocol validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingCall {
    pub header: PacketHeader,
    pub message: CallMessage,
}

/// One incoming local data event tied to an active hook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingData {
    pub header: PacketHeader,
    pub message: DataMessage,
    pub hook_key: HookKey,
}

/// One incoming local fault event tied to a pending or active hook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingFault {
    pub header: PacketHeader,
    pub fault: crate::protocol::FaultMessage,
    pub hook_key: HookKey,
}

/// Outcome of one generated initial call procedure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallResult<T> {
    Reply(T),
    NoReply,
}

/// One hook-associated `Data` packet emitted by leaf code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingData {
    pub dst_path: Vec<String>,
    pub hook_id: u64,
    pub procedure_id: String,
    pub data: Vec<u8>,
    pub end_hook: bool,
}

/// One runtime-normalized reply produced by generated call dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallReply {
    Reply(Vec<u8>),
    NoReply,
}

/// Error surfaced while decoding one incoming call or encoding one generated reply.
#[derive(Debug)]
pub enum DispatchError<E> {
    Decode(FrameError),
    Encode(FrameError),
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
    Endpoint(EndpointError),
    Dispatch(DispatchError<E>),
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
    pub frames: Vec<FrameBytes>,
    pub dropped: bool,
}

impl<L> LeafRuntime<L> {
    #[must_use]
    pub fn new(endpoint: ProtocolEndpoint, leaf: L) -> Self {
        Self { endpoint, leaf }
    }

    #[must_use]
    pub fn endpoint(&self) -> &ProtocolEndpoint {
        &self.endpoint
    }

    pub fn endpoint_mut(&mut self) -> &mut ProtocolEndpoint {
        &mut self.endpoint
    }

    #[must_use]
    pub fn leaf(&self) -> &L {
        &self.leaf
    }

    pub fn leaf_mut(&mut self) -> &mut L {
        &mut self.leaf
    }
}

impl<L> LeafRuntime<L>
where
    L: CallLeaf + super::CallProcedures<Error = <L as CallLeaf>::Error>,
{
    pub fn receive(
        &mut self,
        ingress: &Ingress,
        frame: FrameBytes,
    ) -> Result<RuntimeOutcome, LeafRuntimeError<<L as CallLeaf>::Error>> {
        let outcome = self.endpoint.receive(ingress, frame)?;
        self.process_endpoint_outcome(outcome)
    }

    pub fn poll(&mut self) -> Result<RuntimeOutcome, LeafRuntimeError<<L as CallLeaf>::Error>> {
        let outgoing = self.leaf.poll().map_err(LeafRuntimeError::Leaf)?;
        self.emit_outgoing(outgoing)
    }

    fn process_endpoint_outcome(
        &mut self,
        outcome: crate::protocol::tree::EndpointOutcome,
    ) -> Result<RuntimeOutcome, LeafRuntimeError<<L as CallLeaf>::Error>> {
        let mut runtime = RuntimeOutcome {
            frames: Vec::new(),
            dropped: outcome.dropped,
        };

        if let Some((_route, frame)) = outcome.forward {
            runtime.frames.push(frame);
        }

        let Some(event) = outcome.event else {
            return Ok(runtime);
        };

        match event {
            LocalEvent::Call { header, message } => {
                let incoming = IncomingCall {
                    header,
                    message: message.clone(),
                };
                match self.leaf.dispatch_call(incoming) {
                    Ok(CallReply::Reply(bytes)) => {
                        if let Some(hook) = message.response_hook {
                            runtime.frames.extend(self.send_reply_data(
                                hook,
                                message.procedure_id,
                                bytes,
                                true,
                            )?);
                        }
                    }
                    Ok(CallReply::NoReply) => {}
                    Err(error) => {
                        runtime
                            .frames
                            .extend(self.emit_internal_fault_if_possible(&message)?);
                        return Err(LeafRuntimeError::Dispatch(error));
                    }
                }
            }
            LocalEvent::Data {
                header,
                message,
                hook_key,
            } => {
                let outgoing = self
                    .leaf
                    .on_data(IncomingData {
                        header,
                        message,
                        hook_key,
                    })
                    .map_err(LeafRuntimeError::Leaf)?;
                runtime.frames.extend(self.emit_outgoing(outgoing)?.frames);
            }
            LocalEvent::Fault {
                header,
                message,
                hook_key,
            } => {
                self.leaf
                    .on_fault(IncomingFault {
                        header,
                        fault: message,
                        hook_key,
                    })
                    .map_err(LeafRuntimeError::Leaf)?;
            }
        }

        Ok(runtime)
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
        message: &CallMessage,
    ) -> Result<Vec<FrameBytes>, LeafRuntimeError<<L as CallLeaf>::Error>> {
        let Some(hook) = message.response_hook.as_ref() else {
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
