use egui::{Align2, Area, Color32, Frame, Order, Sense, UiKind, Vec2, mutex::Mutex};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::json;
use std::sync::Arc;
use wasm_bindgen::prelude::Closure;

use unshell::Result;

#[derive(serde::Deserialize, serde::Serialize)]
pub struct Auth {
    // Auth Stuff
    token: Option<Token>,
    #[serde(skip)]
    auth_state: Arc<Mutex<AuthState>>,

    // UI Stuff
    username: String,
    #[serde(skip)]
    password: String,
    show_password: bool,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
struct Token {
    expiration: u128,
    token: String,
}

#[derive(Debug, PartialEq, Eq)]
enum AuthState {
    Unset,
    NotLoggedIn,
    RequestSent,
    Authorised(Token),
    Error(String),
}

impl Default for AuthState {
    fn default() -> Self {
        Self::Unset
    }
}

impl Auth {
    /// Refresh the authentication state
    pub fn logged_in(&mut self) -> bool {
        match (self.token.is_some(), &*self.auth_state.lock()) {
            // The client is actually authorized
            (true, AuthState::Authorised(_)) => true,

            // If the user has just reloaded the session,
            // the AuthState is not automatically set by any other process
            (true, AuthState::Unset) => true,

            // If the authentication state has been updated to unauthorized, delete the token
            (true, _) => {
                self.token = None;
                false
            }

            // If the authentication state has been updated to authorized, set the token
            (false, AuthState::Authorised(token)) => {
                self.token = Some(token.clone());

                // Also clear the password because it is bad to store it
                self.password.clear();

                true
            }

            // The client is actually unauthorized
            (false, _) => false,
        }
    }

    pub fn update(&mut self, ui: &mut egui::Ui) {
        Area::new("Auth".into())
            .kind(UiKind::Modal)
            .sense(Sense::hover())
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .order(Order::Foreground)
            .interactable(true)
            .show(ui.ctx(), |ui| {
                Frame::popup(ui.style()).show(ui, |ui| {
                    ui.heading("UnShell Login");

                    ui.horizontal(|ui| {
                        ui.label("Username");
                        ui.text_edit_singleline(&mut self.username);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Password");
                        let _ = ui.add(
                            // [ui.available_width(), 24.],
                            egui::TextEdit::singleline(&mut self.password)
                                .password(!self.show_password),
                        );

                        self.show_password = ui.button("Show").is_pointer_button_down_on();

                        // ui.toggle_value(&mut self.show_password, "Show");
                    });

                    ui.horizontal(|ui| {
                        let (show_spinner, err_text) = match *self.auth_state.lock() {
                            AuthState::Error(ref e) => {
                                // self.login_button(ui);
                                (false, e.clone())
                            }
                            AuthState::RequestSent => (true, "".into()),
                            _ => (false, "".into()),
                        };

                        if !show_spinner {
                            if ui.button("Login").clicked() {
                                let state = self.auth_state.clone();

                                crate::httpPost(
                                    "/api/auth",
                                    &json!({
                                        "username": self.username.clone(),
                                        "password": self.password.clone()
                                    })
                                    .to_string(),
                                    Closure::once_into_js(move |ok: bool, response: String| {
                                        *(state.lock()) = if ok {
                                            if let Ok(token) =
                                                serde_json::from_str::<Token>(&response)
                                            {
                                                AuthState::Authorised(token)
                                            } else {
                                                AuthState::Error("Malformed Response".into())
                                            }
                                        } else {
                                            AuthState::Error(response)
                                        }
                                    }),
                                );

                                *(self.auth_state.lock()) = AuthState::RequestSent;
                            }
                        } else {
                            ui.spinner();
                        }

                        ui.colored_label(Color32::RED, err_text);
                    });
                });
            });
        //     });
        // });
    }

    pub fn get<R, F>(&self, path: &str, ret: F) -> Result<()>
    where
        F: FnOnce(R) + 'static,
        R: DeserializeOwned,
    {
        if let Some(ref token) = self.token {
            let state = self.auth_state.clone();
            crate::httpGetAuth(
                path,
                format!("Bearer {}", token.token),
                Closure::once_into_js(move |ok: bool, response: String| {
                    if ok {
                        if let Ok(value) = serde_json::from_str::<R>(&response) {
                            ret(value)
                        } else {
                            *(state.lock()) = AuthState::Error("Malformed Response".into());
                        }
                    } else {
                        *(state.lock()) = AuthState::Error(response);
                    }
                }),
            );
        }

        Ok(())
    }

    pub fn post<R, T, F>(&self, path: &str, data: &T, ret: F) -> Result<()>
    where
        R: DeserializeOwned,
        T: Serialize,
        F: FnOnce(R) + 'static,
    {
        if let Some(ref token) = self.token {
            let state = self.auth_state.clone();
            crate::httpPostAuth(
                path,
                format!("Bearer {}", token.token),
                &serde_json::to_string(data)?,
                Closure::once_into_js(move |ok: bool, response: String| {
                    if ok {
                        if let Ok(value) = serde_json::from_str::<R>(&response) {
                            ret(value)
                        } else {
                            *(state.lock()) = AuthState::Error("Malformed Response".into());
                        }
                    } else {
                        *(state.lock()) = AuthState::Error(response);
                    }
                }),
            );
        }

        Ok(())
    }
}

impl Default for Auth {
    fn default() -> Self {
        Self {
            token: Default::default(),
            auth_state: Arc::new(Mutex::new(AuthState::NotLoggedIn)),
            username: Default::default(),
            password: Default::default(),
            show_password: Default::default(),
        }
    }
}
