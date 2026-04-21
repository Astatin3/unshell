//! # Content Type Constants
//!
//! Content types describe how to interpret the `data` field of a
//! [`TreeRequest`](super::TreeRequest) or [`TreeResponse`](super::TreeResponse).
//!
//! They follow a `"namespace/TypeName"` convention, similar to MIME types.
//!
//! ## Built-in types
//!
//! | Constant | Value | Meaning |
//! |---|---|---|
//! | [`NONE`] | `"core/None"` | No data (empty payload) |
//! | [`UTF8_STRING`] | `"core/Utf8String"` | Raw UTF-8 string |
//! | [`BYTES`] | `"core/Bytes"` | Raw bytes (no specific interpretation) |
//! | [`PROCEDURE_LIST`] | `"core/ProcedureList"` | rkyv-serialised `Vec<ProcedureDescriptor>` |
//!
//! ## Custom types
//!
//! Module authors should prefix with their module name:
//!
//! ```rust
//! const MY_TYPE: &str = "mymodule/MyType";
//! ```

/// No data. Use for requests/responses that carry no payload.
///
/// # Example
///
/// ```rust
/// use unshell::protocol::{TreeRequest, RequestType, content};
///
/// // A ping-style read with no payload
/// let req = TreeRequest {
///     request_id: 1,
///     request_type: RequestType::Read,
///     content_type: content::NONE.into(),
///     data: Vec::new(),
/// };
/// ```
pub const NONE: &str = "core/None";

/// A raw UTF-8 string.
///
/// The `data` field contains the string's bytes (no null terminator, no length prefix).
pub const UTF8_STRING: &str = "core/Utf8String";

/// Raw bytes with no specific interpretation.
pub const BYTES: &str = "core/Bytes";

/// A rkyv-serialised `Vec<ProcedureDescriptor>`.
///
/// Used in responses to [`RequestType::GetProcedures`](super::RequestType::GetProcedures).
pub const PROCEDURE_LIST: &str = "core/ProcedureList";

/// Shell command output: UTF-8 stdout and stderr combined.
pub const SHELL_OUTPUT: &str = "shell/Output";

/// Raw file contents as bytes.
pub const FILE_BYTES: &str = "files/Bytes";
