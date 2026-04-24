//! Crossbeam-backed protocol simulation.
//!
//! The simulator is split into focused submodules so protocol state, root
//! knowledge, action helpers, and runtime packet processing stay readable.

mod actions;
mod build;
mod knowledge;
mod runtime;
mod types;

pub use knowledge::{InspectorMode, LearnedLeaf, LearnedNode, LearnedProcedure, RootKnowledge};
pub use types::{ActionResult, HookSnapshot, RecordedEvent, SimError, Simulation, TraceEvent};
