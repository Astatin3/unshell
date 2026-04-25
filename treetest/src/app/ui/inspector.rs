//! Inspector rendering.
//!
//! The inspector has the most view-specific branching because it renders either
//! ground-truth scenario metadata or the root host's learned knowledge model.

use ratatui::{
    Frame,
    prelude::Stylize,
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::{
    model::{Selection, format_leaf_ref, format_path},
    sim::InspectorMode,
};

use super::super::App;

impl App {
    /// Renders the inspector pane for the current selection.
    ///
    /// Rationale: the inspector is the only pane whose data source changes with
    /// inspector mode, so it owns the `ground truth` vs `realistic` branch.
    pub(super) fn render_inspector(&self, frame: &mut Frame<'_>, area: ratatui::layout::Rect) {
        let selection = self.selected();
        let body = match self.simulation.inspector_mode {
            InspectorMode::GroundTruth => self.render_ground_truth_inspector(selection),
            InspectorMode::Realistic => self.render_realistic_inspector(selection),
        };

        frame.render_widget(
            Paragraph::new(body)
                .block(Block::default().borders(Borders::ALL).title("Inspector"))
                .wrap(Wrap { trim: true }),
            area,
        );
    }

    /// Renders the inspector using full scenario truth.
    fn render_ground_truth_inspector(&self, selection: &Selection) -> Text<'static> {
        match selection {
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
                lines.extend(node.endpoint_procedures.iter().map(|procedure| {
                    Line::from(format!(
                        "- {}: {}",
                        procedure.procedure_id, procedure.description
                    ))
                }));
                lines.extend(node.leaves.iter().map(|leaf| {
                    Line::from(format!("- {}", format_leaf_ref(&node.path, &leaf.name)))
                }));
                Text::from(lines)
            }
            Selection::Leaf { node_id, leaf_name } => {
                let node = self.simulation.node(*node_id);
                let leaf = node
                    .leaves
                    .iter()
                    .find(|leaf| &leaf.name == leaf_name)
                    .expect("selection should stay valid");
                let mut lines = vec![
                    Line::from(format_leaf_ref(&node.path, &leaf.name)).bold(),
                    Line::from(leaf.description.clone()),
                    Line::from(format!("Node: {}", node.display_path())),
                    Line::from("Procedures:"),
                ];
                lines.extend(
                    leaf.procedures
                        .iter()
                        .map(|procedure| Line::from(format!("- {}", procedure))),
                );
                Text::from(lines)
            }
        }
    }

    /// Renders the inspector using only what the root host has learned.
    fn render_realistic_inspector(&self, selection: &Selection) -> Text<'static> {
        match selection {
            Selection::Node(node_id) => {
                let node = self.simulation.node(*node_id);
                if let Some(learned) = self.simulation.root_knowledge.node(&node.path) {
                    let mut lines = vec![
                        Line::from(learned.title.clone().unwrap_or_else(|| node.display_path()))
                            .bold(),
                        Line::from(
                            learned
                                .description
                                .clone()
                                .unwrap_or_else(|| "No learned description yet.".to_owned()),
                        ),
                        Line::from(format!("Path: {}", format_path(&learned.path))),
                        Line::from(format!("Known direct child: {}", learned.direct_child)),
                        Line::from(format!(
                            "Endpoint introspected: {}",
                            learned.endpoint_introspected
                        )),
                        Line::default(),
                        Line::from("Known endpoint procedures:"),
                    ];
                    if learned.endpoint_procedures.is_empty() {
                        lines.push(Line::from("- none learned"));
                    } else {
                        lines.extend(learned.endpoint_procedures.iter().map(|procedure| {
                            Line::from(match &procedure.description {
                                Some(description) => {
                                    format!("- {}: {}", procedure.procedure_id, description)
                                }
                                None => format!("- {}", procedure.procedure_id),
                            })
                        }));
                    }
                    lines.push(Line::default());
                    lines.push(Line::from("Known leaves:"));
                    if learned.leaves.is_empty() {
                        lines.push(Line::from("- none learned"));
                    } else {
                        lines.extend(learned.leaves.iter().map(|leaf| {
                            Line::from(format!(
                                "- {}",
                                format_leaf_ref(&learned.path, &leaf.leaf_name)
                            ))
                        }));
                    }
                    Text::from(lines)
                } else {
                    // Showing an explicit unknown state is better than silently
                    // falling back to ground truth, because the whole point of
                    // realistic mode is to expose what the root does not know.
                    Text::from(vec![
                        Line::from(node.display_path()).bold(),
                        Line::from(
                            "The root host has not learned anything about this endpoint yet.",
                        ),
                    ])
                }
            }
            Selection::Leaf { node_id, leaf_name } => {
                let node = self.simulation.node(*node_id);
                if let Some(learned) = self.simulation.root_knowledge.node(&node.path)
                    && let Some(leaf) = learned
                        .leaves
                        .iter()
                        .find(|leaf| &leaf.leaf_name == leaf_name)
                {
                    let mut lines = vec![
                        Line::from(format_leaf_ref(&node.path, &leaf.leaf_name)).bold(),
                        Line::from(
                            leaf.description
                                .clone()
                                .unwrap_or_else(|| "No learned description yet.".to_owned()),
                        ),
                        Line::from(format!("Node: {}", node.display_path())),
                        Line::from("Known procedures:"),
                    ];
                    if leaf.procedures.is_empty() {
                        lines.push(Line::from("- none learned"));
                    } else {
                        lines.extend(leaf.procedures.iter().map(|procedure| {
                            Line::from(match &procedure.description {
                                Some(description) => {
                                    format!("- {}: {}", procedure.procedure_id, description)
                                }
                                None => format!("- {}", procedure.procedure_id),
                            })
                        }));
                    }
                    Text::from(lines)
                } else {
                    Text::from(vec![
                        Line::from(format_leaf_ref(&node.path, leaf_name)).bold(),
                        Line::from("The root host has not learned this leaf yet."),
                    ])
                }
            }
        }
    }
}
