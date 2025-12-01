use std::sync::{Arc, Mutex};

use egui::Color32;

use crate::auth::Auth;

#[derive(serde::Deserialize, serde::Serialize)]
pub struct Config {
    response_text: Arc<Mutex<String>>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            response_text: Arc::new(Mutex::new("NONE".to_string())),
        }
    }
}

impl Config {
    pub fn update(&mut self, auth: &mut Auth, ui: &mut egui::Ui) {
        if ui.button("Set Value").clicked() {
            let text_clone = self.response_text.clone();
            auth.get("/api/test", move |response: Result<String, String>| {
                *text_clone.lock().unwrap() = format!("{:?}", response);
            });
        }

        ui.horizontal(|ui| {
            ui.label("Response: ");
            ui.colored_label(Color32::WHITE, &*self.response_text.lock().unwrap());
        });
    }
}
