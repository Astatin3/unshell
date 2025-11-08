mod client_runtime;

use std::sync::{Arc, Mutex};

pub use unshell_modules::setup_logger;

use unshell_modules::{Manager, ModuleRuntime, module_interface};

use crate::client::client_runtime::RuntimeTest;

pub extern "C" fn test1() {
    warn!("Test1 called");
}
pub extern "C" fn test2() {
    warn!("Test2 called");
}
pub extern "C" fn test3() {
    warn!("Test3 called");
}

module_interface! {
    Interface {
        fn test1();
        fn test2();
        fn test3();
    }
}

#[unsafe(no_mangle)]
pub fn interface() -> Interface {
    Interface::from_raw(test1, test2, test3)
}

#[unsafe(no_mangle)]
pub fn init(manager: Arc<Mutex<Manager>>) -> Box<dyn ModuleRuntime> {
    info!("Initializing client module");
    Box::new(RuntimeTest::new(manager))
}
