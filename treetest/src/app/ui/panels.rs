//! Non-inspector UI panels.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use crate::{
    model::{Selection, format_hook_ref, format_leaf_ref, format_path},
    sim::{InspectorMode, RecordedEvent, Simulation},
};

use super::super::App;

impl App {
    pub(crate) fn render(&self, frame: &mut Frame<'_>) {
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
        let mode = match self.simulation.inspector_mode {
            InspectorMode::GroundTruth => "ground truth",
            InspectorMode::Realistic => "realistic",
        };
        let title = format!(
            "treetest | scenario {} / {}: {} | {}",
            self.scenario_index + 1,
            self.scenarios.len(),
            self.scenarios[self.scenario_index].name,
            mode
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
                    Selection::Leaf { node_id, leaf_name } => format!(
                        "{} {}",
                        if index == self.selection_index {
                            ">"
                        } else {
                            " "
                        },
                        format_leaf_ref(&self.simulation.node(*node_id).path, leaf_name)
                    ),
                };
                ListItem::new(label)
            })
            .collect::<Vec<_>>();
        frame.render_widget(
            List::new(items).block(Block::default().borders(Borders::ALL).title("Tree")),
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
                    "{} -> {} [{}] {}",
                    format_hook_ref(&hook.host_path, hook.hook_id),
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
                "Keys: arrows move selection/scenario | i introspect | e echo leaf | p ping | c chunked | h open chat | d chat data | b chat bye | f invalid peer | g toggle ground-truth/realistic | m clear deeper memory + realistic | s step | a autoplay | q quit",
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
            Paragraph::new(help.into_iter().collect::<ratatui::text::Text<'_>>())
                .block(Block::default().borders(Borders::ALL).title("Status"))
                .wrap(Wrap { trim: true }),
            area,
        );
    }
}

pub(crate) fn build_selections(simulation: &Simulation) -> Vec<Selection> {
    let mut selections = Vec::new();
    let node_ids: Vec<_> = match simulation.inspector_mode {
        InspectorMode::GroundTruth => simulation.tree.nodes.iter().map(|node| node.id).collect(),
        InspectorMode::Realistic => simulation
            .root_knowledge
            .known_paths()
            .into_iter()
            .filter_map(|path| simulation.tree.find_by_path(&path))
            .collect(),
    };

    for node_id in node_ids {
        let node = simulation.node(node_id);
        selections.push(Selection::Node(node.id));
        match simulation.inspector_mode {
            InspectorMode::GroundTruth => {
                for leaf in &node.leaves {
                    selections.push(Selection::Leaf {
                        node_id: node.id,
                        leaf_name: leaf.name.clone(),
                    });
                }
            }
            InspectorMode::Realistic => {
                if let Some(learned) = simulation.root_knowledge.node(&node.path) {
                    for leaf in &learned.leaves {
                        selections.push(Selection::Leaf {
                            node_id: node.id,
                            leaf_name: leaf.leaf_name.clone(),
                        });
                    }
                }
            }
        }
    }
    selections
}
