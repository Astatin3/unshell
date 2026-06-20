use std::{
    io::Write,
    process::{Child, Command, Stdio},
};

use unshell::{
    crypto::hash_str_32,
    protocol::{Endpoint, HookID, Packet, PacketQueue, Session, SessionInitError, SessionStatus},
    unshell_leaf,
};

macro_rules! version {
    () => {
        env!("CARGO_PKG_VERSION")
    };
}

pub const IDENTIFIER: &str = concat!("dev.unshell.", version!(), ".shell");
pub const SESSION_ID: &str = concat!("dev.unshell.", version!(), ".shell.session");

pub const IDENTIFIER_HASH: u32 = hash_str_32(IDENTIFIER);
pub const SESSION_ID_HASH: u32 = hash_str_32(SESSION_ID);

unshell_leaf! {
    pub leaf ShellLeaf for ShellState {
        id: IDENTIFIER_HASH,
        meta: unshell::protocol::LeafMeta {
            name: "Shell",
            identifier: IDENTIFIER,
            version: version!(),
            authors: vec!["ASTATIN3"],
        },
        sessions {
            shell: ShellSession,
        }
        procedures {
            // ping: PingProcedure,
        }
    }
}

/// Runtime state for the native shell leaf.
///
/// The process state lives in per-hook [`ShellSessionState`] values because every
/// routed hook owns one child shell. The leaf-level state is intentionally empty for
/// now, but keeping a named type gives callers a stable constructor as the real shell
/// leaf grows environment and policy configuration.
#[derive(Debug, Default)]
pub struct ShellState;

impl ShellState {
    /// Creates a shell leaf state with default local process settings.
    pub fn new() -> Self {
        Self
    }
}

/// Per-hook native child process state.
///
/// Hook routing is retained by the generated runtime. This state only owns the child
/// process and stream lifecycle so dropping a session cannot leave a shell orphaned.
struct ShellSession {
    _hook_id: HookID,
    child: Child,
    stdin_closed: bool,
}

impl ShellSession {
    /// Starts the user's interactive shell for one routed session.
    ///
    /// `/bin/bash` matches the original shell leaf sketch. This should eventually be
    /// made configurable at `ShellState`, but hard-coding it here keeps the current
    /// migration focused on the session API instead of broadening shell policy.
    fn spawn(hook_id: HookID) -> Result<Self, SessionInitError> {
        let child = Command::new("/bin/bash")
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|_| SessionInitError::rejected())?;

        Ok(Self {
            _hook_id: hook_id,
            child,
            stdin_closed: false,
        })
    }

    /// Closes the child's stdin once callers finish writing to the session.
    fn close_stdin(&mut self) {
        self.stdin_closed = true;
        let _ = self.child.stdin.take();
    }
}

impl Drop for ShellSession {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            return;
        }

        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Session<ShellState> for ShellSession {
    const PROCEDURE_ID: u32 = SESSION_ID_HASH;

    fn init(_leaf: &mut ShellState, packet: Packet) -> Result<Self, SessionInitError> {
        Self::spawn(packet.hook_id)
    }

    fn update(
        _leaf: &mut ShellState,
        session: &mut Self,
        incoming: &mut PacketQueue,
        _endpoint: &mut Endpoint,
    ) -> SessionStatus {
        while !incoming.is_empty() {
            let packet = incoming.remove(0);
            if packet.end_hook {
                session.close_stdin();
            }

            if packet.data.is_empty() || session.stdin_closed {
                continue;
            }

            let Some(stdin) = session.child.stdin.as_mut() else {
                session.close_stdin();
                continue;
            };

            if stdin.write_all(&packet.data).is_err() {
                session.close_stdin();
            }
        }

        match session.child.try_wait() {
            Ok(Some(_)) | Err(_) => SessionStatus::Closed,
            Ok(None) => SessionStatus::Running,
        }
    }
}
