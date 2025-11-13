use unshell_obfuscate::obfuscated_symbol;

use crate::{Component, config::NamedComponent};

use std::collections::HashMap;

#[obfuscated_symbol]
pub fn get_components() -> Vec<NamedComponent> {
    // let mut components: HashMap<&'static str, Box<dyn Component>> = HashMap::new();

    return vec![
        NamedComponent {name:,crate::client::MODULE_NAME, get_interface: crate::client::get_interface, start_runtime: todo!() },
    ];

    // components

    // vec![
    //     Feature::Client,
    //     #[cfg(feature = "server")]
    //     Feature::Server,
    // ]
}
