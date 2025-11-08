use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use crate::{Module, ModuleRuntime};

pub struct Manager {
    modules: Vec<Module>,
}

static mut MANAGER_RUNTIME: Option<Arc<Mutex<Manager>>> = None;

impl Manager {
    /// Create Manager, and run initilization for each Module
    #[allow(static_mut_refs)]
    pub fn run<'a>(modules: Vec<Module>) {
        let module_count = modules.len();
        let this = Self { modules };

        let this = Arc::new(Mutex::new(this));

        let mut runtimes = Vec::new();

        for i in 0..module_count {
            info!("Initializing {}", i);
            let this_lock = this.lock().unwrap();
            let init = if let Ok(init) =
                this_lock.modules[i]
                    .get_symbol::<fn(Arc<Mutex<Manager>>) -> Box<dyn ModuleRuntime>>(b"init")
            {
                init.to_owned()
            } else {
                warn!("init not found");
                continue;
            };

            let runtime = init(this.clone());

            info!("Initialized {}", i);

            runtimes.push(runtime);
        }

        Self::join(&mut runtimes);
    }

    /// Iterateratively loop through all runtimes, until all are finished executing
    pub fn join(runtimes: &mut Vec<Box<dyn ModuleRuntime>>) {
        // let mut len = runtimes.len().clone();
        while runtimes.len() > 0 {
            runtimes.retain(|runtime| runtime.is_running());

            thread::sleep(Duration::from_micros(100));
        }
    }

    // pub extern "C" fn test123() {
    //     info!("Manager Test Sucsessfull!");
    // }

    pub extern "C" fn test1234(&self, float: f32) {
        info!("Manager Test Sucsessfull! {}", float.powf(2.));
    }

    #[allow(static_mut_refs, improper_ctypes_definitions)]
    pub extern "C" fn get_manager() -> Arc<Mutex<Manager>> {
        unsafe { MANAGER_RUNTIME.clone().unwrap() }
    }
}
