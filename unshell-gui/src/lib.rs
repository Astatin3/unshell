#![warn(clippy::all, rust_2018_idioms)]
#![macro_use]
#[allow(unused_extern_crates)]
extern crate log;

mod app;
pub use app::TemplateApp;

mod config;

mod flowchart;
