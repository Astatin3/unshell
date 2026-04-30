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

use alloc::{collections::BTreeMap, string::String, vec, vec::Vec};
use core::{fmt, marker::PhantomData};

use rkyv::{Archive, rancor::Error};

use crate::protocol::{CallMessage, FrameBytes, HookTarget, ProtocolFault};

use super::{
    DispatchError, Endpoint, EndpointError, HookKey, IncomingData, IncomingFault, Ingress,
    LocalEvent, OutgoingData, ProtocolEndpoint, ProtocolLeaf, decode_call_input,
};

/// Canonical compile-time metadata for one procedure surface.
///
/// What it is: a trait that defines the leaf type and local suffix used to derive
/// one stable protocol `procedure_id`.
///
/// Why it exists: compile-time leaf declarations and future typed remote methods
/// need to talk about procedures without hand-assembling identifiers at each use
/// site.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::{ProcedureMetadata, ProtocolLeaf};
/// struct ExampleLeaf;
/// impl ProtocolLeaf for ExampleLeaf {
///     fn leaf_name() -> String { "org.example.v1.shell".into() }
/// }
/// struct Open;
/// impl ProcedureMetadata for Open {
///     type Leaf = ExampleLeaf;
///     const PROCEDURE_SUFFIX: &'static str = "open";
/// }
/// assert_eq!(Open::procedure_id(), "org.example.v1.shell.open");
/// ```
pub trait ProcedureMetadata: Sized {
    /// Leaf surface this procedure belongs to.
    type Leaf: ProtocolLeaf;

    /// Returns the local suffix used to derive the full canonical `procedure_id`.
    const PROCEDURE_SUFFIX: &'static str;

    /// Returns the local suffix used to derive the full canonical `procedure_id`.
    fn procedure_suffix() -> &'static str {
        Self::PROCEDURE_SUFFIX
    }

    /// Returns the canonical `procedure_id` for this procedure.
    fn procedure_id() -> String {
        let mut procedure_id = <Self::Leaf as ProtocolLeaf>::leaf_name();
        procedure_id.push('.');
        procedure_id.push_str(Self::procedure_suffix());
        procedure_id
    }
}

/// Generated metadata for one stateful procedure bound to one leaf type.
///
/// This metadata is intentionally tiny: one procedure suffix plus the derived
/// full `procedure_id`. The leaf still owns all session storage explicitly.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::{ProcedureMetadata, ProtocolLeaf, StatefulProcedureMetadata};
/// struct ExampleLeaf;
/// impl ProtocolLeaf for ExampleLeaf {
///     fn leaf_name() -> String { "org.example.v1.shell".into() }
/// }
/// struct Open;
/// impl ProcedureMetadata for Open {
///     type Leaf = ExampleLeaf;
///     const PROCEDURE_SUFFIX: &'static str = "open";
/// }
/// fn _compat<T: StatefulProcedureMetadata<ExampleLeaf>>() {}
/// _compat::<Open>();
/// assert_eq!(Open::procedure_id(), "org.example.v1.shell.open");
/// ```
pub trait StatefulProcedureMetadata<L>: ProcedureMetadata<Leaf = L> + Sized
where
    L: ProtocolLeaf,
{
}

impl<T, L> StatefulProcedureMetadata<L> for T
where
    T: ProcedureMetadata<Leaf = L>,
    L: ProtocolLeaf,
{
}

