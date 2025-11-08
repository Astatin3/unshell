#![no_main]

use std::collections::HashMap;
use unshell_lib::Component;

#[unsafe(no_mangle)]
pub fn get_components() -> HashMap<&'static str, Box<dyn Component>> {
    let mut components: HashMap<&'static str, Box<dyn Component>> = HashMap::new();

    #[cfg(feature = "client")]
    components.insert(
        unshell_lib::client::MODULE_NAME,
        Box::new(unshell_lib::client::ClientComponent::new()),
    );

    components

    // vec![
    //     Feature::Client,
    //     #[cfg(feature = "server")]
    //     Feature::Server,
    // ]
}

#[cfg(feature = "client")]
pub use unshell_lib::client::*;
#[cfg(feature = "server")]
pub use unshell_lib::server::*;
