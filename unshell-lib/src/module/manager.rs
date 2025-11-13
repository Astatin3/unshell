use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use unshell_obfuscate::symbol;

use crate::{
    config::{NamedComponent, PayloadConfig, RuntimeConfig},
    *,
};
use module::Module;

// #[derive(Debug)]
pub struct Manager<'a> {
    id: &'static str,

    modules: Vec<Module>,

    active_runtimes: Vec<&'a dyn ModuleRuntime>,
    // runtime_config: Vec<RuntimeConfig>,
    components: HashMap<String, &'a NamedComponent>,
}

// static mut MANAGER_RUNTIME: Option<Arc<Mutex<Manager>>> = None;

impl<'a> Manager<'a> {
    fn new(id: &'static str, config: &'a Vec<NamedComponent>, modules: Vec<Module>) -> Self {
        Self {
            id,

            // config,
            modules,

            components: config.iter().map(|c| (c.name.to_string(), c)).collect(),

            active_runtimes: Vec::new(),
        }
    }

    /// Create Manager, and run initilization for each Module
    #[allow(static_mut_refs)]
    pub fn run(config: &'static PayloadConfig, modules: Vec<Module>) {
        // Construct self
        let this = Self::new(&config.id, &config.components, modules);

        // Load each of the pre-prepared modules
        // this.load_components();

        let this = Arc::new(Mutex::new(this));

        Self::start_runtimes(this.clone(), &config.runtime_config);

        // let components = this.components.clone();

        // let mut runtimes: Vec<Box<dyn ModuleRuntime>> = Vec::new();

        // for (_name, component) in components {
        //     let module_runtime = component.start_runtime(this.clone());
        //     if let Some(module_runtime) = module_runtime {
        //         runtimes.push(module_runtime);
        //     }
        // }

        Self::join(this);
    }

    // fn load_components(&mut self) {
    //     for i in 0..self.modules.len() {
    //         debug!("Importing module {}", i);
    //         // let this_lock = .unwrap();
    //         let component_func = if let Ok(component_func) = self.modules[i]
    //             .get_symbol::<fn() -> HashMap<&'static str, Box<dyn Component>>>(
    //                 symbol!("get_components").as_bytes(),
    //             ) {
    //             component_func
    //         } else {
    //             warn!("get_components function not found");
    //             continue;
    //         };

    //         let components = component_func();

    //         let len = components.len();
    //         debug!("[{}] Loaded {} components", i, len);

    //         self.components.extend(components);
    //     }
    // }

    /// Start each runtime
    fn start_runtimes(this: Arc<Mutex<Self>>, runtimes: &'static Vec<RuntimeConfig>) {
        for runtime in runtimes {
            let mut this_lock = this.lock().unwrap();

            let component = match this_lock.components.get(runtime.parent_component) {
                Some(component) => component,
                None => {
                    warn!(
                        "Could not find component {} which is referenced by runtime {}",
                        runtime.parent_component, runtime.name
                    );
                    continue;
                }
            };

            let runtime = match (*component.start_runtime)(runtime) {
                Ok(runtime) => runtime,
                Err(e) => {
                    warn!("Failed to start runtime: {:?}", e);
                    continue;
                }
            };

            this_lock.active_runtimes.push(runtime);
        }
    }

    /// Iterateratively loop through all runtimes, until all are finished executing
    fn join(this: Arc<Mutex<Self>>) {
        loop {
            let mut this_lock = this.lock().unwrap();

            if this_lock.active_runtimes.len() <= 0 {
                break;
            }

            this_lock
                .active_runtimes
                .retain(|runtime| runtime.is_running());

            drop(this_lock);

            thread::sleep(Duration::from_millis(500));
        }
    }

    // pub fn get_component(&self) -> HashMap<&'static str, Box<dyn Component>> {
    //     self.components.clone()
    // }

    pub fn get_name(&self) -> &str {
        self.id
    }

    // pub extern "C" fn test1234(&self, float: f32) {
    //     info!("Manager Test Sucsessfull! {}", float.powf(2.));
    // }

    // #[allow(static_mut_refs, improper_ctypes_definitions)]
    // pub extern "C" fn get_manager() -> Arc<Mutex<Manager>> {
    //     unsafe { MANAGER_RUNTIME.clone().unwrap() }
    // }
}