/// Explicit storage access for one procedure session map inside the leaf.
///
/// Rationale: the leaf remains the source of truth for its active sessions. This
/// avoids hidden generated enums or side tables and keeps debugging obvious.
///
/// # Example
/// ```rust
/// use std::collections::BTreeMap;
/// use unshell::protocol::tree::{HookKey, ProcedureStore};
/// struct Session;
/// struct Leaf { sessions: BTreeMap<HookKey, Session> }
/// impl ProcedureStore<Session> for Leaf {
///     fn procedure_sessions(&mut self) -> &mut BTreeMap<HookKey, Session> {
///         &mut self.sessions
///     }
/// }
/// ```
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
/// use std::collections::BTreeMap;
/// use std::string::String;
/// use unshell::{Procedure, leaf};
/// use unshell::protocol::tree::{Call, HookKey, Procedure, ProcedureEffect, ProcedureStore};
///
/// #[derive(Default)]
/// struct StreamLeaf {
///     sessions: BTreeMap<HookKey, OpenProcedure>,
/// }
///
/// leaf! {
///     id = "org.example.v1.stream",
///     procedures = [OpenProcedure],
///     endpoint_struct = StreamLeaf,
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
pub trait Procedure<L>: ProcedureMetadata<Leaf = L> + Sized
where
    L: ProtocolLeaf,
{
    /// Leaf-specific error surfaced while opening or advancing the session.
    type Error;
    /// Typed input payload decoded from the opening call.
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
///
/// This exists as the normalized result of one session step: some outgoing hook packets plus an
/// explicit decision about whether the session should stay alive.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::ProcedureEffect;
/// let effect = ProcedureEffect::close(Vec::new());
/// assert!(effect.close_session);
/// ```
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ProcedureEffect {
    /// `Data` packets to emit after the session step completes.
    pub outgoing: Vec<OutgoingData>,
    /// Whether the runtime should remove the session after sending `outgoing`.
    pub close_session: bool,
}

impl ProcedureEffect {
    /// Builds an effect that keeps the session alive after emitting `outgoing`.
    #[must_use]
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::ProcedureEffect;
    /// let effect = ProcedureEffect::outgoing(Vec::new());
    /// assert!(!effect.close_session);
    /// ```
    pub fn outgoing(outgoing: Vec<OutgoingData>) -> Self {
        Self {
            outgoing,
            close_session: false,
        }
    }

    /// Builds an effect that closes the session after emitting `outgoing`.
    #[must_use]
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::ProcedureEffect;
    /// let effect = ProcedureEffect::close(Vec::new());
    /// assert!(effect.close_session);
    /// ```
    pub fn close(outgoing: Vec<OutgoingData>) -> Self {
        Self {
            outgoing,
            close_session: true,
        }
    }
}

/// Error surfaced by the procedure runtime.
///
/// This exists so callers can tell apart transport/runtime failures from an opening call that
/// could not establish a procedure session.
///
/// # Example
/// ```rust
/// use unshell::protocol::FrameError;
/// use unshell::protocol::tree::{DispatchError, ProcedureRuntimeError};
/// let error: ProcedureRuntimeError<core::convert::Infallible> =
///     ProcedureRuntimeError::Decode(DispatchError::Decode(FrameError::Truncated));
/// assert!(matches!(error, ProcedureRuntimeError::Decode(_)));
/// ```
#[derive(Debug)]
pub enum ProcedureRuntimeError<E> {
    /// Protocol endpoint routing or framing failed.
    Endpoint(EndpointError),
    /// The opening call failed to decode or open cleanly before a session existed.
    ///
    /// Once a session is already live, runtime failures prefer emitting protocol faults and
    /// tearing down that session rather than surfacing leaf errors directly.
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
///
/// This exists so callers can flush emitted frames to transport while also observing whether the
/// inbound packet was intentionally dropped.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::ProcedureRuntimeOutcome;
/// let outcome = ProcedureRuntimeOutcome::default();
/// assert!(outcome.frames.is_empty());
/// ```
#[derive(Debug, Default)]
pub struct ProcedureRuntimeOutcome {
    /// Frames emitted while processing the current step.
    pub frames: Vec<FrameBytes>,
    /// Whether the endpoint dropped the incoming packet.
    pub dropped: bool,
}

/// Runtime for one leaf paired with one procedure-owned session type.
///
/// This runtime is deliberately narrow. It is the right tool when one leaf owns
/// one hook-backed procedure whose session type is explicit in the leaf's state.
/// Simpler one-shot procedures can stay on [`crate::protocol::tree::LeafRuntime`].
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::ProcedureRuntime;
/// # struct Leaf;
/// # struct Proc;
/// # let _ = core::marker::PhantomData::<ProcedureRuntime<Leaf, Proc>>;
/// ```
#[derive(Debug)]
pub struct ProcedureRuntime<L, P> {
    endpoint: ProtocolEndpoint,
    leaf: L,
    marker: PhantomData<P>,
}

impl<L, P> ProcedureRuntime<L, P> {
    /// Builds a procedure runtime from one endpoint and one leaf instance.
    #[must_use]
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{ProcedureRuntime, ProtocolEndpoint};
    /// struct Leaf;
    /// struct Proc;
    /// let runtime = ProcedureRuntime::<Leaf, Proc>::new(
    ///     ProtocolEndpoint::new(Vec::new(), None, Vec::new(), Vec::new()),
    ///     Leaf,
    /// );
    /// let _ = runtime;
    /// ```
    pub fn new(endpoint: ProtocolEndpoint, leaf: L) -> Self {
        Self {
            endpoint,
            leaf,
            marker: PhantomData,
        }
    }

    /// Returns the underlying protocol endpoint.
    #[must_use]
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{ProcedureRuntime, ProtocolEndpoint};
    /// struct Leaf;
    /// struct Proc;
    /// let runtime = ProcedureRuntime::<Leaf, Proc>::new(ProtocolEndpoint::new(Vec::new(), None, Vec::new(), Vec::new()), Leaf);
    /// let _ = runtime.endpoint();
    /// ```
    pub fn endpoint(&self) -> &ProtocolEndpoint {
        &self.endpoint
    }

    /// Returns a mutable reference to the protocol endpoint.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{ProcedureRuntime, ProtocolEndpoint};
    /// struct Leaf;
    /// struct Proc;
    /// let mut runtime = ProcedureRuntime::<Leaf, Proc>::new(ProtocolEndpoint::new(Vec::new(), None, Vec::new(), Vec::new()), Leaf);
    /// let _ = runtime.endpoint_mut();
    /// ```
    pub fn endpoint_mut(&mut self) -> &mut ProtocolEndpoint {
        &mut self.endpoint
    }

    /// Returns the hosted leaf instance.
    #[must_use]
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{ProcedureRuntime, ProtocolEndpoint};
    /// struct Leaf;
    /// struct Proc;
    /// let runtime = ProcedureRuntime::<Leaf, Proc>::new(ProtocolEndpoint::new(Vec::new(), None, Vec::new(), Vec::new()), Leaf);
    /// let _ = runtime.leaf();
    /// ```
    pub fn leaf(&self) -> &L {
        &self.leaf
    }

    /// Returns a mutable reference to the hosted leaf instance.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{ProcedureRuntime, ProtocolEndpoint};
    /// struct Leaf;
    /// struct Proc;
    /// let mut runtime = ProcedureRuntime::<Leaf, Proc>::new(ProtocolEndpoint::new(Vec::new(), None, Vec::new(), Vec::new()), Leaf);
    /// let _ = runtime.leaf_mut();
    /// ```
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
    ///
    /// # Example
    /// ```rust
    /// # use unshell::protocol::tree::ProcedureRuntime;
    /// # struct Leaf;
    /// # struct Proc;
    /// # let _ = core::marker::PhantomData::<ProcedureRuntime<Leaf, Proc>>;
    /// ```
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
    ///
    /// # Example
    /// ```rust
    /// # use unshell::protocol::tree::ProcedureRuntime;
    /// # struct Leaf;
    /// # struct Proc;
    /// # let _ = core::marker::PhantomData::<ProcedureRuntime<Leaf, Proc>>;
    /// ```
    pub fn poll(&mut self) -> Result<ProcedureRuntimeOutcome, ProcedureRuntimeError<P::Error>> {
        let mut frames = Vec::new();
        let keys = self
            .leaf
            .procedure_sessions()
            .keys()
            .cloned()
            .collect::<Vec<_>>();

        for key in keys {
            let Some(session) = self.leaf.procedure_sessions().remove(&key) else {
                continue;
            };
            // Collect keys first and temporarily remove each session so procedure callbacks can
            // mutate the leaf without fighting the session-table borrow.
            frames.extend(self.poll_session(key, session)?);
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
        match outcome {
            super::EndpointOutcome::Forward { frame, .. } => Ok(ProcedureRuntimeOutcome {
                frames: vec![frame],
                dropped: false,
            }),
            super::EndpointOutcome::Dropped => Ok(ProcedureRuntimeOutcome {
                frames: Vec::new(),
                dropped: true,
            }),
            super::EndpointOutcome::Local(event) => self.process_local_event(event),
        }
    }

    fn poll_session(
        &mut self,
        key: HookKey,
        session: P,
    ) -> Result<Vec<FrameBytes>, ProcedureRuntimeError<P::Error>> {
        self.advance_session(key, session, P::poll)
    }

    fn advance_session<F>(
        &mut self,
        key: HookKey,
        mut session: P,
        step: F,
    ) -> Result<Vec<FrameBytes>, ProcedureRuntimeError<P::Error>>
    where
        F: FnOnce(&mut L, &mut P) -> Result<ProcedureEffect, P::Error>,
    {
        let effect = match step(&mut self.leaf, &mut session) {
            Ok(effect) => self.ensure_terminal_packet(&key, effect),
            Err(error) => {
                let _ = P::close(&mut self.leaf, session);
                let frames = self.emit_internal_fault(Some(key.clone()))?;
                let _ = error;
                return Ok(frames);
            }
        };

        let outgoing = match self.emit_outgoing(effect.outgoing) {
            Ok(outgoing) => outgoing.frames,
            Err(error) => {
                // Emit failures are transport/runtime failures, not leaf-procedure failures. Keep
                // the session when it asked to stay open so the caller can retry later.
                if !effect.close_session {
                    self.leaf.procedure_sessions().insert(key, session);
                } else {
                    let _ = P::close(&mut self.leaf, session);
                }
                return Err(error);
            }
        };

        if !effect.close_session {
            self.leaf.procedure_sessions().insert(key, session);
        } else {
            let _ = P::close(&mut self.leaf, session);
        }

        Ok(outgoing)
    }

    fn process_local_event(
        &mut self,
        event: LocalEvent,
    ) -> Result<ProcedureRuntimeOutcome, ProcedureRuntimeError<P::Error>> {
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
        header: crate::protocol::PacketHeader,
        message: CallMessage,
    ) -> Result<ProcedureRuntimeOutcome, ProcedureRuntimeError<P::Error>> {
        let mut runtime = ProcedureRuntimeOutcome::default();
        if message.procedure_id != P::procedure_id() {
            // Once this runtime receives a call, a wrong procedure id is a protocol mismatch.
            // Fault the caller rather than surfacing a leaf-local error it cannot recover from.
            runtime
                .frames
                .extend(self.emit_internal_fault_if_possible(message.response_hook.as_ref())?);
            return Ok(runtime);
        }
        let Some(hook) = message.response_hook.as_ref() else {
            return Ok(runtime);
        };
        let hook_key = HookKey::new(hook.return_path.clone(), hook.hook_id);

        let session = match self.open_session(header, message) {
            Ok(session) => session,
            Err(error) => {
                // Session open failures still fault the caller when a response hook exists, but do
                // not leak leaf-local details over the wire.
                runtime
                    .frames
                    .extend(self.emit_internal_fault(Some(hook_key.clone()))?);
                let _ = error;
                return Ok(runtime);
            }
        };

        self.leaf.procedure_sessions().insert(hook_key, session);
        Ok(runtime)
    }

    fn process_local_data(
        &mut self,
        header: crate::protocol::PacketHeader,
        message: crate::protocol::DataMessage,
        hook_key: HookKey,
    ) -> Result<ProcedureRuntimeOutcome, ProcedureRuntimeError<P::Error>> {
        let Some(session) = self.leaf.procedure_sessions().remove(&hook_key) else {
            return Ok(ProcedureRuntimeOutcome::default());
        };
        let outgoing = self.advance_session(hook_key.clone(), session, |leaf, session| {
            P::on_data(
                leaf,
                session,
                IncomingData {
                    header,
                    message,
                    hook_key,
                },
            )
        })?;
        Ok(ProcedureRuntimeOutcome {
            frames: outgoing,
            dropped: false,
        })
    }

    fn process_local_fault(
        &mut self,
        header: crate::protocol::PacketHeader,
        message: crate::protocol::FaultMessage,
        hook_key: HookKey,
    ) -> Result<ProcedureRuntimeOutcome, ProcedureRuntimeError<P::Error>> {
        let Some(mut session) = self.leaf.procedure_sessions().remove(&hook_key) else {
            return Ok(ProcedureRuntimeOutcome::default());
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
        // Always attempt both the fault observer and the final close hook so resource cleanup can
        // still run even when the leaf reports an error while handling the fault.
        let close_result = P::close(&mut self.leaf, session);
        if let Err(error) = on_fault_result {
            let _ = close_result;
            let frames = self.emit_internal_fault(Some(hook_key.clone()))?;
            let _ = error;
            return Ok(ProcedureRuntimeOutcome {
                frames,
                dropped: false,
            });
        }
        if let Err(error) = close_result {
            let frames = self.emit_internal_fault(Some(hook_key))?;
            let _ = error;
            return Ok(ProcedureRuntimeOutcome {
                frames,
                dropped: false,
            });
        }
        Ok(ProcedureRuntimeOutcome::default())
    }

    fn open_session(
        &mut self,
        header: crate::protocol::PacketHeader,
        message: CallMessage,
    ) -> Result<P, DispatchError<P::Error>> {
        let CallMessage {
            procedure_id,
            data,
            response_hook,
        } = message;
        let input =
            decode_call_input::<P::Input>(data.as_slice()).map_err(DispatchError::Decode)?;
        P::open(
            &mut self.leaf,
            super::Call {
                input,
                caller_path: header.src_path,
                procedure_id,
                dst_leaf: header.dst_leaf,
                response_hook: response_hook
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
    ///
    /// # Example
    /// ```rust
    /// # use unshell::protocol::tree::ProcedureRuntime;
    /// # struct Leaf;
    /// # struct Proc;
    /// # let _ = core::marker::PhantomData::<ProcedureRuntime<Leaf, Proc>>;
    /// ```
    pub fn emit_internal_fault_if_possible(
        &mut self,
        hook: Option<&HookTarget>,
    ) -> Result<Vec<FrameBytes>, ProcedureRuntimeError<P::Error>> {
        let Some(HookTarget {
            return_path,
            hook_id,
        }) = hook
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
        hook_key: Option<HookKey>,
    ) -> Result<Vec<FrameBytes>, ProcedureRuntimeError<P::Error>> {
        let outcome = self
            .endpoint
            .emit_fault_if_possible(hook_key, ProtocolFault::INTERNAL_ERROR)?;
        Ok(self.process_endpoint_outcome(outcome)?.frames)
    }

    /// Ensures a closing session leaves the protocol hook in a fully terminated state.
    ///
    /// If leaf code requests `close_session` without emitting an explicit terminal packet, the
    /// runtime synthesizes an empty final `Data` frame so the hook closes cleanly on the wire.
    fn ensure_terminal_packet(
        &self,
        hook_key: &HookKey,
        mut effect: ProcedureEffect,
    ) -> ProcedureEffect {
        // Once a session emits `end_hook`, later packets would violate the protocol,
        // so the runtime keeps only the prefix through that terminal packet.
        if let Some(index) = effect.outgoing.iter().position(|packet| packet.end_hook) {
            // The protocol allows only one terminal packet per direction, so ignore anything a
            // procedure tried to emit after the first close marker.
            effect.outgoing.truncate(index + 1);
        }
        let local_end_already_sent = self
            .endpoint
            .hooks
            .active(hook_key)
            .is_none_or(|active| active.local_ended);
        if effect.close_session
            && !effect.outgoing.iter().any(|packet| packet.end_hook)
            && !local_end_already_sent
        {
            // Closing a session without an explicit terminal packet would leave the
            // protocol hook half-open, so emit an empty terminal frame on behalf of
            // the procedure unless the local side already ended earlier.
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
