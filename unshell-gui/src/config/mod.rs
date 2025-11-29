use crate::auth::Auth;

#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct Config {}

impl Config {
    pub fn update(&mut self, auth: &mut Auth, ui: &mut egui::Ui) {
        if ui.button("Test").clicked() {
            auth.test();
        }
    }
}
