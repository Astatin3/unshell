use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use egui::ComboBox;
use egui_extras::{Column, TableBuilder};

use crate::auth::Auth;

#[derive(serde::Deserialize, serde::Serialize)]
pub struct Config {
    tree_option: String,
    #[serde(skip)]
    state: Arc<Mutex<ConfigState>>,
    // trees: Arc<Mutex<Option<Vec<String>>>>,
    // #[serde(skip)]
    // is_requesting: Arc<AtomicBool>,
}

#[derive(Default)]
struct ConfigState {
    trees: Option<Vec<String>>,
    tree_keys: Option<HashMap<String, String>>,
    is_requesting: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tree_option: "".into(),
            state: Arc::new(Mutex::new(ConfigState {
                trees: None,
                tree_keys: None,
                is_requesting: false,
            })), // trees: Arc::new(Mutex::new(None)),
                 // is_requesting: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Config {
    pub fn update(&mut self, auth: &mut Auth, ui: &mut egui::Ui) {
        let state_lock = self.state.lock().unwrap();
        let mut tree_list_none = state_lock.trees.is_none();
        let mut key_list_none = state_lock.tree_keys.is_none();
        let is_requesting = state_lock.is_requesting;

        if !tree_list_none
            && !state_lock
                .trees
                .as_ref()
                .unwrap()
                .contains(&self.tree_option)
        {
            self.tree_option.clear();
        }

        drop(state_lock);

        ui.horizontal(|ui| {
            if ui.button("Refresh").clicked() {
                // self.tree_option.clear();
                let mut state_lock = self.state.lock().unwrap();
                (*state_lock).trees = None;
                (*state_lock).tree_keys = None;
                drop(state_lock);

                tree_list_none = true;
                key_list_none = true;
            }

            if tree_list_none && !is_requesting {
                self.state.lock().unwrap().is_requesting = true;
                let state_clone = self.state.clone();

                auth.get("/api/trees", move |response: Vec<String>| {
                    let mut state_lock = state_clone.lock().unwrap();
                    state_lock.trees = Some(response);
                    state_lock.is_requesting = false;
                    drop(state_lock);
                });
            } else if tree_list_none && is_requesting {
                ui.spinner();
            }

            let state_lock = self.state.lock().unwrap();
            // This might have changed since the above api call
            tree_list_none = state_lock.trees.is_none();

            if !tree_list_none {
                let trees = state_lock.trees.as_ref().unwrap().clone();
                drop(state_lock);

                let before = &self.tree_option.clone();
                egui::ComboBox::from_id_salt("Select Tree")
                    .selected_text(&self.tree_option)
                    .show_ui(ui, |ui| {
                        for tree in trees {
                            ui.selectable_value(&mut self.tree_option, tree.clone(), tree);
                        }
                    });

                if before.ne(&self.tree_option) {
                    (*self.state.lock().unwrap()).tree_keys = None;
                    key_list_none = true;
                }
            }

            if !self.tree_option.is_empty() && !tree_list_none {
                // ui.horizontal(|ui| {
                //     ui.label(&format!("Tree: {}", self.tree_option));
                // });

                if key_list_none && !is_requesting {
                    self.state.lock().unwrap().is_requesting = true;
                    let state_clone = self.state.clone();

                    auth.get(
                        &format!("/api/values/{}", self.tree_option),
                        move |response: Result<HashMap<String, String>, String>| {
                            let mut state_lock = state_clone.lock().unwrap();
                            state_lock.tree_keys = Some(response.unwrap());
                            state_lock.is_requesting = false;
                        },
                    );
                } else if key_list_none && is_requesting {
                    ui.spinner();
                }
            }
        });

        if !key_list_none {
            // let body_text_size = TextStyle::Body.resolve(ui.style()).size;
            // use egui_extras::{Size, StripBuilder};
            // StripBuilder::new(ui)
            //     .size(Size::remainder().at_least(100.0)) // for the table
            //     .size(Size::exact(body_text_size))
            //     .vertical(|mut strip| {
            //         strip.cell(|ui| {

            egui::ScrollArea::both().show(ui, |ui| {
                let table = TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::auto())
                    .column(Column::auto())
                    .min_scrolled_height(0.0)
                    .sense(egui::Sense::click());

                table
                    .header(20., |mut header| {
                        header.col(|ui| {
                            ui.strong("key");
                        });
                        header.col(|ui| {
                            ui.strong("value");
                        });
                    })
                    .body(|mut body| {
                        let state_lock = self.state.lock().unwrap();
                        let map = state_lock.tree_keys.as_ref().unwrap();

                        for (key, value) in (map).iter() {
                            //     // let runtime = self.current_runtimes

                            body.row(18., |mut row| {
                                row.col(|ui| {
                                    ui.label(key);
                                });
                                row.col(|ui| {
                                    ui.label(value);
                                });
                            });
                        }
                    });
            });
            //     });
            // });
        }
    }
}
