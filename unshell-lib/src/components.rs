use unshell_obfuscate::obfuscated_symbol;

use crate::config::NamedComponent;

#[obfuscated_symbol]
pub fn get_components() -> Vec<NamedComponent> {
    // let mut components: HashMap<&'static str, Box<dyn Component>> = HashMap::new();

    // let a = crate::client::get_interface;

    return vec![
        #[cfg(feature = "client")]
        crate::client::get_named_component(),
    ];

    // components

    // vec![
    //     Feature::Client,
    //     #[cfg(feature = "server")]
    //     Feature::Server,
    // ]
}
