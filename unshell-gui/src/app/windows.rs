use egui::Rect;

use crate::app::{AppState, AppWindow};

#[derive(serde::Deserialize, serde::Serialize)]
pub struct WindowWrapper {
    pub name: String,
    pub window: AppWindow,
}

impl egui_tiles::Behavior<WindowWrapper> for AppState {
    fn tab_title_for_pane(&mut self, pane: &WindowWrapper) -> egui::WidgetText {
        format!("{}", pane.name).into()
    }

    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        pane: &mut WindowWrapper,
    ) -> egui_tiles::UiResponse {
        let mut ret = egui_tiles::UiResponse::None;

        let mut rect = Rect::NOTHING;

        ui.horizontal(|ui| {
            rect = ui.max_rect();

            let bg_color = ui.style().visuals.extreme_bg_color;

            ui.painter().rect_filled(rect, 0.0, bg_color);

            ui.vertical_centered(|ui| {
                ui.label(&pane.name);
            });
        });

        let mut open_space = Rect::NOTHING;

        #[allow(deprecated)]
        ui.allocate_ui_at_rect(rect, |ui| {
            ui.horizontal(|ui| {
                pane.window.render_title_buttons(self, ui);
                open_space = ui.available_rect_before_wrap();
            })
        });

        let drag_sense = ui.interact(
            open_space,
            ui.id().with(&format!("Pane_{}_sense", pane.name)),
            egui::Sense::drag(),
        );

        if drag_sense.drag_started() {
            ret = egui_tiles::UiResponse::DragStarted;
        }
        if drag_sense.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }

        pane.window.update(self, ui);

        ret
    }

    fn is_tab_closable(
        &self,
        tiles: &egui_tiles::Tiles<WindowWrapper>,
        tile_id: egui_tiles::TileId,
    ) -> bool {
        match tiles.get(tile_id).unwrap() {
            egui_tiles::Tile::Pane(_) => true,
            egui_tiles::Tile::Container(_) => false,
        }
    }

    fn on_tab_close(
        &mut self,
        tiles: &mut egui_tiles::Tiles<WindowWrapper>,
        tile_id: egui_tiles::TileId,
    ) -> bool {
        match tiles.get(tile_id).unwrap() {
            egui_tiles::Tile::Pane(pane) => self.open_windows.remove(&pane.window),
            egui_tiles::Tile::Container(_) => false,
        }
    }

    fn tab_bar_color(&self, visuals: &egui::Visuals) -> egui::Color32 {
        visuals.panel_fill // same as the tab contents
    }
}
