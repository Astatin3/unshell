use chrono::DateTime;
use chrono::Utc;
use egui::Color32;
use egui::TextStyle;
use egui_extras::Column;
use egui_extras::TableBuilder;

use crate::auth::Auth;

use std::sync::Arc;
use std::sync::Mutex;
use std::time::SystemTime;

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LogViewer {
    enable_location: bool,
    #[serde(skip)]
    state: Arc<Mutex<LogState>>,
}

#[derive(Default)]
struct LogState {
    logs: Vec<Record>,
    // trees: Option<Vec<String>>,
    // tree_keys: Option<HashMap<String, String>>,
    is_requesting: bool,
    requested_data: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Record {
    log_level: LogLevel,
    location: Option<String>,
    time: SystemTime,
    message: String,
}

impl Record {
    pub fn display_level(&self, ui: &mut egui::Ui) {
        match self.log_level {
            LogLevel::Debug => ui.colored_label(Color32::LIGHT_BLUE, "DBUG"),
            LogLevel::Info => ui.colored_label(Color32::DARK_GREEN, "INFO"),
            LogLevel::Warn => ui.colored_label(Color32::YELLOW, "WARN"),
            LogLevel::Error => ui.colored_label(Color32::RED, "ERR!"),
        };
    }
    pub fn display_location(&self, ui: &mut egui::Ui) {
        if let Some(ref location) = self.location {
            ui.label(location);
        }
    }
    pub fn display_time(&self, ui: &mut egui::Ui) {
        let date: DateTime<Utc> = self.time.into();
        let date = date.to_rfc2822().to_string();
        ui.label(date);
    }
    pub fn display_message(&self, ui: &mut egui::Ui) {
        ui.strong(&self.message);
    }
}

impl LogViewer {
    pub fn update(&mut self, auth: &mut Auth, ui: &mut egui::Ui) {
        ui.heading("Log Viewer");

        ui.horizontal(|ui| {
            if ui.button("Refresh").clicked() {
                self.refresh_logs(auth);
            }

            ui.checkbox(&mut self.enable_location, "Enable Location");
        });

        // let logs = ;

        let body_text_size = TextStyle::Body.resolve(ui.style()).size;
        use egui_extras::{Size, StripBuilder};
        StripBuilder::new(ui)
            .size(Size::remainder().at_least(100.0)) // for the table
            .size(Size::exact(body_text_size))
            .vertical(|mut strip| {
                strip.cell(|ui| {
                    egui::ScrollArea::both()
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            let table = TableBuilder::new(ui)
                                .striped(true)
                                .resizable(true)
                                .stick_to_bottom(true)
                                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                                .column(Column::auto())
                                .column(Column::auto())
                                .column(Column::auto())
                                .min_scrolled_height(0.0)
                                .sense(egui::Sense::click());

                            let table = if self.enable_location {
                                table.column(Column::auto())
                            } else {
                                table
                            };

                            table
                                .header(20., |mut header| {
                                    header.col(|ui| {
                                        ui.strong("Time");
                                    });
                                    header.col(|ui| {
                                        ui.strong("Level");
                                    });
                                    header.col(|ui| {
                                        ui.strong("Message");
                                    });
                                    if self.enable_location {
                                        header.col(|ui| {
                                            ui.strong("Location");
                                        });
                                    }
                                })
                                .body(|mut body| {
                                    let state_lock = self.state.lock().unwrap();
                                    // let logs = state_lock.logs.as_ref();

                                    for log in state_lock.logs.iter() {
                                        //     // let runtime = self.current_runtimes

                                        body.row(18., |mut row| {
                                            row.col(|ui| log.display_time(ui));
                                            row.col(|ui| log.display_level(ui));
                                            row.col(|ui| log.display_message(ui));
                                            if self.enable_location {
                                                row.col(|ui| log.display_location(ui));
                                            }
                                        });
                                    }
                                });
                        });
                });
            });

        {
            let state_lock = self.state.lock().unwrap();

            match (
                state_lock.is_requesting,
                state_lock.logs.len() == 0,
                state_lock.requested_data,
            ) {
                (true, _, _) => {
                    drop(state_lock);
                    ui.spinner();
                }
                (false, true, true) => {
                    drop(state_lock);
                    ui.label("There are no logs");
                }
                (false, true, false) => {
                    drop(state_lock);
                    self.refresh_logs(auth);
                }
                _ => {
                    drop(state_lock);
                }
            }
        }
    }

    fn refresh_logs(&self, auth: &mut Auth) {
        let state_clone = self.state.clone();
        {
            let mut state_lock = self.state.lock().unwrap();
            state_lock.logs.clear();
            state_lock.is_requesting = true;
        }
        auth.get(&format!("/api/log/{}", 0), move |e: Vec<String>| {
            let mut state_lock = state_clone.lock().unwrap();
            state_lock.logs.append(
                &mut e
                    .iter()
                    .map(|log| serde_json::from_str(log).unwrap())
                    .collect(),
            );
            state_lock.is_requesting = false;
            state_lock.requested_data = true;

            // crate::log(&format!("{e:?}"));
        });
    }
}

impl Default for LogViewer {
    fn default() -> Self {
        Self {
            enable_location: false,
            state: Arc::new(Mutex::new(LogState::default())),
        }
    }
}
