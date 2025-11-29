#![warn(clippy::all, rust_2018_idioms)]
#![macro_use]
#[allow(unused_extern_crates)]
extern crate log;

pub mod app;
mod auth;
mod config;
mod flowchart;

// mod JsFunc {
// use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub fn log(text: &str);

}

#[wasm_bindgen(module = "/assets/sw.js")]
extern "C" {
    pub fn httpGet(url: &str, ok_callback: JsValue);
    pub fn httpPost(url: &str, data: &str, ok_callback: JsValue);
    pub fn httpGetAuth(url: &str, auth: String, ok_callback: JsValue);
    pub fn httpPostAuth(url: &str, auth: String, data: &str, ok_callback: JsValue);
}

// }
// #[cfg(not(target_arch = "wasm32"))]
// mod JsFunc {

//     pub fn httpGet(url: &str, callback: fn() -> {}) {}
// }
