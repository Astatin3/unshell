//! UI module entry point.
//!
//! Rendering is split into panel layout and inspector rendering so the tree
//! browser, trace panes, and learned-knowledge inspector can evolve separately.

mod inspector;
mod panels;

pub(crate) use panels::build_selections;
