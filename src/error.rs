//! Error handling for unshell modules.
//!
//! This module defines the `ModuleError` enum which provides a unified
//! error type for all operations in the unshell framework.
//!
//! # Error Categories
//!
//! - **Tree errors**: Errors related to tree operations (not found, message errors)
//! - **Object errors**: Type and method validation errors
//! - **System errors**: Library loading, linking, cryptographic errors
//! - **Data errors**: Serialization/deserialization errors
//!
//! # Usage
//!
//! ```rust
//! use unshell::error::{ModuleError, Result};
//!
//! fn example() -> Result<i32> {
//!     Err(ModuleError::Error("Something went wrong".into()))
//! }
//!
//! // Using ? operator with string conversion
//! fn example2() -> Result<i32> {
//!     let err: ModuleError = "error message".into();
//!     Err(err)
//! }
//! ```
//!
//! # Serialization
//!
//! ModuleError implements serde serialization, making it suitable for
//! cross-process communication and logging:
//!
//! ```rust
//! use unshell::error::ModuleError;
//! use serde_json;
//!
//! let error = ModuleError::TreeMessageError("Invalid path".to_string());
//! let json = serde_json::to_string(&error).unwrap();
//! ```

use std::fmt;

pub type Result<T> = std::result::Result<T, ModuleError>;

///Generic error type for module-related operations.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ModuleError {
    NoError,

    // Tree errors
    TreeNotExist,
    TreeMessageError(String),

    // Object errors
    UnsupportedMethod,
    InvalidType,

    LibLoadingError(String),
    // LogError(log::SetLoggerError),
    LinkError(String),
    CryptError(String),
    DatabaseError(String),
    SerdeJsonError(String),

    Error(String),
}

impl From<&str> for ModuleError {
    fn from(value: &str) -> Self {
        Self::Error(value.into())
    }
}

impl From<serde_json::Error> for ModuleError {
    fn from(value: serde_json::Error) -> Self {
        Self::SerdeJsonError(value.to_string())
    }
}

impl From<Box<dyn std::error::Error>> for ModuleError {
    fn from(value: Box<dyn std::error::Error>) -> Self {
        ModuleError::Error(value.to_string())
    }
}

impl std::error::Error for ModuleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }

    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        Some(self)
    }
}

impl fmt::Display for ModuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(format!("{:?}", self).as_str())
    }
}

impl From<ModuleError> for std::string::String {
    fn from(value: ModuleError) -> Self {
        value.to_string()
    }
}
