use std::collections::HashMap;

use egui::TextEdit;
use serde_json::Value;

#[derive(serde::Deserialize, serde::Serialize)]
enum ConfigStructField {
    Header(String),
    Text(String),
    String {
        default: Option<String>,
        max_length: Option<usize>,
        protected: bool,
    },
    Integer {
        default: i32,
        min: Option<i32>,
        max: Option<i32>,
    },
    // Checkbox
    // Dropdown
    // Collapsing header
    // Slider
    // ...
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct Config {
    config: Vec<(String, ConfigStructField)>,
    state: HashMap<String, Value>,
}

impl Config {
    fn new(config: Vec<(String, ConfigStructField)>) -> Self {
        Self {
            config,
            state: HashMap::default(),
        }
    }

    pub fn update(&mut self, ui: &mut egui::Ui) {
        for (id, field) in &self.config {
            match field {
                ConfigStructField::Header(text) => {
                    ui.heading(text);
                }
                ConfigStructField::Text(text) => {
                    ui.label(text);
                }
                ConfigStructField::String {
                    default,
                    max_length,
                    protected,
                } => {
                    let value = if let Some(Value::String(value)) = self.state.get_mut(id) {
                        value
                    } else {
                        self.state.insert(
                            id.clone(),
                            Value::String(default.clone().unwrap_or(String::new())),
                        );
                        if let Some(Value::String(value)) = self.state.get_mut(id) {
                            value
                        } else {
                            unreachable!()
                        }
                    };

                    let mut widget = TextEdit::singleline(value).password(*protected);

                    if let Some(limit) = &max_length {
                        widget = widget.char_limit(*limit);
                    }

                    ui.add(widget);
                }
                _ => {} // ConfigStructField::Integer { default, min, max } => todo!(),
            }
        }

        // match &self.field {
        //     ConfigStructField::Header(text) => {
        //         ui.heading(text);
        //     }
        //     ConfigStructField::Text(text) => {
        //         ui.label(text);
        //     }
        //     ConfigStructField::String {
        //         default,
        //         max_length,
        //         protected,
        //     } => ui.text_edit_singleline(),
        //     ConfigStructField::Integer { default, min, max } => todo!(),
        // }
    }

    pub fn export(&self) -> String {
        serde_json::to_string(&self.config).unwrap()
    }
}

pub fn default_configurable() -> Config {
    Config::new(vec![
        (
            "Header".into(),
            ConfigStructField::Header("Test Header!".into()),
        ),
        ("text".into(), ConfigStructField::Text("Test Text!".into())),
        (
            "Config".into(),
            ConfigStructField::String {
                default: Some("Test String".into()),
                max_length: Some(30),
                protected: false,
            },
        ),
        (
            "Protected".into(),
            ConfigStructField::String {
                default: Some("Protected String".into()),
                max_length: None,
                protected: true,
            },
        ),
    ])
}
