#[macro_use]
extern crate log;

use libloading::{Library, Symbol};
use log::{info, warn};
use unshell_logger::SetupLogger;
use unshell_modules::module_interface;

module_interface! {
    Interface {
        fn test1();
        fn test2();
        fn test3();
    }
}

#[allow(dead_code)]
#[derive(Debug)]
enum ModuleError {
    LibLoadingError(libloading::Error),
    LinkError(String),
}

struct Module {
    name: String,
    lib: Library,
}

impl Module {
    pub fn new(path: &str) -> Result<Self, ModuleError> {
        let lib = unsafe { Library::new(&path) }.map_err(|e| ModuleError::LibLoadingError(e))?;

        Ok(Self {
            name: path.to_owned(),
            lib,
        })
    }
    pub fn get_symbol<T>(&self, symbol: &[u8]) -> Result<Symbol<'_, T>, ModuleError> {
        let symbol = unsafe { self.lib.get::<T>(symbol) }
            .map_err(|e| ModuleError::LinkError(format!("Failed to load symbol: {}", e)))?;

        Ok(symbol)
    }
    pub fn init_logger(&self) {
        if let Ok(setup_logger) = self.get_symbol::<SetupLogger>(b"setup_logger") {
            setup_logger(log::logger(), log::max_level()).unwrap();
        } else {
            warn!("setup_logger not found");
        }
    }
    pub fn get_interface<T>(&self) -> Result<T, ModuleError> {
        if let Ok(interface_function) = self.get_symbol::<fn() -> T>(b"interface") {
            Ok(interface_function())
        } else {
            Err(ModuleError::LinkError(format!(
                "Interface function not found!"
            )))
        }
    }
}

fn main() {
    pretty_env_logger::init();

    info!("Initalized");

    match || -> Result<(), ModuleError> {
        let module =
            Module::new("../unshell-module-test/target/release/libunshell_module_test.so")?;
        module.init_logger();

        let interface = module.get_interface::<Interface>()?;

        interface.test1();

        Ok(())
    }() {
        Ok(_) => {}
        Err(e) => {
            error!("ERROR! {:?}", e);
        }
    }
}
