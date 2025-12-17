// #![macro_use]

mod api;
mod config;
pub mod logger;
mod modules;
mod server;
pub use api::app::start_api;

pub use server::Server;

#[static_init::dynamic]
pub static DATABASE_TREES: Vec<&'static str> = vec!["users"];

#[static_init::dynamic]
pub static DEFAULT_HOST: String = "localhost".to_string();
#[static_init::dynamic]
pub static DATABASE_NAME: String = "database".to_string();

#[static_init::dynamic]
pub static SERVER_CONFIG: unshell_lib::config::PayloadConfig = unshell_lib::config::PayloadConfig {
    id: "Server",
    components: unshell_lib::get_components(),
    runtime_config: Vec::new(),
};
