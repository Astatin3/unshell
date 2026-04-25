//! Procedure-scoped session runtime for complex hook-backed leaves.
//!
//! This layer exists for procedures that need long-lived per-hook state, such as
//! a remote shell. The leaf owns the session table explicitly, while the runtime
//! handles the protocol bookkeeping around initial `Call`, follow-on `Data`, and
//! upstream `Fault` traffic.
//!
//! # Model
//!
//! - One opening `Call` targets one procedure suffix such as `open`.
//! - If that procedure succeeds, it returns one session value.
//! - The runtime stores that session under the hook key declared by the caller.
//! - Later hook traffic is routed back to that same session automatically.
//!
//! The protocol still owns transport truth such as half-close state and fault
//! routing. Procedure sessions only own application resources and behavior.

use alloc::{collections::BTreeMap, string::String, vec::Vec};
use core::{fmt, marker::PhantomData};

use rkyv::{Archive, rancor::Error};

use crate::protocol::{CallMessage, FrameBytes, HookTarget, ProtocolFault};

use super::{
    DispatchError, Endpoint, EndpointError, HookKey, IncomingCall, IncomingData, IncomingFault,
    Ingress, LocalEvent, OutgoingData, ProtocolEndpoint, ProtocolLeaf, decode_call_input,
};

/// Generated metadata for one stateful procedure bound to one leaf type.
///
/// This metadata is intentionally tiny: one procedure suffix plus the derived
/// full `procedure_id`. The leaf still owns all session storage explicitly.
pub trait StatefulProcedureMetadata<L>: Sized
where
    L: ProtocolLeaf,
{
    /// Returns the local suffix used to derive the full canonical `procedure_id`.
    fn procedure_suffix() -> &'static str;

    /// Returns the canonical `procedure_id` for this procedure.
    fn procedure_id() -> String {
        let mut procedure_id = L::leaf_name();
        procedure_id.push('.');
        procedure_id.push_str(Self::procedure_suffix());
        procedure_id
    }
}

/// Explicit storage access for one procedure session map inside the leaf.
///
/// Rationale: the leaf remains the source of truth for its active sessions. This
/// avoids hidden generated enums or side tables and keeps debugging obvious.
pub trait ProcedureStore<P> {
    /// Returns the hook-keyed session table for one procedure type.
    fn procedure_sessions(&mut self) -> &mut BTreeMap<HookKey, P>;
}

