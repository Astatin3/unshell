//! Ratatui application shell for the protocol demo.

mod ui;

use std::{io, time::Duration};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::DefaultTerminal;

use crate::{model::Selection, scenarios::built_in_scenarios, sim::Simulation};

/// Errors returned by the TUI application.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Sim(#[from] crate::sim::SimError),
}

/// Starts the TUI application.
pub fn run() -> Result<(), AppError> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let terminal = ratatui::init();
    let result = App::new()?.run(terminal);
    ratatui::restore();
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    result
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

impl App {
    fn new() -> Result<Self, AppError> {
        let scenarios = built_in_scenarios();
        let simulation = Simulation::new(scenarios[0].clone())?;
        let selections = ui::build_selections(&simulation);
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

    fn run(mut self, mut terminal: DefaultTerminal) -> Result<(), AppError> {
        loop {
            terminal.draw(|frame| self.render(frame))?;
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

    fn handle_key(&mut self, code: KeyCode) -> Result<bool, AppError> {
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

    fn load_scenario(&mut self, index: usize) -> Result<(), AppError> {
        self.scenario_index = index;
        self.simulation = Simulation::new(self.scenarios[index].clone())?;
        self.selections = ui::build_selections(&self.simulation);
        self.selection_index = self
            .selections
            .iter()
            .position(|selection| *selection == self.simulation.initial_selection())
            .unwrap_or(0);
        self.status = format!("Loaded scenario: {}", self.scenarios[index].name);
        Ok(())
    }

    fn selected(&self) -> &Selection {
        &self.selections[self.selection_index]
    }

    fn perform_introspection(&mut self) -> Result<(), AppError> {
        match self.selected().clone() {
            Selection::Node(node_id) => {
                let result = self.simulation.call_endpoint_introspection(node_id)?;
                let steps = self.simulation.drain()?;
                self.status = format!("{} ({steps} steps)", result.label);
            }
            Selection::Leaf { node_id, leaf_name } => {
                let result = self
                    .simulation
                    .call_leaf_introspection(node_id, &leaf_name)?;
                let steps = self.simulation.drain()?;
                self.status = format!("{} ({steps} steps)", result.label);
            }
        }
        Ok(())
    }

    fn perform_echo(&mut self) -> Result<(), AppError> {
        if let Selection::Leaf { node_id, leaf_name } = self.selected().clone() {
            let result =
                self.simulation
                    .call_echo_leaf(node_id, &leaf_name, "demo echo from root")?;
            let steps = self.simulation.drain()?;
            self.status = format!("{} ({steps} steps)", result.label);
        } else {
            self.status = "Select a leaf first, then press e.".to_owned();
        }
        Ok(())
    }

    fn perform_ping(&mut self) -> Result<(), AppError> {
        if let Selection::Node(node_id) = self.selected().clone() {
            if let Some(procedure_id) = self
                .simulation
                .node(node_id)
                .endpoint_procedures
                .first()
                .map(|procedure| procedure.procedure_id.clone())
            {
                let result = self.simulation.call_endpoint_procedure(
                    node_id,
                    &procedure_id,
                    b"ping".to_vec(),
                )?;
                let steps = self.simulation.drain()?;
                self.status = format!("{} ({steps} steps)", result.label);
            } else {
                self.status = "Selected node has no endpoint procedures.".to_owned();
            }
        } else {
            self.status = "Select a node first, then press p.".to_owned();
        }
        Ok(())
    }

    fn perform_chunked(&mut self) -> Result<(), AppError> {
        if let Selection::Node(node_id) = self.selected().clone() {
            if let Some(procedure_id) = self
                .simulation
                .node(node_id)
                .endpoint_procedures
                .iter()
                .find(|procedure| {
                    procedure.description.contains("chunk")
                        || procedure.procedure_id.contains("chunked")
                })
                .map(|procedure| procedure.procedure_id.clone())
            {
                let result = self.simulation.call_endpoint_procedure(
                    node_id,
                    &procedure_id,
                    b"chunk please".to_vec(),
                )?;
                let steps = self.simulation.drain()?;
                self.status = format!("{} ({steps} steps)", result.label);
            } else {
                self.status = "Selected node has no chunked procedure.".to_owned();
            }
        } else {
            self.status = "Select a node first, then press c.".to_owned();
        }
        Ok(())
    }

    fn perform_chat_call(&mut self) -> Result<(), AppError> {
        if let Selection::Node(node_id) = self.selected().clone() {
            if let Some(procedure_id) = self
                .simulation
                .node(node_id)
                .endpoint_procedures
                .iter()
                .find(|procedure| procedure.procedure_id.contains("chat"))
                .map(|procedure| procedure.procedure_id.clone())
            {
                let result = self.simulation.call_endpoint_procedure(
                    node_id,
                    &procedure_id,
                    b"open chat".to_vec(),
                )?;
                let steps = self.simulation.drain()?;
                self.status = format!("{} ({steps} steps)", result.label);
            } else {
                self.status = "Selected node has no chat procedure.".to_owned();
            }
        } else {
            self.status = "Select a node first, then press h.".to_owned();
        }
        Ok(())
    }

    fn perform_chat_data(&mut self) -> Result<(), AppError> {
        if let Some(hook_id) = self.simulation.hook_ids().last().copied() {
            let result =
                self.simulation
                    .send_root_hook_data(hook_id, "hello from the root", false)?;
            let steps = self.simulation.drain()?;
            self.status = format!("{} ({steps} steps)", result.label);
        } else {
            self.status = "No known hook yet. Press h to open chat first.".to_owned();
        }
        Ok(())
    }

    fn perform_chat_bye(&mut self) -> Result<(), AppError> {
        if let Some(hook_id) = self.simulation.hook_ids().last().copied() {
            let result = self.simulation.send_root_hook_data(hook_id, "bye", true)?;
            let steps = self.simulation.drain()?;
            self.status = format!("{} ({steps} steps)", result.label);
        } else {
            self.status = "No known hook yet. Press h to open chat first.".to_owned();
        }
        Ok(())
    }

    fn perform_invalid_fault_demo(&mut self) -> Result<(), AppError> {
        if let Some(hook_id) = self.simulation.hook_ids().last().copied() {
            let root_id = crate::model::NodeId(0);
            if self.simulation.tree.nodes.len() > 1 {
                let attacker = crate::model::NodeId(1);
                let result = self.simulation.inject_invalid_peer_data(
                    attacker,
                    root_id,
                    hook_id,
                    "demo.endpoint.v1.chat.session",
                    "spoofed data",
                )?;
                let steps = self.simulation.drain()?;
                self.status = format!("{} ({steps} steps)", result.label);
            } else {
                self.status =
                    "This scenario has no second node for invalid-peer traffic.".to_owned();
            }
        } else {
            self.status = "Open a hook first before injecting invalid traffic.".to_owned();
        }
        Ok(())
    }
}
