//! # Session Management
//!
//! A `Session` represents an active connection context to a specific node path.
//!
//! The operator can have multiple named sessions open simultaneously. Each session
//! has a "current path" (e.g., `/agents/abc123`) that prefixes commands.
//! Sessions can be backgrounded and switched between without disconnecting.
//!
//! ## Session lifecycle
//!
//! ```text
//! connect → handshake → session created
//!         ↓
//!    use agents/abc123   ← sets current_path
//!         ↓
//!    call shell/exec     ← sends to /agents/abc123/shell/exec
//!         ↓
//!    background          ← pushed to session list, detached
//!         ↓
//!    sessions            ← lists all active sessions
//!         ↓
//!    use <session_id>    ← reattaches
//! ```

/// A named, backgroundable session context.
#[derive(Debug, Clone)]
pub struct Session {
    /// Human-readable name (e.g., "abc123" or "session-1").
    pub name: String,
    /// The current working path (e.g., `/agents/abc123`).
    pub current_path: String,
    /// Whether this session is in the foreground.
    pub active: bool,
}

impl Session {
    /// Create a new session at the given path.
    #[must_use]
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            current_path: path.into(),
            active: true,
        }
    }

    /// Return the full path for a sub-path command.
    ///
    /// If `sub_path` is absolute (starts with `/`), return it unchanged.
    /// Otherwise, append it to `current_path`.
    ///
    /// # Example
    ///
    /// ```rust
    /// let sess = Session::new("abc123", "/agents/abc123");
    /// assert_eq!(sess.resolve("shell/exec"), "/agents/abc123/shell/exec");
    /// assert_eq!(sess.resolve("/router/nodes"), "/router/nodes");
    /// ```
    #[must_use]
    pub fn resolve(&self, sub_path: &str) -> String {
        if sub_path.starts_with('/') {
            sub_path.to_owned()
        } else {
            format!("{}/{sub_path}", self.current_path.trim_end_matches('/'))
        }
    }
}
