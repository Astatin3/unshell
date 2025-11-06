#![no_main]
#[macro_use]
extern crate log;

pub use unshell_logger::setup_logger;
use unshell_modules::module_interface;

extern "C" fn test1() {
    warn!("Test1 called");
}
extern "C" fn test2() {
    warn!("Test2 called");
}
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
pub fn interface() -> Interface {
    Interface::from_raw(test1, test2, test3)
}

#[unsafe(no_mangle)]
pub fn testfunc() {
    info!("testfunc called");
}
