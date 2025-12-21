mod render_interface;

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use unshell_lib::Result;
use unshell_lib::config::TreeMessage;

use crate::auth::Auth;

#[derive(serde::Deserialize, serde::Serialize)]
pub struct InterfaceWindow {
    path: PathBuf,

    #[serde(skip)]
    state: Arc<Mutex<InterfaceWindowState>>,
}

pub struct InterfaceWindowState {
    is_request: bool,
    is_error: bool,
    branch: Option<TreeMessage>,
}

impl InterfaceWindow {
    pub fn update(&mut self, auth: &mut Auth, ui: &mut egui::Ui) {
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
                            Err(_) => {
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

                let mut branch = (state_lock.branch).as_mut().unwrap();

                let clear = match branch {
                    TreeMessage::InterfaceAndValue(interface_struct, interface_data) => {
                        render_interface::render(ui, interface_struct, interface_data);
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
        self.path = PathBuf::from(self.path.parent().unwrap());
    }

    fn go_down(&mut self, path: String) {
        self.path = self.path.join(path);
    }
}

impl Default for InterfaceWindow {
    fn default() -> Self {
        Self {
            path: "/".into(),
            state: Arc::new(Mutex::new(InterfaceWindowState::default())),
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
