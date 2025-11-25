use unshell_obfuscate::obfuscated_symbol;

use crate::config::NamedComponent;

/// Publicly facing accessor function for the payload to load inside the breakout modules.
#[obfuscated_symbol]
pub fn get_components() -> Vec<NamedComponent> {
    return vec![
        #[cfg(feature = "client")]
        crate::client::get_named_component(),
        #[cfg(feature = "server")]
        crate::server::get_named_component(),
    ];
}
