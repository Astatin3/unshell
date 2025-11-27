use egui::{TextStyle, Ui};
use egui_extras::{Column, TableBuilder};
use unshell_lib::config::RuntimeConfig;

#[derive(serde::Deserialize, serde::Serialize)]
pub struct Config {
    state: ConfigState,
    current_payloads: Vec<PayloadConfig>,
}

#[derive(serde::Deserialize, serde::Serialize)]
enum ConfigState {
    Base,
    NewConfig(PayloadConfig),
    EditConfig(usize, PayloadConfig),
}

impl Config {
    pub fn new() -> Self {
        Self {
            state: ConfigState::Base,
            current_payloads: vec![PayloadConfig {
                name: "Test".to_string(),
                components: vec!["server".to_string()],
                runtimes: Vec::new(),
            }],
        }
    }

    pub fn title(&self) -> &str {
        match self.state {
            ConfigState::Base => "Config",
            ConfigState::NewConfig(..) => "Config/New",
            ConfigState::EditConfig(..) => "Config/Edit",
        }
    }

    pub fn update(&mut self, ui: &mut Ui) {
        match &mut self.state {
            ConfigState::Base => self.table_ui(ui),
            ConfigState::EditConfig(_, config) => {
                ui.heading("Edit Payload");
                match Self::edit_ui(ui, config) {
                    Some(true) => {
                        if let ConfigState::EditConfig(n, config) =
                            std::mem::replace(&mut self.state, ConfigState::Base)
                        {
                            self.current_payloads[n] = config;
                        }
                    }
                    Some(false) => {
                        self.state = ConfigState::Base;
                    }
                    _ => {}
                }
            }
            ConfigState::NewConfig(config) => {
                ui.heading("Edit Payload");
                match Self::edit_ui(ui, config) {
                    Some(true) => {
                        if let ConfigState::NewConfig(config) =
                            std::mem::replace(&mut self.state, ConfigState::Base)
                        {
                            self.current_payloads.push(config);
                        }
                    }
                    Some(false) => {
                        self.state = ConfigState::Base;
                    }
                    _ => {}
                }
            }
        }
    }

    fn table_ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.heading("Payloads");

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("New").clicked() {
                    self.state = ConfigState::NewConfig(PayloadConfig::new());
                }
            });
        });

        let body_text_size = TextStyle::Body.resolve(ui.style()).size;
        use egui_extras::{Size, StripBuilder};
        StripBuilder::new(ui)
            .size(Size::remainder().at_least(100.0)) // for the table
            .size(Size::exact(body_text_size)) // for the source code link
            .vertical(|mut strip| {
                strip.cell(|ui| {
                    egui::ScrollArea::horizontal().show(ui, |ui| {
                        let table = TableBuilder::new(ui)
                            .striped(true)
                            .resizable(true)
                            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                            .column(Column::auto().resizable(false))
                            .column(Column::remainder())
                            .column(Column::remainder())
                            .column(Column::remainder())
                            .column(Column::auto().resizable(false))
                            .min_scrolled_height(0.0)
                            .sense(egui::Sense::click());

                        table
                            .header(20., |mut header| {
                                header.col(|ui| {
                                    ui.strong("#");
                                });
                                header.col(|ui| {
                                    ui.strong("Name");
                                });
                                header.col(|ui| {
                                    ui.strong("Components");
                                });
                                header.col(|ui| {
                                    ui.strong("Runtimes");
                                });
                                header.col(|_| {});
                            })
                            .body(|mut body| {
                                for i in 0..self.current_payloads.len() {
                                    // let runtime = self.current_runtimes

                                    body.row(18., |mut row| {
                                        row.col(|ui| {
                                            ui.label(i.to_string());
                                        });
                                        row.col(|ui| {
                                            ui.label(self.current_payloads[i].name.clone());
                                        });
                                        row.col(|ui| {
                                            ui.label(format!(
                                                "{:?}",
                                                self.current_payloads[i].components.clone()
                                            ));
                                        });
                                        row.col(|ui| {
                                            ui.label("A");
                                        });
                                        row.col(|ui| {
                                            if ui.button("Edit").clicked() {
                                                self.state = ConfigState::EditConfig(
                                                    i,
                                                    self.current_payloads[i].clone(),
                                                );
                                            }
                                            // if ui.button("Delete").clicked() {
                                            //     self.state = ConfigState::EditConfig(
                                            //         i,
                                            //         self.current_payloads[i].clone(),
                                            //     );
                                            // }
                                        });

                                        if row.response().clicked() {
                                            self.state = ConfigState::EditConfig(
                                                i,
                                                self.current_payloads[i].clone(),
                                            );
                                        }
                                    });
                                }
                            });
                    });
                });
            });
    }

    fn edit_ui(ui: &mut Ui, config: &mut PayloadConfig) -> Option<bool> {
        ui.horizontal(|ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut config.name);
        });

        ui.horizontal(|ui| {
            ui.label("Components: ");

            for component in vec!["client", "server"] {
                let enabled = config
                    .components
                    .iter()
                    .enumerate()
                    .find(|s| s.1.eq(component));
                if ui.selectable_label(enabled.is_some(), component).clicked() {
                    if let Some((i, _)) = enabled {
                        let _ = config.components.remove(i);
                    } else {
                        config.components.push(component.to_string());
                    }
                }
            }
        });

        let mut ret = None;
        ui.horizontal(|ui| {
            ret = if ui.button("Back").clicked() {
                Some(false)
            } else if ui.button("Save").clicked() {
                Some(true)
            } else {
                None
            };
        });

        ret

        // return None;
    }
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
struct PayloadConfig {
    name: String,
    components: Vec<String>,
    runtimes: Vec<RuntimeConfig>,
}

impl PayloadConfig {
    pub fn new() -> Self {
        Self {
            name: "New Payload".to_string(),
            components: Vec::new(),
            runtimes: Vec::new(),
        }
    }
}
