//! # CLI Module
//!
//! This module provides the interactive CLI for the unshell tree protocol testbed.
//! It supports both local tree operations and remote connections.
//!
//! # Usage
//!
//! ```no_run
//! use ush_treetest::cli::{Cli, parse_and_execute};
//!
//! let mut cli = Cli::new();
//! let output = parse_and_execute(&mut cli, "leaves").unwrap();
//! println!("{}", output);
//! ```

pub mod cli;

pub use cli::{Cli, parse_and_execute};