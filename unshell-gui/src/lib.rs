#![warn(clippy::all, rust_2018_idioms)]
#![macro_use]
// #[allow(unused_extern_crates)]
// extern crate log;

pub mod app;
mod auth;
mod blobs;
mod config;
mod flowchart;
mod interface;
mod log_viewer;
mod payload_config;

use std::time::Duration;
const FORCE_REDRAW_DELAY: Duration = Duration::from_millis(300);

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

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub async fn run() {
    use wasm_bindgen::JsCast as _;

    use app::TemplateApp;

    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    let document = web_sys::window()
        .expect("No window")
        .document()
        .expect("No document");

    let canvas = document
        .get_element_by_id("the_canvas_id")
        .expect("Failed to find the_canvas_id")
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .expect("the_canvas_id was not a HtmlCanvasElement");

    let start_result = eframe::WebRunner::new()
        .start(
            canvas,
            web_options,
            Box::new(|cc| Ok(Box::new(TemplateApp::new(cc)))),
        )
        .await;

    // Remove the loading text and spinner:
    if let Some(loading_text) = document.get_element_by_id("loading_text") {
        match start_result {
            Ok(_) => {
                loading_text.remove();
            }
            Err(e) => {
                loading_text.set_inner_html(
                    "<p> The app has crashed. See the developer console for details. </p>",
                );
                panic!("Failed to start eframe: {e:?}");
            }
        }
    }
}

// }
// #[cfg(not(target_arch = "wasm32"))]
// mod JsFunc {

//     pub fn httpGet(url: &str, callback: fn() -> {}) {}
// }
