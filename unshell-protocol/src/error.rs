/// Direction across the next local routing boundary.
///
/// The endpoint derives this from its own absolute path and the packet's
/// destination path. Packets are never trusted to declare their direction because
/// that would let an untrusted peer spoof the local routing boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteDirection {
    /// The packet moves toward this endpoint's direct parent.
    Upward,

    /// The packet moves toward one of this endpoint's direct children.
    Downward,
}

/// Top-level endpoint failure for packet conversion and local routing.
///
/// These are local processing failures, not protocol fault packets. A transport or
/// leaf may choose to drop the packet, log it, or translate it into a higher-level
/// fault depending on where the packet came from. Route variants stay flat so the
/// hot route path does not need a second nested enum just to explain the failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointError {
    /// This endpoint cannot route because its absolute path has not been assigned.
    ///
    /// The current runtime uses an empty path as "not initialized". If the protocol
    /// later supports an empty root path, route initialization should become an
    /// explicit flag instead of being inferred from `path.is_empty()`.
    EndpointPathUnset,

    /// The packet destination is not local, below this endpoint, or above this endpoint.
    ///
    /// This catches sideways or forged paths, for example local `/a/b` receiving a
    /// packet addressed to `/a/c`.
    DestinationOutsideLocalTree,

    /// A route points upward, but this endpoint has no parent segment to forward to.
    ///
    /// This means the path topology is internally inconsistent for upward routing.
    MissingParentRoute,

    /// The packet needs a registered connection for the computed next hop, but none exists.
    ///
    /// Route derivation succeeded. Delivery fails only because the local connection
    /// table does not contain the adjacent endpoint in the required direction.
    MissingConnection {
        /// Adjacent endpoint that should receive the packet next.
        next_hop: u32,

        /// Direction that the local connection must be registered for.
        direction: RouteDirection,
    },

    /// Inbound transport bytes arrived from an endpoint that is not registered locally.
    ///
    /// Direction-aware routing needs to know whether the remote endpoint is the
    /// parent or a child before it can decide whether local delivery is downward or
    /// upward traffic. Unknown peers are rejected before hook state can be mutated.
    UnknownConnection {
        /// Adjacent endpoint that supplied the inbound packet.
        remote_id: u32,
    },

    /// The same adjacent endpoint is registered as both parent and child.
    ///
    /// The legacy connection table stores direction as a boolean. Both entries being
    /// present would make inbound hook policy ambiguous, so the endpoint refuses to
    /// route the packet until the connection state is made unambiguous.
    AmbiguousConnection {
        /// Adjacent endpoint whose direction cannot be inferred.
        remote_id: u32,
    },

    /// An inbound packet tried to move in the opposite direction from its connection.
    ///
    /// A parent/upstream peer may send packets downward, while a child/downstream
    /// peer may send packets upward. This prevents a child from using its transport
    /// link to forge downward traffic to siblings or descendants.
    InboundDirectionMismatch {
        /// Adjacent endpoint that supplied the inbound packet.
        remote_id: u32,

        /// Direction allowed by the registered connection.
        expected: RouteDirection,

        /// Direction implied by the packet destination path.
        actual: RouteDirection,
    },

    /// The packet is trying to move upward without known hook state.
    ///
    /// Upward hook traffic is gated by local hook state so a peer cannot forge a
    /// return path just by choosing an ancestor destination.
    UnknownHook {
        /// Hook id claimed by the upward packet.
        hook_id: u16,
    },

    /// The hook exists, but it is registered for a different adjacent peer.
    ///
    /// Hook state is peer-bound so one child cannot reuse another child's paved
    /// return channel. For locally generated upward traffic, `actual_peer` is the
    /// parent next hop; for inbound upward traffic, it is the child that supplied the
    /// frame.
    HookPeerMismatch {
        /// Hook id claimed by the upward packet.
        hook_id: u16,

        /// Adjacent peer recorded when the hook was paved.
        expected_peer: u32,

        /// Adjacent peer trying to use the hook now.
        actual_peer: u32,
    },

    /// A packet could not be converted into bytes for transport.
    ///
    /// Endpoint-level code that drains outbound queues often wants one error type
    /// for both routing and framing. Keeping the source error preserves the exact
    /// packet-size invariant that failed.
    PacketSerialize {
        /// Exact packet serialization failure.
        source: SerializeError,
    },

    /// Incoming bytes could not be parsed into a packet.
    ///
    /// This represents a frame rejection before routing begins. The source error is
    /// retained so callers can distinguish truncation from malformed body fields.
    PacketDeserialize {
        /// Exact packet deserialization failure.
        source: DeserializeError,
    },
}

/// Errors produced while converting a [`Packet`] into its wire representation.
///
/// These failures are size-bound checks rather than transport errors. They protect
/// the length fields in the frame from integer overflow or values that cannot be
/// represented by the protocol's current `u32` length fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerializeError {
    /// The packet path contains more bytes than the frame length field can represent.
    PathTooLarge,

    /// The body section is too large to encode in a `u32` length field.
    BodyTooLarge,
}

/// Errors produced while parsing a [`Packet`] from untrusted wire bytes.
///
/// Deserialization rejects partial or inconsistent frames before endpoint routing
/// sees them. Keeping these separate from route failures makes it clear whether a
/// packet failed before or after it became structured data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeserializeError {
    /// The buffer ended before the parser could read the required field.
    BufferTooShort,

    /// The advertised body length does not fit inside the provided buffer.
    BodyLengthMismatch,

    /// The path length overflowed while computing the path byte range.
    PathTooLong,
}

impl From<SerializeError> for EndpointError {
    /// Wraps packet serialization failures for endpoint-level callers.
    fn from(source: SerializeError) -> Self {
        Self::PacketSerialize { source }
    }
}

impl From<DeserializeError> for EndpointError {
    /// Wraps packet deserialization failures for endpoint-level callers.
    fn from(source: DeserializeError) -> Self {
        Self::PacketDeserialize { source }
    }
}
