#![no_main]
#[macro_use]
extern crate log;

pub use unshell_logger::setup_logger;
use unshell_modules::{module, module_interface};

// #[unsafe(no_mangle)]
extern "C" fn test1() {
    warn!("Test1 called");
}
// #[unsafe(no_mangle)]
extern "C" fn test2() {
    warn!("Test2 called");
}
// #[unsafe(no_mangle)]
extern "C" fn test3() {
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
pub fn test() {
    info!("Module loaded");
}

#[unsafe(no_mangle)]
pub fn functions() -> Interface {
    info!("Module loaded");
    // let m = TestModule::new();
    let i = unsafe { Interface::from_raw(test1, test2, test3) };

    i.test1();

    i
}

#[unsafe(no_mangle)]
pub fn testfunc() {
    info!("testfunc called");
}
