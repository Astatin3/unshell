//! Runtime connection admission and routing metadata.
//!
//! A connection is not routable just because a transport exists. Only
//! [`ConnectionState::Registered`] connections are allowed to produce protocol
//! ingress or receive forwarded frames.

use crate::alloc::string::String;
use crate::alloc::vec::Vec;

/// Stable runtime handle for one transport connection slot.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConnectionId(u64);

impl ConnectionId {
    /// Creates a connection identifier from a raw value.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw identifier value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Monotonic incarnation number for one connection slot.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConnectionGeneration(u64);

impl ConnectionGeneration {
    /// First generation assigned to a new connection slot.
    pub const INITIAL: Self = Self(0);

    /// Creates a generation from a raw value.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw generation value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Returns the next generation, saturating at `u64::MAX`.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

/// Local tree relationship for a registered connection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ConnectionDirection {
    /// The peer is the direct parent of this endpoint.
    Parent,
    /// The peer is a direct child of this endpoint.
    Child,
}

/// Metadata that makes a connection routable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredConnection {
    direction: ConnectionDirection,
    peer_path: Vec<String>,
    generation: ConnectionGeneration,
}

impl RegisteredConnection {
    /// Creates registered routing metadata.
    #[must_use]
    pub const fn new(
        direction: ConnectionDirection,
        peer_path: Vec<String>,
        generation: ConnectionGeneration,
    ) -> Self {
        Self {
            direction,
            peer_path,
            generation,
        }
    }

    /// Returns the local tree relationship.
    #[must_use]
    pub const fn direction(&self) -> ConnectionDirection {
        self.direction
    }

    /// Returns the registered peer path.
    #[must_use]
    pub fn peer_path(&self) -> &[String] {
        &self.peer_path
    }

    /// Returns the connection generation.
    #[must_use]
    pub const fn generation(&self) -> ConnectionGeneration {
        self.generation
    }
}

/// Runtime lifecycle state for one connection slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConnectionState {
    /// The transport exists but has not started or completed admission.
    Connected {
        /// Connection generation for this transport incarnation.
        generation: ConnectionGeneration,
    },
    /// The runtime is evaluating whether this peer should become routable.
    Authenticating {
        /// Connection generation for this transport incarnation.
        generation: ConnectionGeneration,
    },
    /// The peer is admitted into protocol routing.
    Registered(RegisteredConnection),
    /// The runtime is tearing this connection down and should reject new work.
    Draining {
        /// Connection generation for this transport incarnation.
        generation: ConnectionGeneration,
    },
    /// The connection is closed and retained only as historical metadata.
    Closed {
        /// Connection generation for this transport incarnation.
        generation: ConnectionGeneration,
    },
}

impl ConnectionState {
    /// Returns the generation associated with this state.
    #[must_use]
    pub const fn generation(&self) -> ConnectionGeneration {
        match self {
            Self::Connected { generation }
            | Self::Authenticating { generation }
            | Self::Draining { generation }
            | Self::Closed { generation } => *generation,
            Self::Registered(registered) => registered.generation(),
        }
    }

    /// Returns registered metadata when this connection is routable.
    #[must_use]
    pub const fn registered(&self) -> Option<&RegisteredConnection> {
        match self {
            Self::Registered(registered) => Some(registered),
            Self::Connected { .. }
            | Self::Authenticating { .. }
            | Self::Draining { .. }
            | Self::Closed { .. } => None,
        }
    }
}

/// One runtime connection slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Connection {
    id: ConnectionId,
    state: ConnectionState,
}

impl Connection {
    /// Creates a connected but unroutable connection slot.
    #[must_use]
    pub const fn connected(id: ConnectionId, generation: ConnectionGeneration) -> Self {
        Self {
            id,
            state: ConnectionState::Connected { generation },
        }
    }

