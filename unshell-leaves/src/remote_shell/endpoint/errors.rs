use std::fmt;
use std::io;

/// Error produced by the remote shell endpoint implementation.
#[derive(Debug)]
pub enum ShellLeafError {
    /// Underlying PTY or I/O failure.
    Io(io::Error),
    /// Shell open requires a response hook so the session can stream bytes back.
    MissingHook,
}

impl fmt::Display for ShellLeafError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::MissingHook => f.write_str("shell open requires a response hook"),
        }
    }
}

impl std::error::Error for ShellLeafError {}

impl From<io::Error> for ShellLeafError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
