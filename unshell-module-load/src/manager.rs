use unshell_modules::{ManagerInterface, Module};

pub struct Manager {
    modules: Vec<Module>,
}

impl Manager {
    pub fn new(modules: Vec<Module>) -> Self {
        let this = Self { modules };

        let interface = this.get_interface();

        for module in &this.modules {
            if let Ok(init) = module.get_symbol::<fn(ManagerInterface)>(b"init") {
                init(interface.clone());
            } else {
                warn!("init not found");
            }
        }

        this
    }

    pub fn get_interface(&self) -> ManagerInterface {
        ManagerInterface::from_raw(Self::test123)
    }

    pub extern "C" fn test123() {
        info!("Manager Test Sucsessfull!");
    }
}
