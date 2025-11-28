use egui::Frame;
use egui_tiles::Tree;

use crate::app::{AppState, windows::WindowWrapper};

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
    // tab: Tab,
    state: AppState,
    tree: Tree<WindowWrapper>,
}

impl Default for TemplateApp {
    fn default() -> Self {
        // let mut tiles = egui_tiles::Tiles::default();

        // let invis_1 = tiles.insert_pane(WindowWrapper {
        //     nr: 0,
        //     window: AppWindow::None,
        // });
        // let invis_2 = tiles.insert_pane(WindowWrapper {
        //     nr: 0,
        //     window: AppWindow::None,
        // });

        // tiles.set_visible(invis_1, false);
        // tiles.set_visible(invis_2, false);
        // let root = tiles.insert_horizontal_tile(vec![invis_1]);

        Self {
            state: AppState::default(),
            tree: egui_tiles::Tree::new_horizontal("tree_root", Vec::new()),
        }
    }
}

impl TemplateApp {
    /// Called once before the first frame.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        // if let Some(storage) = cc.storage {
        //     eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        // } else {
        Default::default()
        // }
    }
}

impl eframe::App for TemplateApp {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        egui::TopBottomPanel::top("tab_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:

            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    ui.label("File");
                });

                ui.menu_button("View", |ui| {
                    ui.label("View");

                    self.state.labels(&mut self.tree, ui);
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::widgets::global_theme_preference_switch(ui);
                });

                // ui.style
            });
        });

        egui::TopBottomPanel::bottom("tab_panel").show(ctx, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("UnShell UI {}", env!("CARGO_PKG_VERSION")));
                egui::warn_if_debug_build(ui);
            });
        });

        egui::CentralPanel::default()
            .frame(Frame::central_panel(&ctx.style()).inner_margin(0))
            .show(ctx, |ui| self.tree.ui(&mut self.state, ui));
    }
}
