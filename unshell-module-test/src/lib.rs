#![no_main]
#[macro_use]
extern crate log;

use std::{
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

// pub use unshell_logger::setup_logger;
pub use unshell_modules::setup_logger;
use unshell_modules::{Manager, ModuleRuntime, module_interface};

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

struct RuntimeTest {
    thread_handle: JoinHandle<()>,
}

impl RuntimeTest {
    pub fn new(manager: Arc<Mutex<Manager>>) -> RuntimeTest {
        Self {
            thread_handle: thread::spawn(move || {
                thread::sleep(Duration::from_secs(2));

                let manager_lock = manager.lock().unwrap();
                manager_lock.test1234(111.1111);
                drop(manager_lock);
            }),
        }
    }
}

impl ModuleRuntime for RuntimeTest {
    // fn init(&mut self) {}

    fn is_running(&self) -> bool {
        !self.thread_handle.is_finished()
    }

    fn kill(self: Box<Self>) {
        if !self.thread_handle.is_finished() {
            let _ = self.thread_handle.join();
        }
        // drop(self);
    }
}

#[unsafe(no_mangle)]
pub fn init(manager: Arc<Mutex<Manager>>) -> Box<dyn ModuleRuntime> {
    Box::new(RuntimeTest::new(manager))
}
