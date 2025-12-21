use egui::{Color32, TextEdit};
use unshell_lib::config::{InterfaceData, InterfaceStruct, config_struct};

pub fn render(
    ui: &mut egui::Ui,
    interface_struct: &InterfaceStruct,
    interface_data: &mut InterfaceData,
) {
    match (interface_struct, interface_data) {
        (InterfaceStruct::ConfigStruct(interface), InterfaceData::ConfigStruct(data)) => {
            render_config_struct(ui, interface, data);
        }
    }
}

fn render_config_struct(
    ui: &mut egui::Ui,
    interface: &config_struct::ConfigStructKeys,
    data: &mut config_struct::ConfigStructValues,
) {
    for (interface, data) in interface.iter().zip(data) {
        match (interface, data) {
            (config_struct::ConfigStructField::Header(text), serde_json::Value::Null) => {
                ui.heading(text);
            }

            (config_struct::ConfigStructField::Text(text), serde_json::Value::Null) => {
                ui.label(text);
            }

            (
                config_struct::ConfigStructField::String {
                    default: _,
                    max_length,
                    protected,
                },
                serde_json::Value::String(value),
            ) => {
                let mut widget = TextEdit::singleline(value);

                if let Some(limit) = &max_length {
                    widget = widget.char_limit(*limit);
                }

                if let Some(protected) = &protected {
                    widget = widget.password(*protected);
                }

                ui.add(widget);
            }
            (
                config_struct::ConfigStructField::Integer { default, min, max },
                serde_json::Value::Number(number),
            ) => todo!(),

            (interface, data) => {
                ui.colored_label(
                    Color32::RED,
                    &format!("Incorrect type and value! {interface:?} and {data:?}"),
                );
            }
        }
    }
}
