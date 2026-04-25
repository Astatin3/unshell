//! Ratatui application shell for the protocol demo.
//!
//! The `app` module only defines the high-level pieces and re-exports the entry
//! point. The actual behavior is split into shell, actions, and UI modules so
//! the control flow reads from broad orchestration down to specific rendering.

mod actions;
mod shell;
mod ui;

use ratatui::DefaultTerminal;

use crate::{
    model::{NodeId, ScenarioDefinition, Selection},
    scenarios::built_in_scenarios,
    sim::Simulation,
};

/// Errors returned by the TUI application.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// Terminal setup, teardown, or input/output failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// The simulator rejected an operation or could not advance.
    #[error(transparent)]
    Sim(#[from] crate::sim::SimError),
}

/// Starts the TUI application.
pub fn run() -> Result<(), AppError> {
    shell::run()
}

#[derive(Debug)]
struct App {
    scenarios: Vec<ScenarioDefinition>,
    scenario_index: usize,
    simulation: Simulation,
    selection_index: usize,
    selections: Vec<Selection>,
    status: String,
}
