use serde_json::json;
use std::{collections::HashMap, sync::Arc, time::Duration};

use chrono::Utc;
use egui::{Align2, Area, Frame, Order, Sense, UiKind, Vec2, mutex::Mutex};
use wasm_bindgen::prelude::Closure;

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
                        if ui.button("Login").clicked() {
                            let state = self.auth_state.clone();

                            crate::httpPost(
                                "/auth",
                                &json!({
                                    "username": self.username.clone(),
                                    "password": self.password.clone()
                                })
                                .to_string(),
                                Closure::once_into_js(move |ok: bool, response: String| {
                                    *(state.lock()) = if ok {
                                        if let Ok(token) = serde_json::from_str::<Token>(&response)
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

                        ui.label(format!("{:?}", self.auth_state.lock()));
                    });
                });
            });
        //     });
        // });
    }

    pub fn test(&self) {
        if let Some(ref token) = self.token {
            let state = self.auth_state.clone();
            crate::httpGetAuth(
                "/api/test1234/kjhejwer/kwherjwer/iuwehrhiwer/wiuerhjwer",
                format!("Bearer {}", token.token),
                Closure::once_into_js(move |ok: bool, response: String| {
                    if ok {
                        crate::log(&response);
                    } else {
                        *(state.lock()) = AuthState::Error(response);
                    }
                }),
            );
        }
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
