use crate::auth::Auth;

use std::sync::Arc;
use std::sync::Mutex;

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LogViewer {
    #[serde(skip)]
    state: Arc<Mutex<LogState>>,
}

#[derive(Default)]
struct LogState {
    logs: Vec<String>,
    // trees: Option<Vec<String>>,
    // tree_keys: Option<HashMap<String, String>>,
    // is_requesting: bool,
}

impl LogViewer {
    pub fn update(&mut self, auth: &mut Auth, ui: &mut egui::Ui) {
        ui.heading("Log Viewer");
        for log in &self.state.lock().unwrap().logs {
            ui.label(log);
        }
        if ui.button("Poll").clicked() {
            let state_clone = self.state.clone();
            auth.get(&format!("/api/log/{}", 0), move |e: Vec<String>| {
                (*state_clone.lock().unwrap()).logs = e;
                // crate::log(&format!("{e:?}"));
            });
        }
    }
}

impl Default for LogViewer {
    fn default() -> Self {
        Self {
            // logs: Vec::new(),
            state: Arc::new(Mutex::new(LogState::default())),
        }
    }
}
