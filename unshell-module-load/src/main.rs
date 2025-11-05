use std::any::Any;

use libloading::{Library, Symbol};
use log::{info, warn};
use unshell_logger::SetupLogger;
use unshell_modules::{module, module_interface};
// use unshell_modules::IntoFunctionPtr;
// use unshell_modules::test;
// use unshell_modules::ExportFunction;

// fn test1() {
//     warn!("Test1 not called");
// }

// fn test2() {
//     warn!("Test2 not called");
// }
// fn test3() {
//     warn!("Test3 not called");
// }

module_interface! {
    Interface {
        fn test1();
        fn test2();
        fn test3();
    }
}

fn main() {
    // println!("Hello, world!");

    // test();

    pretty_env_logger::init();

    warn!("Warning message");

    unsafe {
        let lib = Library::new("../unshell-module-test/target/release/libunshell_module_test.so")
            .unwrap();

        let ret = lib.get::<SetupLogger>(b"setup_logger");

        if let Ok(setup_logger) = ret {
            setup_logger(log::logger(), log::max_level()).unwrap();
        } else {
            warn!("setup_logger not found");
        }

        let module = lib.get::<fn() -> Interface>(b"functions").unwrap();

        let i = module();

        // i.test1();

        info!("Func: {:?}", i.test1);

        i.test1();
    }
}
