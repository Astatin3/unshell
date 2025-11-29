use std::{collections::HashMap, sync::Arc};

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
    password: String,
    show_password: bool,
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
struct Token {
    access_token: String,
    token_type: String,
}

#[derive(Debug, PartialEq, Eq)]
enum AuthState {
    NotLoggedIn,
    RequestSent,
    Authorised(Token),
    Error(String),
}

impl Default for AuthState {
    fn default() -> Self {
        Self::NotLoggedIn
    }
}

impl Auth {
    pub fn logged_in(&mut self) -> bool {
        if self.token.is_some() {
            true
        } else {
            match *self.auth_state.lock() {
                AuthState::Authorised(ref token) => {
                    self.token = Some(token.clone());
                    true
                }
                _ => false,
            }
        }

        // self.auth_state.lock().eq(&AuthState::Authorised)
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

                    if ui.button("Login").clicked() {
                        let json = serde_json::to_string(&HashMap::from([
                            ("client_id", self.username.clone()),
                            ("client_secret", self.password.clone()),
                        ]))
                        .unwrap();

                        let state = self.auth_state.clone();
                        *(state.lock()) = AuthState::RequestSent;

                        crate::httpPost(
                            "/auth",
                            &json,
                            Closure::once_into_js(move |response: String| {
                                *(state.lock()) =
                                    if let Ok(token) = serde_json::from_str::<Token>(&response) {
                                        AuthState::Authorised(token)
                                    } else {
                                        AuthState::Error("Malformed Response".into())
                                    }

                                // self.logged_in
                                //     .store(true, std::sync::atomic::Ordering::Relaxed);
                                // self.logged_in();
                            }),
                        );
                    }
                });
            });
        //     });
        // });
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
