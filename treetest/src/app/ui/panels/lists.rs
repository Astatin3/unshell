//! Tree and selection list rendering.

use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, List, ListItem},
};

use crate::{
    model::{Selection, format_leaf_ref},
    sim::{InspectorMode, Simulation},
};

use super::super::super::App;

impl App {
    pub(super) fn render_selection_list(&self, frame: &mut Frame<'_>, area: Rect) {
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
