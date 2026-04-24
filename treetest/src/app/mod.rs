//! Ratatui application shell for the protocol demo.

mod actions;
mod shell;
mod ui;

use ratatui::DefaultTerminal;

use crate::{
    model::{NodeId, Selection},
    scenarios::built_in_scenarios,
    sim::Simulation,
};

/// Errors returned by the TUI application.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Sim(#[from] crate::sim::SimError),
}

/// Starts the TUI application.
pub fn run() -> Result<(), AppError> {
    shell::run()
}

#[derive(Debug)]
struct App {
    scenarios: Vec<crate::model::ScenarioDefinition>,
    scenario_index: usize,
    simulation: Simulation,
    selection_index: usize,
    selections: Vec<Selection>,
    status: String,
}
