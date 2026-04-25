//! Canonical documented Rust file example.
//!
//! This example exists as a style reference for the repository. It shows the
//! minimum level of explanation expected in a real source file:
//!
//! - module-level documentation
//! - public API rustdoc
//! - doctest examples
//! - field-level documentation
//! - short rationale comments for non-obvious decisions
//! - a small internal test module
//!
//! The implementation itself is intentionally simple. The point is the shape of
//! the file, not the sophistication of the algorithm.

use std::collections::VecDeque;

/// Rolling event log with a fixed visible capacity.
///
/// The oldest entries fall off once capacity is reached. This is a common shape
/// in terminal UIs and tracing tools where recent context matters most.
///
/// # Example
/// ```rust
/// let mut log = documented_rust_file::RollingLog::new(2);
/// log.push("alpha");
/// log.push("beta");
/// log.push("gamma");
/// assert_eq!(log.entries(), ["beta", "gamma"]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollingLog {
    /// Maximum number of entries kept in memory.
    capacity: usize,
    /// Recent entries ordered from oldest to newest.
    entries: VecDeque<String>,
}

impl RollingLog {
    /// Creates a new rolling log with the given capacity.
    ///
    /// # Panics
    ///
    /// Panics if `capacity` is zero.
    ///
    /// # Example
    /// ```rust
    /// let log = documented_rust_file::RollingLog::new(3);
    /// assert_eq!(log.len(), 0);
    /// ```
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "rolling log capacity must be non-zero");

        Self {
            capacity,
            entries: VecDeque::with_capacity(capacity),
        }
    }

    /// Appends one entry, evicting the oldest entry when full.
    ///
    /// # Example
    /// ```rust
    /// let mut log = documented_rust_file::RollingLog::new(2);
    /// log.push("first");
    /// log.push("second");
    /// assert_eq!(log.len(), 2);
    /// ```
    pub fn push(&mut self, entry: impl Into<String>) {
        // Rationale: trim before push so capacity is never exceeded, even
        // transiently, which keeps later invariants and tests straightforward.
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(entry.into());
    }

    /// Returns the number of currently stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when there are no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the stored entries as a freshly owned vector.
    ///
    /// Rationale: this returns owned `String`s instead of references because it
    /// keeps the example easy to copy into UI code, serialization, or tests.
    pub fn entries(&self) -> Vec<String> {
        self.entries.iter().cloned().collect()
    }

    /// Returns the most recent entry, if any.
    pub fn newest(&self) -> Option<&str> {
        self.entries.back().map(String::as_str)
    }
}

/// Formats a small status line suitable for logs or demos.
///
/// # Example
/// ```rust
/// assert_eq!(documented_rust_file::format_status("sync", true), "sync: ok");
/// assert_eq!(documented_rust_file::format_status("sync", false), "sync: pending");
/// ```
pub fn format_status(label: &str, done: bool) -> String {
    let suffix = if done { "ok" } else { "pending" };
    format!("{label}: {suffix}")
}

/// Small executable entry point so Cargo can build this file as an example.
fn main() {
    let mut log = RollingLog::new(2);
    log.push(format_status("example", true));

    // Printing one line keeps the example executable visible without distracting
    // from the file's main purpose as a documentation template.
    if let Some(line) = log.newest() {
        println!("{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::{RollingLog, format_status};

    #[test]
    fn rolling_log_evicts_oldest_entry() {
        let mut log = RollingLog::new(2);
        log.push("alpha");
        log.push("beta");
        log.push("gamma");

        assert_eq!(log.entries(), ["beta", "gamma"]);
        assert_eq!(log.newest(), Some("gamma"));
    }

    #[test]
    fn status_helper_is_stable() {
        assert_eq!(format_status("build", true), "build: ok");
        assert_eq!(format_status("build", false), "build: pending");
    }
}
