use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use egui::Color32;

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
    tree_keys: Option<Vec<String>>,
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
        let mut state_lock = self.state.lock().unwrap();
        let tree_list_none = state_lock.trees.is_none();
        let key_list_none = state_lock.tree_keys.is_none();
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

        if ui.button("Refresh").clicked() {
            self.tree_option.clear();
            (*self.state.lock().unwrap()).trees = None;
        }

        ui.horizontal(|ui| {
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
        });

        ui.horizontal(|ui| {
            if !tree_list_none {
                // let tree_len = self.state.lock().unwrap().trees.as_ref().unwrap().len();

                // for tree in &state_lock.unwrap() {
                //     if ui.button(tree).clicked() {
                //         self.tree_option = tree.to_string();
                //         (*state_lock).tree_keys.as_mut();
                //     }
                // }

                let state_lock = self.state.lock().unwrap();
                let trees = state_lock.trees.as_ref().unwrap().clone();
                drop(state_lock);

                for tree in trees {
                    // let tree: &&String = self
                    //     .state
                    //     .lock()
                    //     .unwrap()
                    //     .tree_keys
                    //     .unwrap()
                    //     .iter()
                    //     .nth(i)
                    //     .unwrap();

                    // let tree sel

                    if ui.button(&tree).clicked() {
                        self.tree_option = tree.to_string();
                        (*self.state.lock().unwrap()).tree_keys = None;
                    }
                }
            }
        });

        if !self.tree_option.is_empty() && !tree_list_none {
            ui.horizontal(|ui| {
                ui.label(&format!("Tree: {}", self.tree_option));
            });

            ui.horizontal(|ui| {
                if key_list_none && !is_requesting {
                    self.state.lock().unwrap().is_requesting = true;
                    let state_clone = self.state.clone();

                    auth.get(
                        &format!("/api/keys/{}", self.tree_option),
                        move |response: Result<Vec<String>, String>| {
                            let mut state_lock = state_clone.lock().unwrap();
                            state_lock.tree_keys = Some(response.unwrap());
                            state_lock.is_requesting = false;
                        },
                    );
                } else if key_list_none && is_requesting {
                    ui.spinner();
                } else {
                    ui.label(&format!(
                        "Keys: {:?}",
                        self.state.lock().unwrap().tree_keys.clone()
                    ));
                }
            });
        }
    }
}
