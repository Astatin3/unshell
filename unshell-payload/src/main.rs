use unshell_lib::{
    ModuleError,
    module::{Manager, Module},
};

#[macro_use]
extern crate unshell_lib;

fn main() {
    #[cfg(not(feature = "obfuscate"))]
    unshell_lib::logger::PrettyLogger::init();

    debug!("Initialized");

    match || -> Result<(), ModuleError> {
        let args = std::env::args();

        let mut modules = Vec::new();
        for arg in args.skip(1) {
            debug!("Loading module: {}", arg);
            modules.push(Module::new(&arg)?)
        }
        Manager::run(modules);

        Ok(())
    }() {
        Ok(_) => {}
        Err(e) => {
            debug!("ERROR! {:?}", e);
        }
    }
}
