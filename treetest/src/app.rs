//! Ratatui application shell for the protocol demo.

use std::{io, time::Duration};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style, Stylize},
    text::{Line, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use crate::{
    model::{Selection, format_path},
    scenarios::built_in_scenarios,
    sim::{RecordedEvent, Simulation},
};

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
        let selections = build_selections(&simulation);
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
            KeyCode::Char('i') => {
                self.perform_introspection()?;
            }
            KeyCode::Char('e') => {
                self.perform_echo()?;
            }
            KeyCode::Char('p') => {
                self.perform_ping()?;
            }
            KeyCode::Char('c') => {
                self.perform_chunked()?;
            }
            KeyCode::Char('h') => {
                self.perform_chat_call()?;
            }
            KeyCode::Char('d') => {
                self.perform_chat_data()?;
            }
            KeyCode::Char('b') => {
                self.perform_chat_bye()?;
            }
            KeyCode::Char('f') => {
                self.perform_invalid_fault_demo()?;
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

    fn load_scenario(&mut self, index: usize) -> Result<(), AppError> {
        self.scenario_index = index;
        self.simulation = Simulation::new(self.scenarios[index].clone())?;
        self.selections = build_selections(&self.simulation);
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

    fn render(&self, frame: &mut Frame<'_>) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(14),
                Constraint::Length(8),
            ])
            .split(frame.area());

        self.render_header(frame, chunks[0]);
        self.render_body(frame, chunks[1]);
        self.render_footer(frame, chunks[2]);
    }

    fn render_header(&self, frame: &mut Frame<'_>, area: Rect) {
        let title = format!(
            "treetest | scenario {} / {}: {}",
            self.scenario_index + 1,
            self.scenarios.len(),
            self.scenarios[self.scenario_index].name
        );
        frame.render_widget(
            Paragraph::new(title).block(Block::default().borders(Borders::ALL).title("Scenario")),
            area,
        );
    }

    fn render_body(&self, frame: &mut Frame<'_>, area: Rect) {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(34),
                Constraint::Percentage(36),
                Constraint::Percentage(32),
            ])
            .split(area);

        let scenario_items = self
            .scenarios
            .iter()
            .enumerate()
            .map(|(index, scenario)| {
                let label = if index == self.scenario_index {
                    format!("> {}", scenario.name)
                } else {
                    format!("  {}", scenario.name)
                };
                ListItem::new(label)
            })
            .collect::<Vec<_>>();
        frame.render_widget(
            List::new(scenario_items)
                .block(Block::default().borders(Borders::ALL).title("Scenarios")),
            columns[0],
        );

        let center = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(54), Constraint::Percentage(46)])
            .split(columns[1]);
        self.render_selection_list(frame, center[0]);
        self.render_inspector(frame, center[1]);

        let right = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(columns[2]);
        self.render_trace(frame, right[0]);
        self.render_hooks(frame, right[1]);
    }

    fn render_selection_list(&self, frame: &mut Frame<'_>, area: Rect) {
        let items = self
            .selections
            .iter()
            .enumerate()
            .map(|(index, selection)| {
                let label = match selection {
                    Selection::Node(node_id) => {
                        let node = self.simulation.node(*node_id);
                        format!(
                            "{} {}",
                            if index == self.selection_index {
                                ">"
                            } else {
                                " "
                            },
                            node.display_path()
                        )
                    }
                    Selection::Leaf { node_id, leaf_name } => {
                        format!(
                            "{} {} :: {}",
                            if index == self.selection_index {
                                ">"
                            } else {
                                " "
                            },
                            self.simulation.node(*node_id).display_path(),
                            leaf_name
                        )
                    }
                };
                ListItem::new(label)
            })
            .collect::<Vec<_>>();
        frame.render_widget(
            List::new(items).block(Block::default().borders(Borders::ALL).title("Tree")),
            area,
        );
    }

    fn render_inspector(&self, frame: &mut Frame<'_>, area: Rect) {
        let selection = self.selected();
        let body = match selection {
            Selection::Node(node_id) => {
                let node = self.simulation.node(*node_id);
                let mut lines = vec![
                    Line::from(node.title.clone()).bold(),
                    Line::from(node.description.clone()),
                    Line::from(format!("Path: {}", node.display_path())),
                    Line::from(format!("Children: {}", node.children.len())),
                    Line::from(format!("Leaves: {}", node.leaves.len())),
                    Line::from(format!(
                        "Endpoint procedures: {}",
                        node.endpoint_procedures.len()
                    )),
                    Line::default(),
                    Line::from("Endpoint procedures:"),
                ];
                lines.extend(
                    node.endpoint_procedures
                        .iter()
                        .map(|procedure| Line::from(format!("- {}", procedure.procedure_id))),
                );
                lines.extend(
                    node.leaves
                        .iter()
                        .map(|leaf| Line::from(format!("- leaf {}", leaf.name))),
                );
                Text::from(lines)
            }
            Selection::Leaf { node_id, leaf_name } => {
                let node = self.simulation.node(*node_id);
                let leaf = node
                    .leaves
                    .iter()
                    .find(|leaf| &leaf.name == leaf_name)
                    .expect("selection should stay valid");
                Text::from(vec![
                    Line::from(format!("Leaf {}", leaf.name)).bold(),
                    Line::from(leaf.description.clone()),
                    Line::from(format!("Node: {}", node.display_path())),
                    Line::from(format!("Procedures: {}", leaf.procedures.join(", "))),
                ])
            }
        };

        frame.render_widget(
            Paragraph::new(body)
                .block(Block::default().borders(Borders::ALL).title("Inspector"))
                .wrap(Wrap { trim: true }),
            area,
        );
    }

    fn render_trace(&self, frame: &mut Frame<'_>, area: Rect) {
        let items = self
            .simulation
            .trace
            .iter()
            .rev()
            .take(12)
            .map(|event| {
                ListItem::new(format!(
                    "#{:03} {} | {}",
                    event.tick, event.node_path, event.summary
                ))
            })
            .collect::<Vec<_>>();
        frame.render_widget(
            List::new(items).block(Block::default().borders(Borders::ALL).title("Trace")),
            area,
        );
    }

    fn render_hooks(&self, frame: &mut Frame<'_>, area: Rect) {
        let items = self
            .simulation
            .hooks
            .values()
            .map(|hook| {
                let status = if hook.closed { "closed" } else { "open" };
                ListItem::new(format!(
                    "#{} {} -> {} [{}] {}",
                    hook.hook_id,
                    format_path(&hook.host_path),
                    format_path(&hook.peer_path),
                    status,
                    hook.last_message,
                ))
            })
            .collect::<Vec<_>>();
        frame.render_widget(
            List::new(items).block(Block::default().borders(Borders::ALL).title("Hooks")),
            area,
        );
    }

    fn render_footer(&self, frame: &mut Frame<'_>, area: Rect) {
        let help = vec![
            Line::from(self.status.clone()).style(Style::default().add_modifier(Modifier::BOLD)),
            Line::from(
                "Keys: arrows move selection/scenario | i introspect | e echo leaf | p ping | c chunked | h open chat | d chat data | b chat bye | f invalid peer | s step | a autoplay | q quit",
            ),
            Line::from(format!(
                "Current selection: {}",
                self.simulation.selection_summary(self.selected())
            )),
            Line::from(match self.simulation.recorded_events.last() {
                Some(RecordedEvent::Data {
                    node_path, message, ..
                }) => {
                    format!(
                        "Last local event: Data at {node_path} ({})",
                        String::from_utf8_lossy(&message.data)
                    )
                }
                Some(RecordedEvent::Fault {
                    node_path, message, ..
                }) => {
                    format!(
                        "Last local event: Fault at {node_path} (0x{:02X})",
                        message.fault.0
                    )
                }
                Some(RecordedEvent::Call {
                    node_path, message, ..
                }) => {
                    format!(
                        "Last local event: Call at {node_path} ({})",
                        message.procedure_id
                    )
                }
                None => "Last local event: none yet".to_owned(),
            }),
        ];

        frame.render_widget(
            Paragraph::new(Text::from(help))
                .block(Block::default().borders(Borders::ALL).title("Status"))
                .wrap(Wrap { trim: true }),
            area,
        );
    }
}

fn build_selections(simulation: &Simulation) -> Vec<Selection> {
    let mut selections = Vec::new();
    for node in &simulation.tree.nodes {
        selections.push(Selection::Node(node.id));
        for leaf in &node.leaves {
            selections.push(Selection::Leaf {
                node_id: node.id,
                leaf_name: leaf.name.clone(),
            });
        }
    }
    selections
}
