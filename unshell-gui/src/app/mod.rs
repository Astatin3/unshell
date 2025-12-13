mod app;
mod windows;

use std::collections::HashSet;

use crate::{
    app::windows::WindowWrapper, auth::Auth, config::Config, flowchart::FlowChart,
    log_viewer::LogViewer, payload_config::PayloadConfig,
};
pub use app::TemplateApp;
use egui_tiles::{TileId, Tree};

#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct AppState {
    pub auth: Auth,

    pub open_windows: HashSet<AppWindow>,

    pub flowchart: FlowChart,
    pub config: Config,
    pub payload_config: PayloadConfig,
    pub log_viewer: LogViewer,
}

impl AppState {
    pub fn labels(&mut self, tree: &mut Tree<WindowWrapper>, ui: &mut egui::Ui) {
        for (_, (key, name)) in (vec![
            (AppWindow::Flowchart, "Flowchart"),
            (AppWindow::PayloadConfig, "Payload Config"),
            (AppWindow::Config, "Config"),
            (AppWindow::LogViewer, "Log Viewer"),
        ])
        .iter()
        .enumerate()
        {
            let enabled = self.open_windows.contains(&key);

            if ui.selectable_label(enabled, *name).clicked() {
                if enabled {
                    self.close_window(tree, key);
                } else {
                    self.open_window(tree, key, name);
                }
            }
        }
    }

    pub fn close_window(&mut self, tree: &mut Tree<WindowWrapper>, key: &AppWindow) {
        match Self::find_pane_id(*key, tree) {
            Some(tid) => {
                let tid = tid.clone();
                tree.remove_recursively(tid);
                tree.tiles.remove(tid);
                self.open_windows.remove(&key);
            }
            None => unreachable!(),
        }
    }

    pub fn open_window(&mut self, tree: &mut Tree<WindowWrapper>, key: &AppWindow, name: &str) {
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
        self.open_windows.insert(key.clone());
    }

    fn find_pane_id(window_type: AppWindow, tree: &Tree<WindowWrapper>) -> Option<&TileId> {
        for (tid, window) in tree.tiles.iter() {
            match window {
                egui_tiles::Tile::Pane(pane) => {
                    if pane.window == window_type {
                        return Some(tid);
                    }
                }
                egui_tiles::Tile::Container(_) => {}
            }
        }
        None
    }
}

#[derive(Clone, Copy, serde::Deserialize, serde::Serialize, PartialEq, Eq, Hash)]
pub enum AppWindow {
    Flowchart,
    Config,
    PayloadConfig,
    LogViewer,
}

impl AppWindow {
    fn update(&self, state: &mut AppState, ui: &mut egui::Ui) {
        match self {
            AppWindow::Flowchart => state.flowchart.paint(ui),
            AppWindow::Config => state.config.update(&mut state.auth, ui),
            AppWindow::PayloadConfig => state.payload_config.update(ui),
            AppWindow::LogViewer => state.log_viewer.update(&mut state.auth, ui),
        }
    }

    fn render_title_buttons(&self, state: &mut AppState, ui: &mut egui::Ui) {
        match self {
            AppWindow::Flowchart => {
                state.flowchart.titlebar_buttons(ui);
            }
            AppWindow::Config => {
                state.config.titlebar_buttons(ui);
            }
            _ => {
                ui.label("");
            }
        }
    }
}
