use egui::Frame;
use egui_tiles::Tree;

use crate::{
    FORCE_REDRAW_DELAY,
    app::{AppState, windows::WindowWrapper},
};

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
        Self {
            state: AppState::default(),
            tree: egui_tiles::Tree::new_horizontal("tree_root", Vec::new()),
        }
    }
}

impl TemplateApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        }
    }
}

impl eframe::App for TemplateApp {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.state.auth.logged_in() {
            egui::CentralPanel::default()
                .frame(Frame::central_panel(&ctx.style()).inner_margin(0))
                .show(ctx, |ui| self.state.auth.update(ui));
        } else {
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

        ctx.request_repaint_after(FORCE_REDRAW_DELAY);
    }
}
