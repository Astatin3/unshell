use libloading::{Library, Symbol};

use crate::module::proc_load::memfd_create_dlopen;
use crate::{ModuleError, logger::SetupLogger, logger::logger};

use crate::*;

pub struct Module {
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

    // TODO: Implement actual reflective ELF loading (possibly even custom format)
    // Look at https://github.com/weizhiao/rust-elfloader
    pub fn new_bytes(bytes: &[u8]) -> Result<Self, ModuleError> {
        let lib =
            memfd_create_dlopen(bytes).map_err(|e| ModuleError::Error(e.to_string().into()))?;

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
}
