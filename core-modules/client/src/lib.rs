mod client_runtime;

pub const MODULE_NAME: &'static str = "client";

use std::any::TypeId;

use unshell_lib::{
    ModuleError,
    ModuleRuntime,
    // client::client_runtime::ClientRuntime,
    config::{InterfaceWrapper, NamedComponent, RuntimeConfig},
    module_interface,
    warn, // module_interface,
};

use crate::client_runtime::ClientRuntime;

pub use unshell_lib::logger::setup_logger;

pub extern "C" fn test1() {
    warn!("Test1 called xxxxxxxxxxx");
}
pub extern "C" fn test2() {
    warn!("Test2 called");
}
pub extern "C" fn test3() {
    warn!("Test3 called");
}

module_interface! {
    ClientInterface {
        fn test1();
        fn test2();
        fn test3();
    }
}

pub struct ClientInterfaceWrapper;

impl InterfaceWrapper for ClientInterfaceWrapper {
    fn get_interface<T: 'static>(&self) -> Option<T>
    where
        Self: Sized,
    {
        if TypeId::of::<T>() == TypeId::of::<ClientInterface>() {
            let my_struct = ClientInterface::from_raw(test1, test2, test3);

            unsafe { Some(std::mem::transmute_copy(&my_struct)) }
        } else {
            None
        }
    }
}

fn get_interface() -> Option<&'static (dyn InterfaceWrapper + Sync)> {
    Some(&ClientInterfaceWrapper)
}

fn start_runtime(config: &'static RuntimeConfig) -> Result<Box<dyn ModuleRuntime>, ModuleError> {
    Ok(Box::new(ClientRuntime::new(config)?))
}

#[unsafe(no_mangle)]
pub fn get_components() -> Vec<NamedComponent> {
    vec![NamedComponent {
        name: MODULE_NAME,
        get_interface: &get_interface,
        start_runtime: &start_runtime,
    }]
}
