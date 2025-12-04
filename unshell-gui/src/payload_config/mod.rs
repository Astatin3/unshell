#[derive(serde::Deserialize, serde::Serialize)]
pub struct PayloadConfig {}

struct ServerConfigState {
    // config: Vec<PayloadConfig>
}

impl PayloadConfig {
    pub fn update(&mut self, ui: &mut egui::Ui) {
        ui.heading("Test");
    }
}

impl Default for PayloadConfig {
    fn default() -> Self {
        Self {}
    }
}
