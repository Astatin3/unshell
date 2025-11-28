use crate::app::{AppState, AppWindow};

#[derive(serde::Deserialize, serde::Serialize)]
pub struct WindowWrapper {
    pub nr: usize,
    pub window: AppWindow,
}

impl egui_tiles::Behavior<WindowWrapper> for AppState {
    fn tab_title_for_pane(&mut self, pane: &WindowWrapper) -> egui::WidgetText {
        format!("Pane {}", pane.nr).into()
    }

    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        pane: &mut WindowWrapper,
    ) -> egui_tiles::UiResponse {
        let mut ret = egui_tiles::UiResponse::None;

        ui.horizontal(|ui| {
            let titlebar = ui.interact(
                ui.max_rect(),
                ui.id().with(&format!("Pane_{}_sense", pane.nr)),
                egui::Sense::drag(),
            );

            if titlebar.drag_started() {
                ret = egui_tiles::UiResponse::DragStarted;
            }
            if titlebar.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }

            let color = egui::epaint::Hsva::new(0.103 * pane.nr as f32, 0.5, 0.5, 1.0);
            ui.painter().rect_filled(ui.max_rect(), 0.0, color);

            ui.label("Test");
        });

        pane.window.update(self, ui);

        ret
    }
}