/// One procedure that owns per-hook session state.
///
/// The opening `Call` constructs one session value. The runtime then hands later
/// `Data`, `Fault`, and `poll()` ticks back to that stored session until the
/// session requests removal or the protocol faults it out.
///
/// # Example
/// ```rust
/// use alloc::collections::BTreeMap;
/// use alloc::string::String;
/// use unshell::{Leaf, Procedure};
/// use unshell::protocol::tree::{Call, HookKey, Procedure, ProcedureEffect, ProcedureStore};
///
/// #[derive(Default, Leaf)]
/// #[leaf(id = "org.example.v1.stream")]
/// struct StreamLeaf {
///     sessions: BTreeMap<HookKey, OpenProcedure>,
/// }
///
/// impl ProcedureStore<OpenProcedure> for StreamLeaf {
///     fn procedure_sessions(&mut self) -> &mut BTreeMap<HookKey, OpenProcedure> {
///         &mut self.sessions
///     }
/// }
///
/// #[derive(Procedure)]
/// #[procedure(leaf = StreamLeaf, name = "open")]
/// struct OpenProcedure {
///     prefix: String,
/// }
///
/// impl Procedure<StreamLeaf> for OpenProcedure {
///     type Error = core::convert::Infallible;
///     type Input = String;
///
///     fn open(
///         _leaf: &mut StreamLeaf,
///         call: Call<Self::Input>,
///     ) -> Result<Self, Self::Error> {
///         Ok(Self { prefix: call.input })
///     }
///
///     fn poll(
///         _leaf: &mut StreamLeaf,
///         _session: &mut Self,
///     ) -> Result<ProcedureEffect, Self::Error> {
///         Ok(ProcedureEffect::default())
///     }
/// }
/// ```
pub trait Procedure<L>: StatefulProcedureMetadata<L> + Sized
where
    L: ProtocolLeaf,
{
    type Error;
    type Input;

    /// Creates one session from the opening `Call`.
    fn open(leaf: &mut L, call: super::Call<Self::Input>) -> Result<Self, Self::Error>;

    /// Handles one inbound hook `Data` packet for this procedure.
    fn on_data(
        _leaf: &mut L,
        _session: &mut Self,
        _data: IncomingData,
    ) -> Result<ProcedureEffect, Self::Error> {
        Ok(ProcedureEffect::default())
    }

    /// Handles one inbound hook `Fault` packet for this procedure.
    fn on_fault(
        _leaf: &mut L,
        _session: &mut Self,
        _fault: IncomingFault,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Polls one live session for locally-generated hook traffic.
    fn poll(_leaf: &mut L, _session: &mut Self) -> Result<ProcedureEffect, Self::Error> {
        Ok(ProcedureEffect::default())
    }

    /// Releases application resources when the runtime discards one session.
    ///
    /// This hook exists because a runtime error may force the session to be
    /// dropped before the normal protocol close path completes. Simple state
    /// objects can keep the default no-op implementation.
    fn close(_leaf: &mut L, _session: Self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Output produced while advancing one session.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ProcedureEffect {
    /// `Data` packets to emit after the session step completes.
    pub outgoing: Vec<OutgoingData>,
    /// Whether the runtime should remove the session after sending `outgoing`.
    pub close_session: bool,
}

impl ProcedureEffect {
    #[must_use]
    pub fn outgoing(outgoing: Vec<OutgoingData>) -> Self {
        Self {
            outgoing,
            close_session: false,
        }
    }

    #[must_use]
    pub fn close(outgoing: Vec<OutgoingData>) -> Self {
        Self {
            outgoing,
            close_session: true,
        }
    }
}

/// Error surfaced by the procedure runtime.
#[derive(Debug)]
pub enum ProcedureRuntimeError<E> {
    Endpoint(EndpointError),
    Decode(super::DispatchError<E>),
}

impl<E> fmt::Display for ProcedureRuntimeError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Endpoint(error) => write!(f, "{error}"),
            Self::Decode(error) => write!(f, "{error}"),
        }
    }
}

impl<E> core::error::Error for ProcedureRuntimeError<E> where E: core::error::Error + 'static {}

impl<E> From<EndpointError> for ProcedureRuntimeError<E> {
    fn from(value: EndpointError) -> Self {
        Self::Endpoint(value)
    }
}

/// Frames emitted while advancing one stateful procedure runtime.
#[derive(Debug, Default)]
pub struct ProcedureRuntimeOutcome {
    pub frames: Vec<FrameBytes>,
    pub dropped: bool,
}

/// Runtime for one leaf paired with one procedure-owned session type.
///
/// This runtime is deliberately narrow. It is the right tool when one leaf owns
/// one hook-backed procedure whose session type is explicit in the leaf's state.
/// Simpler one-shot procedures can stay on [`crate::protocol::tree::LeafRuntime`].
#[derive(Debug)]
pub struct ProcedureRuntime<L, P> {
    endpoint: ProtocolEndpoint,
    leaf: L,
    marker: PhantomData<P>,
}

