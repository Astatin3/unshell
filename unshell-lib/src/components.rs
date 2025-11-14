use unshell_obfuscate::obfuscated_symbol;

use crate::config::NamedComponent;

#[obfuscated_symbol]
pub fn get_components() -> Vec<NamedComponent> {
    return vec![
        #[cfg(feature = "client")]
        crate::client::get_named_component(),
    ];
}
