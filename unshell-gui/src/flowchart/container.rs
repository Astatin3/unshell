use egui::{Pos2, Rect, UiBuilder, Vec2};

#[derive(serde::Deserialize, serde::Serialize)]
pub struct DraggableContainer {
    pub pos: egui::Vec2, // Offset from center of clip_rect
    pub size: egui::Vec2,

    is_dragging: bool,
    drag_offset: egui::Vec2,
    drag_id: String,

    pub vel: egui::Vec2,
}

impl DraggableContainer {
    // pub fn new(center_offset: egui::Vec2, size: egui::Vec2, id: usize) -> Self {
    //     Self {
    //         pos: center_offset,
    //         size,
    //         is_dragging: false,
    //         drag_offset: egui::Vec2::ZERO,
    //         drag_id: format!("flowchart_drag_area{}", id),
    //         vel: Vec2::ZERO,
    //     }
    // }

    pub fn new_zero(id: usize) -> Self {
        Self {
            pos: Vec2::ZERO,
            size: Vec2 { x: 100., y: 100. },
            is_dragging: false,
            drag_offset: egui::Vec2::ZERO,
            drag_id: format!("flowchart_drag_area{}", id),
            vel: Vec2::ZERO,
        }
    }

    pub fn get_pos(&self, center: &Pos2) -> Pos2 {
        center.clone() + self.pos
    }

    pub fn show<R>(
        &mut self,
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&mut egui::Ui, &Rect) -> R,
    ) -> R {
        // Calculate center of the clip rect
        let clip_center = Pos2::ZERO;

        // Calculate actual position from center offset
        let center_pos = clip_center + self.pos;
        let pos = center_pos - self.size / 2.0; // Top-left corner from center
        let rect = egui::Rect::from_min_size(pos, self.size);

        // Handle dragging logic
        let response = ui.interact(rect, ui.id().with(&self.drag_id), egui::Sense::drag());

        // if response.secondary_clicked() {

        // }

        if response.drag_started() {
            self.is_dragging = true;
            if let Some(pointer_pos) = ui.input(|i| i.pointer.latest_pos()) {
                let pointer_pos = ui
                    .ctx()
                    .layer_transform_from_global(ui.painter().layer_id())
                    .unwrap_or_default()
                    * pointer_pos;
                self.drag_offset = center_pos - pointer_pos;
            }
        }

        if response.dragged() && self.is_dragging {
            // Pointer code from https://github.com/emilk/egui/pull/7149
            if let Some(pointer_pos) = ui.input(|i| i.pointer.latest_pos()) {
                let pointer_pos = ui
                    .ctx()
                    .layer_transform_from_global(ui.painter().layer_id())
                    .unwrap_or_default()
                    * pointer_pos;
                let new_center = pointer_pos + self.drag_offset;
                self.pos = new_center - clip_center;
            }

            // egui::Frame::default()
            //     .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
            //     .corner_radius(ui.visuals().widgets.noninteractive.corner_radius)
            //     .show(ui, |ui| {
            //         ui.label(egui::RichText::new("Content").color(egui::Color32::WHITE));
            //         // self.frame.show(ui, |ui| {
            //         //     ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            //         //     ui.label(egui::RichText::new("Content").color(egui::Color32::WHITE));
            //         // });
            //     });
        }

        if response.drag_stopped() {
            self.is_dragging = false;
        }

        // Create a child UI at the specified position
        let mut child_ui = ui.new_child(UiBuilder::new().max_rect(rect));

        // Add contents
        add_contents(&mut child_ui, &rect)
    }
}
