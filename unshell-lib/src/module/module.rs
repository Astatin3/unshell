use libloading::{Library, Symbol};

use crate::{ModuleError, logger::SetupLogger, logger::logger};

use crate::*;

pub struct Module {
    // name: String,
    lib: Library,
}

impl Module {
    pub fn new(path: &str) -> Result<Self, ModuleError> {
        let lib = unsafe { Library::new(&path) }.map_err(|e| ModuleError::LibLoadingError(e))?;

        let this = Self { lib };

        if let Ok(setup_logger) = this.get_symbol::<SetupLogger>(b"setup_logger") {
            setup_logger(logger());
        } else {
            warn!("setup_logger not found");
        }

        Ok(this)
    }
    pub fn get_symbol<T>(&self, symbol: &[u8]) -> Result<Symbol<'_, T>, ModuleError> {
        let symbol = unsafe { self.lib.get::<T>(symbol) }
            .map_err(|e| ModuleError::LinkError(format!("Failed to load symbol: {}", e)))?;

        Ok(symbol)
    }
    // pub fn get_interface<T>(&self) -> Result<T, ModuleError> {
    //     if let Ok(interface_function) = self.get_symbol::<fn() -> T>(b"interface") {
    //         Ok(interface_function())
    //     } else {
    //         Err(ModuleError::LinkError(format!(
    //             "Interface function not found!"
    //         )))
    //     }
    // }
}

// extern "C" fn test1234() {
//     info!("Test1234!");
// }
