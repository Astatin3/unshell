use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use unshell_obfuscate::symbol;

use crate::*;
use module::Module;

// #[derive(Debug)]
pub struct Manager {
    modules: Vec<Module>,
    components: HashMap<&'static str, Box<dyn Component>>,
}

// static mut MANAGER_RUNTIME: Option<Arc<Mutex<Manager>>> = None;

impl Manager {
    /// Create Manager, and run initilization for each Module
    #[allow(static_mut_refs)]
    pub fn run<'a>(modules: Vec<Module>) {
        let this: Self = Self::load_modules(modules);
        let components = this.components.clone();

        let this = Arc::new(Mutex::new(this));

        let mut runtimes: Vec<Box<dyn ModuleRuntime>> = Vec::new();

        for (name, component) in components {
            let module_runtime = component.start_runtime(this.clone());
            if let Some(module_runtime) = module_runtime {
                info!("Initialized {}", name);
                runtimes.push(module_runtime);
            }
        }

        Self::join(&mut runtimes);
    }

    pub fn load_modules<'a>(modules: Vec<Module>) -> Self {
        let module_count = modules.len();
        let mut this = Self {
            modules,
            components: HashMap::new(),
        };

        // let mut runtimes = Vec::new();

        info!("Symbol name: {}", symbol!("get_components"));

        for i in 0..module_count {
            info!("Importing module {}", i);
            // let this_lock = .unwrap();
            let component_func = if let Ok(component_func) = this.modules[i]
                .get_symbol::<fn() -> HashMap<&'static str, Box<dyn Component>>>(
                    symbol!("get_components").as_bytes(),
                ) {
                component_func
            } else {
                warn!("get_components function not found");
                continue;
            };

            let components = component_func();

            let len = components.len();
            info!("[{}] Loaded {} components", i, len);

            this.components.extend(components);
        }

        this
    }

    /// Iterateratively loop through all runtimes, until all are finished executing
    pub fn join(runtimes: &mut Vec<Box<dyn ModuleRuntime>>) {
        // let mut len = runtimes.len().clone();
        while runtimes.len() > 0 {
            runtimes.retain(|runtime| runtime.is_running());

            thread::sleep(Duration::from_micros(100));
        }
    }

    pub fn get_component(&self) -> HashMap<&'static str, Box<dyn Component>> {
        self.components.clone()
    }

    // pub extern "C" fn test1234(&self, float: f32) {
    //     info!("Manager Test Sucsessfull! {}", float.powf(2.));
    // }

    // #[allow(static_mut_refs, improper_ctypes_definitions)]
    // pub extern "C" fn get_manager() -> Arc<Mutex<Manager>> {
    //     unsafe { MANAGER_RUNTIME.clone().unwrap() }
    // }
}
