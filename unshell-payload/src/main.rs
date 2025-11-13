use unshell_lib::{
    ModuleError,
    config::{PayloadConfig, RuntimeConfig},
    module::{Manager, Module},
};
use unshell_obfuscate::symbol;

#[macro_use]
extern crate unshell_lib;

static PAYLOAD_CONFIG: PayloadConfig = PayloadConfig {
    id: symbol!("Test ID"),
    components: unshell_lib::get_components(),
    runtime_config: vec![
        RuntimeConfig {
            "client"
        }
    ],
};

// static RUNTIME_CONFIG: PayloadConfig = PayloadConfig {
//     id: symbol!("Test ID"),
//     components: Vec::new(),
// };

fn main() {
    #[cfg(not(feature = "obfuscate"))]
    unshell_lib::logger::PrettyLogger::init();

    debug!("Initialized");

    match || -> Result<(), ModuleError> {
        let args = std::env::args();

        // let mut modules = Vec::new();
        // for arg in args.skip(1) {
        //     debug!("Loading module: {}", arg);
        //     modules.push(Module::new(&arg)?)
        // }

        Manager::run(&PAYLOAD_CONFIG, Vec::new());

        Ok(())
    }() {
        Ok(_) => {}
        Err(e) => {
            debug!("ERROR! {:?}", e);
        }
    }
}
