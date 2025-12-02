// #![macro_use]

mod api;
pub mod database;
pub use api::app::start_api;

#[static_init::dynamic]
static DATABASE_TREES: Vec<&'static str> = vec!["users"];
