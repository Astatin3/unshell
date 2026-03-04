#![macro_use]
extern crate unshell;

use unshell::{info, logger::PrettyLogger, obfuscate::sym, tree::Tree};

fn main() {
    PrettyLogger::init();

    let mut manager = Tree::new();
    // manager.init_logger();

    println!("{}", sym!("TEST"));

    info!("Test thing!");
    info!("Test thing!");

    // loop {
    //     if test123(&mut manager) {
    //         break;
    //     }
    // }

    // println!("Test");
}

// fn test123(manager: &mut Tree) -> bool {
//     let result = manager.send_message(
//         Value::String(sym!("Logger").to_string()),
//         Value::String(symbols::CMD_GET.to_string()),
//     );

//     junk_asm!(20.);

//     let is_null = result.is_null();

//     // if let Ok(result) = serde_json::from_value::<Record>(result) {
//     //     log(&result);
//     // }

//     is_null

//     // println!("Logger: {}", result);
// }

// use std::{any::Any, collections::HashMap, fs::File, io::Read};

// use static_init::dynamic;
// use unshell_lib::{
//     ModuleError,
//     config::{PayloadConfig, RuntimeConfig},
//     module::{Manager, Module},
// };
// use unshell_obfuscate::{obs, symbol};

// #[macro_use]
// extern crate unshell_lib;

// // The main and initial 'configuration' for a payload

// #[dynamic]
// static PAYLOAD_CONFIG: PayloadConfig = PayloadConfig {
//     id: symbol!("Test ID"),
//     components: Vec::new(),
//     runtime_config: vec![RuntimeConfig {
//         parent_component: symbol!("client").to_string(),
//         name: symbol!("client runtime").to_string(),
//         config: HashMap::from([
//             (symbol!("host").to_string(), obs!("localhost:1234")),
//             (symbol!("retry").to_string(), obs!("1000")),
//         ]),
//     }],
// };

// fn main() {

//     debug!("Initialized");

//     match run() {
//         Ok(_) => {}
//         Err(e) => {
//             error!("ERROR! '{:?}'", e);
//         }
//     }
// }

// fn run() -> Result<(), Box<dyn std::error::Error>> {
//     let args = std::env::args();

//     // TEMPORARY, load the module paths from command line args.
//     let mut modules = Vec::new();
//     for arg in args.skip(1) {
//         // debug!("Loading module: {}", arg);

//         // let mut file = File::open(arg).map_err(|e| ModuleError::Error(e.to_string().into()))?;
//         // let mut buffer = Vec::new();
//         // file.read_to_end(&mut buffer)
//         //     .map_err(|e| ModuleError::Error(e.to_string().into()))?;

//         debug!("Initializing module: {}", arg);
//         let module = Module::new(&arg)?;

//         modules.push(module);

//         // modules.push(Module::new(&arg)?)
//     }

//     // let modules = vec

//     debug!("Starting manager...");

//     // Run the manager, this is blocking.
//     let manager = Manager::start(&PAYLOAD_CONFIG, modules);

//     Manager::join(manager);

//     Ok(())
// }
