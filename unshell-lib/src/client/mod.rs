mod client_runtime;

pub const MODULE_NAME: &'static str = "client";

// pub use unshell_modules::setup_logger;

// use unshell_modules::{Manager, ModuleRuntime, module_interface};

use crate::{
    Component,
    module::Interface,
    module_interface,
    warn,
    // module_interface,
};

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

// #[unsafe(no_mangle)]
// pub fn interface() -> Interface {
//     Interface::from_raw(test1, test2, test3)
// }

// #[unsafe(no_mangle)]
// pub fn init(manager: Arc<Mutex<Manager>>) -> Box<dyn ModuleRuntime> {
//     info!("Initializing client module");
// }

#[derive(Clone)]
pub struct ClientComponent;

impl ClientComponent {
    pub fn new() -> Self {
        ClientComponent
    }
}

impl Component for ClientComponent {
    fn name(&self) -> &'static str {
        MODULE_NAME
    }

    // fn start_runtime(&self, manager: Arc<Mutex<Manager>>) -> Option<Box<dyn ModuleRuntime>> {
    //     Some(Box::new(RuntimeTest::new(manager)))
    // }

    fn clone_box(&self) -> Box<dyn Component> {
        Box::new(self.clone())
    }

    fn get_interface(&self) -> Box<dyn Interface> {
        Box::new(ClientInterface::from_raw(test1, test2, test3))
    }
}
