// use crate::payload_config::structs::ConfigStructField;

mod structs;

#[derive(serde::Deserialize, serde::Serialize)]
pub struct PayloadConfig {
    config_struct: structs::Config,
}

// struct ServerConfigState {
//     // config: Vec<PayloadConfig>
// }

impl PayloadConfig {
    pub fn update(&mut self, ui: &mut egui::Ui) {
        // ui.heading("Test");
        self.config_struct.update(ui);
    }
}

impl Default for PayloadConfig {
    fn default() -> Self {
        Self {
            config_struct: structs::default_configurable(),
        }
    }
}
