// #![no]

#[macro_use]
extern crate log;

mod manager;

use std::sync::{Arc, Mutex};

use unshell_modules::{Module, ModuleError, module_interface};

use crate::manager::Manager;

module_interface! {
    Interface {
        fn test1();
        fn test2();
        fn test3();
    }
}

// const modules: Arc<Mutex<Vec<Module>>> = Arc::new(Mutex::new(Vec::new()));

fn main() {
    pretty_env_logger::init();

    info!("Initalized");

    match || -> Result<(), ModuleError> {
        let args = std::env::args();

        let mut modules = Vec::new();
        for arg in args.skip(1) {
            modules.push(Module::new(&arg)?)
        }
        let _manager = Manager::new(modules);

        // for i in 1..args.len() {}

        // let interface = module.get_interface::<Interface>()?;

        // interface.test1();

        Ok(())
    }() {
        Ok(_) => {}
        Err(e) => {
            error!("ERROR! {:?}", e);
        }
    }
}
