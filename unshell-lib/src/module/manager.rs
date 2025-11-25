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
    pub fn start(config: &'static PayloadConfig, modules: Vec<Module>) -> Arc<Mutex<Self>> {
        // Construct self
        let mut this = Self::new(&config.id, config.components.clone(), modules);

        debug!("Imported {} base components", this.components.len());
        debug!("Imported {} base runtimes", &config.runtime_config.len());

        // Load each of the pre-prepared modules
        this.load_components();

        let this = Arc::new(Mutex::new(this));

        debug!("Starting runtimes...");
        for runtime in &config.runtime_config {
            Self::start_runtime(this.clone(), runtime);
        }

        this
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

    /// Iterateratively loop through all runtimes, until all are finished executing
    pub fn join(this: Arc<Mutex<Self>>) {
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

    /// Start a runtime
    pub fn start_runtime<'a>(this: Arc<Mutex<Self>>, runtime: &'static RuntimeConfig) {
        let mut this_lock = this.lock().unwrap();

        let component = match this_lock.components.get(&runtime.parent_component) {
            Some(component) => component,
            None => {
                warn!(
                    "Could not find component '{}' which is referenced by runtime: {}",
                    runtime.parent_component, runtime.name
                );
                return;
            }
        };

        debug!("Starting runtime: {}", runtime.name);

        let runtime = match (*component.start_runtime)(runtime) {
            Ok(runtime) => runtime,
            Err(e) => {
                warn!("Failed to start runtime: {:?}", e);
                return;
            }
        };

        this_lock.active_runtimes.push(runtime);
    }

    pub fn get_name(&self) -> &str {
        self.id
    }
}
