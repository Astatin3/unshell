//! Application lifecycle and event loop glue.
//!
//! This module owns terminal setup/teardown and the high-level event loop. The
//! rest of the app modules assume they run inside this shell and therefore do
//! not repeat raw-mode or alternate-screen management logic.

use std::{io, time::Duration};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

use super::{App, AppError, DefaultTerminal, NodeId, built_in_scenarios, ui};

/// Boots the terminal UI and guarantees cleanup on exit.
pub(super) fn run() -> Result<(), AppError> {
    // Enter raw mode first so every later keypress is visible to the app.
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    // ratatui wraps the terminal backend after the alternate screen is active.
    let terminal = ratatui::init();
    let result = App::new()?.run(terminal);

    // Restore terminal state even when the app exits through an error path.
    ratatui::restore();
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    result
}

impl App {
    /// Creates the initial application state.
    ///
    /// The first built-in scenario is loaded immediately so the user sees a
    /// working demo as soon as the TUI opens.
    pub(super) fn new() -> Result<Self, AppError> {
        let scenarios = built_in_scenarios();

        // Start on the first scenario rather than waiting for manual selection.
        let simulation = crate::sim::Simulation::new(scenarios[0].clone())?;

        // Build visible rows from the current inspector mode.
        let selections = ui::build_selections(&simulation);

        // Prefer the scenario's declared initial focus when available.
        let selection_index = selections
            .iter()
            .position(|selection| *selection == simulation.initial_selection())
            .unwrap_or(0);

        Ok(Self {
            scenarios,
            scenario_index: 0,
            simulation,
            selection_index,
            selections,
            status: "Use arrows to move, Enter to switch scenarios, q to quit.".to_owned(),
        })
    }

    /// Runs the main draw/poll loop.
    pub(super) fn run(mut self, mut terminal: DefaultTerminal) -> Result<(), AppError> {
        loop {
            terminal.draw(|frame| self.render(frame))?;

            // Poll with a timeout so redraws stay responsive without busy-spinning.
            if event::poll(Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
                && !self.handle_key(key.code)?
            {
                break;
            }
        }
        Ok(())
    }

    /// Routes one keypress into one app action.
    pub(super) fn handle_key(&mut self, code: KeyCode) -> Result<bool, AppError> {
        match code {
            KeyCode::Char('q') => return Ok(false),
            KeyCode::Up => {
                if self.selection_index > 0 {
                    self.selection_index -= 1;
                }
            }
            KeyCode::Down => {
                if self.selection_index + 1 < self.selections.len() {
                    self.selection_index += 1;
                }
            }
            KeyCode::Left => {
                if self.scenario_index > 0 {
                    self.load_scenario(self.scenario_index - 1)?;
                }
            }
            KeyCode::Right => {
                if self.scenario_index + 1 < self.scenarios.len() {
                    self.load_scenario(self.scenario_index + 1)?;
                }
            }
            KeyCode::Enter => {
                // Enter cycles scenarios so the demo works even on keyboards
                // without convenient left/right usage in some terminals.
                let next = (self.scenario_index + 1) % self.scenarios.len();
                self.load_scenario(next)?;
            }
            KeyCode::Char('i') => self.perform_introspection()?,
            KeyCode::Char('e') => self.perform_echo()?,
            KeyCode::Char('p') => self.perform_ping()?,
            KeyCode::Char('c') => self.perform_chunked()?,
            KeyCode::Char('h') => self.perform_chat_call()?,
            KeyCode::Char('d') => self.perform_chat_data()?,
            KeyCode::Char('b') => self.perform_chat_bye()?,
            KeyCode::Char('f') => self.perform_invalid_fault_demo()?,
            KeyCode::Char('g') => {
                self.simulation.toggle_inspector_mode();

                // Rebuild rows because realistic mode can hide undiscovered nodes.
                self.refresh_selections(Some(self.selected().node_id()));
                self.status = if self.simulation.is_realistic_mode() {
                    "Inspector switched to realistic mode.".to_owned()
                } else {
                    "Inspector switched to ground truth mode.".to_owned()
                };
            }
            KeyCode::Char('m') => {
                self.simulation.enable_realistic_mode_with_memory_reset();

                // Jump to root because deeper selections may no longer be known.
                self.refresh_selections(Some(NodeId(0)));
                self.status =
                    "Cleared root memory for deeper nodes and enabled realistic mode.".to_owned();
            }
            KeyCode::Char('s') => {
                let processed = self.simulation.step()?;
                self.status = if processed {
                    "Processed one queued frame.".to_owned()
                } else {
                    "Network already idle.".to_owned()
                };
            }
            KeyCode::Char('a') => {
                let steps = self.simulation.drain()?;
                self.status = format!("Drained {steps} queued frames.");
            }
            _ => {}
        }
        Ok(true)
    }

    /// Replaces the active scenario with a fresh simulation.
    pub(super) fn load_scenario(&mut self, index: usize) -> Result<(), AppError> {
        self.scenario_index = index;

        // Rebuild from scratch so each scenario switch resets learned state,
        // trace history, and active hooks.
        self.simulation = crate::sim::Simulation::new(self.scenarios[index].clone())?;
        self.refresh_selections(Some(self.simulation.initial_selection().node_id()));
        self.status = format!("Loaded scenario: {}", self.scenarios[index].name);
        Ok(())
    }

    /// Returns the current tree selection.
    pub(super) fn selected(&self) -> &crate::model::Selection {
        &self.selections[self.selection_index]
    }

    /// Rebuilds the visible selection list and preserves focus when possible.
    ///
    /// Rationale: realistic mode can hide items that ground-truth mode showed,
    /// so selection repair needs to happen in one dedicated place.
    pub(super) fn refresh_selections(&mut self, preferred_node: Option<NodeId>) {
        // Prefer an explicit node if the caller knows what should stay selected.
        let current = preferred_node.unwrap_or_else(|| self.selected().node_id());
        self.selections = ui::build_selections(&self.simulation);

        // Fall back to the first row when the previous node disappeared.
        self.selection_index = self
            .selections
            .iter()
            .position(|selection| selection.node_id() == current)
            .unwrap_or(0);
    }
}
