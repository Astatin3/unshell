mod server_runtime;

pub use server_runtime::ListenerRuntime;

use unshell_lib::{
    ModuleError, ModuleRuntime,
    config::{InterfaceWrapper, NamedComponent, RuntimeConfig},
};

pub const COMPONENT_NAME: &'static str = "server";

fn get_interface() -> Option<&'static (dyn InterfaceWrapper + Sync)> {
    None
}

fn start_runtime(config: &'static RuntimeConfig) -> Result<Box<dyn ModuleRuntime>, ModuleError> {
    Ok(Box::new(ListenerRuntime::new(config)?))
}

#[unsafe(no_mangle)]
pub const fn get_named_component() -> NamedComponent {
    NamedComponent {
        name: COMPONENT_NAME,
        get_interface: &get_interface,
        start_runtime: &start_runtime,
    }
}
