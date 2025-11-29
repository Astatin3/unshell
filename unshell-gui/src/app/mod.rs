mod app;
mod windows;

use std::collections::HashMap;

use crate::{app::windows::WindowWrapper, auth::Auth, config::Config, flowchart::FlowChart};
pub use app::TemplateApp;
use egui_tiles::{TileId, Tree};

#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct AppState {
    pub auth: Auth,

    pub open_windows: HashMap<AppWindow, TileId>,

    pub flowchart: FlowChart,
    pub config: Config,
}

impl AppState {
    pub fn labels(&mut self, tree: &mut Tree<WindowWrapper>, ui: &mut egui::Ui) {
        for (_, (key, name)) in (vec![
            (AppWindow::Flowchart, "Flowchart"),
            (AppWindow::Config, "Config"),
        ])
        .iter()
        .enumerate()
        {
            let enabled = self.open_windows.contains_key(&key);

            if ui.selectable_label(enabled, *name).clicked() {
                if enabled {
                    let tid = *self.open_windows.get(&key).unwrap();
                    tree.remove_recursively(tid);
                    tree.tiles.remove(tid);
                    self.open_windows.remove(&key);

                    // if self.open_windows.is_empty()
                } else {
                    let tid = tree.tiles.insert_pane(WindowWrapper {
                        name: name.to_string(),
                        window: *key,
                    });

                    match self.open_windows.len() {
                        0 => {
                            tree.root = Some(tid);
                        }
                        1 => {
                            let old_root = tree.root.unwrap();
                            let tab_id = tree.tiles.insert_tab_tile(vec![old_root, tid]);
                            tree.root = Some(tab_id);
                            tree.make_active(|t, _| t == tid);
                        }
                        _ => {
                            let root = tree.root().unwrap();
                            let n = tree.tiles.get_container(root).unwrap().num_children();
                            tree.move_tile_to_container(tid, tree.root.unwrap().clone(), n, true);
                        }
                    }
                    self.open_windows.insert(key.clone(), tid);
                }
            }
        }
    }
}

#[derive(Clone, Copy, serde::Deserialize, serde::Serialize, PartialEq, Eq, Hash)]
pub enum AppWindow {
    Flowchart,
    Config,
}

impl AppWindow {
    fn update(&self, state: &mut AppState, ui: &mut egui::Ui) {
        match self {
            AppWindow::Flowchart => state.flowchart.paint(ui),
            AppWindow::Config => state.config.update(&mut state.auth, ui),
        }
    }

    fn render_title_buttons(&self, state: &mut AppState, ui: &mut egui::Ui) {
        match self {
            AppWindow::Flowchart => {
                if ui.button("Arrange").clicked() {
                    state.flowchart.arrange();
                }
            }
            _ => {
                ui.label("");
            }
        }
    }
}
