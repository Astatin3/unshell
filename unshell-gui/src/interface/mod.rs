mod interface;

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use log::debug;
use unshell_lib::{Result, config::TreeMessage};

use crate::auth::Auth;

#[derive(serde::Deserialize, serde::Serialize)]
pub struct InterfaceWindow {
    path: PathBuf,
    // #[serde(skip)]
    // data_bind: Bind<String, ModuleError>,
    #[serde(skip)]
    state: Arc<Mutex<InterfaceWindowState>>,
    // #[serde(skip)]
    // promise: Option<PromiseWrapper<Result<TreeMessage>>>,
}

pub struct InterfaceWindowState {
    is_request: bool,
    is_error: bool,
    branch: Option<TreeMessage>,
}

impl InterfaceWindow {
    pub fn update(&mut self, auth: &mut Auth, ui: &mut egui::Ui) {
        // let data_bind = Bind::<InterfaceWindowState, ModuleError>::new(false);

        ui.heading("Interface");
        ui.label(self.path.to_string_lossy());

        {
            if !self.path.eq("/") && ui.button("Go up").clicked() {
                self.go_up();
                self.state.lock().unwrap().branch = None;
            }

            let state_lock = self.state.lock().unwrap();

            let (is_request, is_error, is_some) = (
                state_lock.is_request,
                state_lock.is_error,
                state_lock.branch.is_some(),
            );

            drop(state_lock);

            if is_request {
                ui.spinner();
            } else if is_error {
                self.reset_path();
                let mut state_lock = self.state.lock().unwrap();

                state_lock.is_error = false;
                state_lock.is_request = false;
                state_lock.branch = None;

                drop(state_lock)
            } else if !is_some {
                {
                    let mut state_lock = self.state.lock().unwrap();

                    state_lock.is_request = true;
                }

                let state_clone = self.state.clone();
                auth.get(
                    &format!("/api/interface{}", self.path.display()),
                    move |response: Result<TreeMessage>| {
                        let mut state_lock = state_clone.lock().unwrap();

                        match response {
                            Ok(item) => {
                                state_lock.branch = Some(item);
                            }
                            Err(err) => {
                                crate::log(&format!("Got error {err:?}"));
                                state_lock.is_error = true;
                            }
                        }

                        state_lock.is_request = false;

                        drop(state_lock);
                    },
                )
                .unwrap();
            } else {
                let state_clone = self.state.clone();

                let mut state_lock = state_clone.lock().unwrap();

                let branch = (state_lock.branch).as_mut().unwrap();

                let clear = match branch {
                    TreeMessage::InterfaceAndValue(interface_struct, interface_data) => {
                        interface::render(ui, interface_struct, interface_data);

                        if ui.button("Save").clicked() {
                            auth.post(
                                &format!("/api/interface{}", self.path.display()),
                                &TreeMessage::State(interface_data.clone()),
                                move |response: Result<TreeMessage>| {
                                    debug!("{response:?}");
                                },
                            )
                            .unwrap();
                        }

                        false
                    }
                    TreeMessage::Folder(items) => {
                        let mut clear = false;
                        for item in items {
                            if ui.button(&format!("Item {}", item)).clicked() {
                                self.go_down(item.clone());
                                clear = true;
                                break;
                            }
                        }
                        clear
                    }
                    _ => false,
                };

                if clear {
                    state_lock.branch = None;
                }

                drop(state_lock)
            }
        }
    }

    fn reset_path(&mut self) {
        self.path = PathBuf::from("/");
    }

    fn go_up(&mut self) {
        if let Some(parent) = self.path.parent() {
            self.path = PathBuf::from(parent);
        }
    }

    fn go_down(&mut self, path: String) {
        self.path = self.path.join(path);
    }
}

impl Default for InterfaceWindow {
    fn default() -> Self {
        Self {
            path: "/".into(),
            // promise: None,
            state: Arc::new(Mutex::new(InterfaceWindowState::default())),
            // data_bind: Bind::new(false),
        }
    }
}

impl Default for InterfaceWindowState {
    fn default() -> Self {
        Self {
            is_request: false,
            branch: None,
            is_error: false,
        }
    }
}
