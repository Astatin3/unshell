use unshell_modules::{Manager, Module, ModuleError, module_interface};

#[macro_use]
extern crate log;

module_interface! {
    Interface {
        fn test1();
        fn test2();
        fn test3();
    }
}

fn main() {
    // Init the logger (This uses like 600MB of storage)
    pretty_env_logger::init();

    info!("Initialized");

    match || -> Result<(), ModuleError> {
        let args = std::env::args();

        let mut modules = Vec::new();
        for arg in args.skip(1) {
            info!("Loading module: {}", arg);
            modules.push(Module::new(&arg)?)
        }
        Manager::run(modules);
        // manager.join();

        Ok(())
    }() {
        Ok(_) => {}
        Err(e) => {
            error!("ERROR! {:?}", e);
        }
    }
}