    /// Creates a registered connection slot.
    #[must_use]
    pub const fn registered(
        id: ConnectionId,
        direction: ConnectionDirection,
        peer_path: Vec<String>,
        generation: ConnectionGeneration,
    ) -> Self {
        Self {
            id,
            state: ConnectionState::Registered(RegisteredConnection::new(
                direction, peer_path, generation,
            )),
        }
    }

    /// Returns the connection id.
    #[must_use]
    pub const fn id(&self) -> ConnectionId {
        self.id
    }

    /// Returns the current connection state.
    #[must_use]
    pub const fn state(&self) -> &ConnectionState {
        &self.state
    }

    /// Replaces the current connection state.
    pub fn set_state(&mut self, state: ConnectionState) {
        self.state = state;
    }
}

/// Connection metadata table owned by the runtime.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Connections {
    entries: Vec<Connection>,
}

impl Connections {
    /// Creates an empty table.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Inserts a connection descriptor.
    pub fn push(&mut self, connection: Connection) {
        self.entries.push(connection);
    }

    /// Returns all connection descriptors.
    #[must_use]
    pub fn entries(&self) -> &[Connection] {
        &self.entries
    }

    /// Finds a connection by id.
    #[must_use]
    pub fn get(&self, id: ConnectionId) -> Option<&Connection> {
        self.entries.iter().find(|entry| entry.id() == id)
    }

    /// Finds a mutable connection by id.
    #[must_use]
    pub fn get_mut(&mut self, id: ConnectionId) -> Option<&mut Connection> {
        self.entries.iter_mut().find(|entry| entry.id() == id)
    }

    /// Returns registered metadata for a routable connection.
    #[must_use]
    pub fn registered(&self, id: ConnectionId) -> Option<&RegisteredConnection> {
        self.get(id)
            .and_then(|connection| connection.state().registered())
    }

    /// Finds a registered connection by direction.
    #[must_use]
    pub fn registered_by_direction(&self, direction: ConnectionDirection) -> Option<&Connection> {
        self.entries.iter().find(|entry| {
            entry
                .state()
                .registered()
                .is_some_and(|registered| registered.direction() == direction)
        })
    }

    /// Finds a registered connection by direction and peer path.
    #[must_use]
    pub fn registered_by_path(
        &self,
        direction: ConnectionDirection,
        peer_path: &[String],
    ) -> Option<&Connection> {
        self.entries.iter().find(|entry| {
            entry.state().registered().is_some_and(|registered| {
                registered.direction() == direction && registered.peer_path() == peer_path
            })
        })
    }

    /// Makes every matching registered connection except `except` unroutable.
    pub(crate) fn demote_registered_direction_except(
        &mut self,
        direction: ConnectionDirection,
        except: ConnectionId,
    ) {
        for entry in &mut self.entries {
            let Some(registered) = entry.state().registered() else {
                continue;
            };
            if entry.id() == except || registered.direction() != direction {
                continue;
            }

            entry.set_state(ConnectionState::Connected {
                generation: registered.generation(),
            });
        }
    }

    /// Makes every matching registered peer path except `except` unroutable.
    pub(crate) fn demote_registered_path_except(
        &mut self,
        direction: ConnectionDirection,
        peer_path: &[String],
        except: ConnectionId,
    ) {
        for entry in &mut self.entries {
            let Some(registered) = entry.state().registered() else {
                continue;
            };
            if entry.id() == except
                || registered.direction() != direction
                || registered.peer_path() != peer_path
            {
                continue;
            }

            entry.set_state(ConnectionState::Connected {
                generation: registered.generation(),
            });
        }
    }
}

/// Read-only connection table view exposed to leaf contexts.
pub trait ConnectionTable {
    /// Returns registered metadata for a routable connection.
    fn registered(&self, id: ConnectionId) -> Option<&RegisteredConnection>;
}

impl ConnectionTable for Connections {
    fn registered(&self, id: ConnectionId) -> Option<&RegisteredConnection> {
        Self::registered(self, id)
    }
}
