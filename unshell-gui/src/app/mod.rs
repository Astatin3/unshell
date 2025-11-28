mod app;
mod windows;

use std::collections::HashMap;

pub use app::TemplateApp;
use egui_tiles::{TileId, Tree};
use log::info;

use crate::{app::windows::WindowWrapper, config::Config, flowchart::FlowChart};

#[derive(Default, serde::Deserialize, serde::Serialize)]
struct AppState {
    pub open_windows: HashMap<AppWindow, TileId>,

    pub flowchart: FlowChart,
    pub config: Config,
}

impl AppState {
    pub fn labels(&mut self, tree: &mut Tree<WindowWrapper>, ui: &mut egui::Ui) {
        for (i, (key, name)) in (vec![
            (AppWindow::Flowchart, "Flowchart"),
            (AppWindow::Config, "Config"),
        ])
        .iter()
        .enumerate()
        {
            let enabled = self.open_windows.contains_key(&key);

            if ui.selectable_label(enabled, *name).clicked() {
                // if enabled {
                //     let tid = *self.open_windows.get(&key).unwrap();
                //     tree.remove_recursively(tid);
                //     tree.tiles.remove(tid);
                //     self.open_windows.remove(&key);

                //     // if self.open_windows.is_empty()
                // } else {
                let tid = tree.tiles.insert_pane(WindowWrapper {
                    nr: i + 1,
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
                    }
                    _ => {
                        let pid = tree.tiles.insert_tab_tile(vec![tid]);
                        tree.move_tile_to_container(pid, tree.root.unwrap().clone(), 0, true);
                    }
                }

                self.open_windows.insert(key.clone(), tid);
            }
        }

        // if ui
        //     .selectable_label(
        //         self.open_windows.contains_key(&AppWindow::Flowchart),
        //         "Network",
        //     )
        //     .clicked()
        // {
        //     // self.open_windows. = Tab::Flowchart;
        // }

        // if ui
        //     .selectable_label(self.open_windows.contains_key(&AppWindow::Config), "Config")
        //     .clicked()
        // {
        //     // self.open_windows. = Tab::Flowchart;
        // }

        // if ui
        //     .selectable_label(self.tab == Tab::Test, self.config.title())
        //     .clicked()
        // {
        //     self.tab = Tab::Test;
        // }
    }

    // fn contains(&self, key: &AppWindow)

    // fn toggle(&mut self, key: &AppWindow) {
    //     if self.
    // }
}

// impl Default for AppState {
//     fn default() -> Self {
//         Self {
//             open_windows: HashMap::from([
//                 (AppWindow::Flowchart, false),
//                 (AppWindow::Config, false),
//             ]),

//             flowchart: Default::default(),
//             config: Default::default(),
//         }
//     }
// }

#[derive(Clone, Copy, serde::Deserialize, serde::Serialize, PartialEq, Eq, Hash)]
enum AppWindow {
    None,
    Flowchart,
    Config,
}

impl AppWindow {
    fn update(&self, state: &mut AppState, ui: &mut egui::Ui) {
        match self {
            AppWindow::None => {}
            AppWindow::Flowchart => state.flowchart.paint(ui),
            AppWindow::Config => state.config.update(ui),
        }
    }
}
