use std::fmt;
use std::io;

#[derive(Debug)]
pub enum ShellLeafError {
    Io(io::Error),
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
