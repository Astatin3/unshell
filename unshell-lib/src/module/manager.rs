use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use crate::{
    config::{NamedComponent, PayloadConfig, RuntimeConfig},
    *,
};
use module::Module;
use unshell_obfuscate::symbol;

// #[derive(Debug)]
pub struct Manager {
    id: &'static str,

    pub modules: Vec<Module>,

    active_runtimes: Vec<Box<dyn ModuleRuntime>>,
    components: HashMap<String, NamedComponent>,
}

// static mut MANAGER_RUNTIME: Option<Arc<Mutex<Manager>>> = None;

impl Manager {
    fn new(id: &'static str, components: Vec<NamedComponent>, modules: Vec<Module>) -> Self {
        Self {
            id,
            modules,
            components: components
                .into_iter()
                .map(|c| (c.name.to_string(), c))
                .collect(),
            active_runtimes: Vec::new(),
        }
    }

    /// Create Manager, and run initilization for each Module
    #[allow(static_mut_refs)]
    pub fn run(config: &'static PayloadConfig, modules: Vec<Module>) {
        // Construct self
        let mut this = Self::new(&config.id, config.components.clone(), modules);

        debug!("Imported {} base components", this.components.len());
        debug!("Imported {} base runtimes", &config.runtime_config.len());

        // Load each of the pre-prepared modules
        this.load_components();

        let this = Arc::new(Mutex::new(this));

        Self::start_runtimes(this.clone(), &config.runtime_config);

        // drop(config);

        Self::join(this);
    }

    fn load_components(&mut self) {
        for module in &self.modules {
            // Load get_components function from shared object library
            let component_func = match module
                .get_symbol::<fn() -> Vec<NamedComponent>>(symbol!("get_components").as_bytes())
            {
                Ok(func) => func,
                Err(_) => {
                    warn!("get_components function not found");
                    continue;
                }
            };

            let components = component_func();
            let component_name = "TODO"; //TODO: Make this actually load component name

            debug!("{} - Retrieved payload metadata", component_name);

            // Add each component into self
            for c in components {
                debug!("{} - Found component '{}'", "TODO", c.name);
                self.components.insert(c.name.to_owned(), c);
            }
        }
    }

    /// Start each runtime
    fn start_runtimes(this: Arc<Mutex<Self>>, runtimes: &'static Vec<RuntimeConfig>) {
        debug!("Starting runtimes...");
        for runtime in runtimes {
            let mut this_lock = this.lock().unwrap();

            let component = match this_lock.components.get(&runtime.parent_component) {
                Some(component) => component,
                None => {
                    warn!(
                        "Could not find component '{}' which is referenced by runtime: {}",
                        runtime.parent_component, runtime.name
                    );
                    continue;
                }
            };

            debug!("Starting runtime: {}", runtime.name);

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
                debug!("There are no more runtimes! Exiting...");
                break;
            }

            this_lock.active_runtimes.retain(|runtime| {
                if runtime.is_running() {
                    true
                } else {
                    debug!("Runtime exited!"); //TODO: Make this better
                    false
                }
            });

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