impl<L, P> ProcedureRuntime<L, P> {
    #[must_use]
    pub fn new(endpoint: ProtocolEndpoint, leaf: L) -> Self {
        Self {
            endpoint,
            leaf,
            marker: PhantomData,
        }
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

impl<L, P> ProcedureRuntime<L, P>
where
    L: ProtocolLeaf + ProcedureStore<P>,
    P: Procedure<L>,
    P::Input: Archive,
    <P::Input as Archive>::Archived: rkyv::Portable
        + for<'b> rkyv::bytecheck::CheckBytes<rkyv::api::high::HighValidator<'b, Error>>
        + rkyv::Deserialize<P::Input, rkyv::api::high::HighDeserializer<Error>>,
    P::Error: fmt::Display,
{
    /// Delivers one framed protocol packet into the runtime.
    pub fn receive(
        &mut self,
        ingress: &Ingress,
        frame: FrameBytes,
    ) -> Result<ProcedureRuntimeOutcome, ProcedureRuntimeError<P::Error>> {
        let outcome = self.endpoint.receive(ingress, frame)?;
        self.process_endpoint_outcome(outcome)
    }

    /// Polls all live sessions for locally-generated hook traffic.
    ///
    /// Rationale: many long-lived procedures, including a remote shell, need to
    /// emit output even when no new inbound protocol packet has arrived.
    pub fn poll(&mut self) -> Result<ProcedureRuntimeOutcome, ProcedureRuntimeError<P::Error>> {
        let mut frames = Vec::new();
        let keys = self
            .leaf
            .procedure_sessions()
            .keys()
            .cloned()
            .collect::<Vec<_>>();

        for key in keys {
            let Some(mut session) = self.leaf.procedure_sessions().remove(&key) else {
                continue;
            };
            let effect = match P::poll(&mut self.leaf, &mut session) {
                Ok(effect) => self.ensure_terminal_packet(&key, effect),
                Err(error) => {
                    let _ = P::close(&mut self.leaf, session);
                    frames.extend(self.emit_internal_fault(&key)?);
                    let _ = error;
                    continue;
                }
            };

            match self.emit_outgoing(effect.outgoing) {
                Ok(outgoing) => frames.extend(outgoing.frames),
                Err(error) => {
                    if !effect.close_session {
                        self.leaf.procedure_sessions().insert(key, session);
                    } else {
                        let _ = P::close(&mut self.leaf, session);
                    }
                    return Err(error);
                }
            }

            if !effect.close_session {
                self.leaf.procedure_sessions().insert(key, session);
            } else {
                let _ = P::close(&mut self.leaf, session);
            }
        }

        Ok(ProcedureRuntimeOutcome {
            frames,
            dropped: false,
        })
    }

    fn process_endpoint_outcome(
        &mut self,
        outcome: super::EndpointOutcome,
    ) -> Result<ProcedureRuntimeOutcome, ProcedureRuntimeError<P::Error>> {
        let mut runtime = ProcedureRuntimeOutcome {
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
                if message.procedure_id != P::procedure_id() {
                    runtime
                        .frames
                        .extend(self.emit_internal_fault_if_possible(&message)?);
                    return Ok(runtime);
                }
                if message.response_hook.is_none() {
                    return Ok(runtime);
                }

                let session = match self.open_session(IncomingCall {
                    header,
                    message: message.clone(),
                }) {
                    Ok(session) => session,
                    Err(error) => {
                        runtime
                            .frames
                            .extend(self.emit_internal_fault_if_possible(&message)?);
                        let _ = error;
                        return Ok(runtime);
                    }
                };

                if let Some(hook) = message.response_hook {
                    self.leaf
                        .procedure_sessions()
                        .insert(HookKey::new(hook.return_path, hook.hook_id), session);
                }
            }
            LocalEvent::Data {
                header,
                message,
                hook_key,
            } => {
                let Some(mut session) = self.leaf.procedure_sessions().remove(&hook_key) else {
                    return Ok(runtime);
                };
                let effect = match P::on_data(
                    &mut self.leaf,
                    &mut session,
                    IncomingData {
                        header,
                        message,
                        hook_key: hook_key.clone(),
                    },
                ) {
                    Ok(effect) => self.ensure_terminal_packet(&hook_key, effect),
                    Err(error) => {
                        let _ = P::close(&mut self.leaf, session);
                        runtime.frames.extend(self.emit_internal_fault(&hook_key)?);
                        let _ = error;
                        return Ok(runtime);
                    }
                };
                match self.emit_outgoing(effect.outgoing) {
                    Ok(outgoing) => runtime.frames.extend(outgoing.frames),
                    Err(error) => {
                        if !effect.close_session {
                            self.leaf.procedure_sessions().insert(hook_key, session);
                        } else {
                            let _ = P::close(&mut self.leaf, session);
                        }
                        return Err(error);
                    }
                }
                if !effect.close_session {
                    self.leaf.procedure_sessions().insert(hook_key, session);
                } else {
                    let _ = P::close(&mut self.leaf, session);
                }
            }
            LocalEvent::Fault {
                header,
                message,
                hook_key,
            } => {
                let Some(mut session) = self.leaf.procedure_sessions().remove(&hook_key) else {
                    return Ok(runtime);
                };
                let on_fault_result = P::on_fault(
                    &mut self.leaf,
                    &mut session,
                    IncomingFault {
                        header,
                        fault: message,
                        hook_key: hook_key.clone(),
                    },
                );
                let close_result = P::close(&mut self.leaf, session);
                if let Err(error) = on_fault_result {
                    let _ = close_result;
                    runtime.frames.extend(self.emit_internal_fault(&hook_key)?);
                    let _ = error;
                    return Ok(runtime);
                }
                if let Err(error) = close_result {
                    runtime.frames.extend(self.emit_internal_fault(&hook_key)?);
                    let _ = error;
                    return Ok(runtime);
                }
            }
        }

        Ok(runtime)
    }

    fn open_session(&mut self, call: IncomingCall) -> Result<P, DispatchError<P::Error>> {
        let input = decode_call_input::<P::Input>(call.message.data.as_slice())
            .map_err(DispatchError::Decode)?;
        P::open(
            &mut self.leaf,
            super::Call {
                input,
                caller_path: call.header.src_path,
                procedure_id: call.message.procedure_id,
                dst_leaf: call.header.dst_leaf,
                response_hook: call
                    .message
                    .response_hook
                    .map(|hook| HookKey::new(hook.return_path, hook.hook_id)),
            },
        )
        .map_err(DispatchError::Handler)
    }

    fn emit_outgoing(
        &mut self,
        outgoing: Vec<OutgoingData>,
    ) -> Result<ProcedureRuntimeOutcome, ProcedureRuntimeError<P::Error>> {
        let mut runtime = ProcedureRuntimeOutcome::default();
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

    /// Emits an upstream internal fault for the current procedure if the caller
    /// declared a response hook.
    pub fn emit_internal_fault_if_possible(
        &mut self,
        message: &CallMessage,
    ) -> Result<Vec<FrameBytes>, ProcedureRuntimeError<P::Error>> {
        let Some(HookTarget {
            return_path,
            hook_id,
        }) = message.response_hook.as_ref()
        else {
            return Ok(Vec::new());
        };
        let outcome = self.endpoint.emit_fault_if_possible(
            Some(HookKey::new(return_path.clone(), *hook_id)),
            ProtocolFault::INTERNAL_ERROR,
        )?;
        Ok(self.process_endpoint_outcome(outcome)?.frames)
    }

    fn emit_internal_fault(
        &mut self,
        hook_key: &HookKey,
    ) -> Result<Vec<FrameBytes>, ProcedureRuntimeError<P::Error>> {
        let outcome = self
            .endpoint
            .emit_fault_if_possible(Some(hook_key.clone()), ProtocolFault::INTERNAL_ERROR)?;
        Ok(self.process_endpoint_outcome(outcome)?.frames)
    }

    fn ensure_terminal_packet(
        &self,
        hook_key: &HookKey,
        mut effect: ProcedureEffect,
    ) -> ProcedureEffect {
        if let Some(index) = effect.outgoing.iter().position(|packet| packet.end_hook) {
            effect.outgoing.truncate(index + 1);
        }
        let local_end_already_sent = self
            .endpoint
            .hooks
            .active(hook_key)
            .map_or(true, |active| active.local_ended);
        if effect.close_session
            && !effect.outgoing.iter().any(|packet| packet.end_hook)
            && !local_end_already_sent
        {
            effect.outgoing.push(OutgoingData {
                dst_path: hook_key.return_path.clone(),
                hook_id: hook_key.hook_id,
                procedure_id: P::procedure_id(),
                data: Vec::new(),
                end_hook: true,
            });
        }
        effect
    }
}
