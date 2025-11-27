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

    // pub fn show<R>(
    //     &mut self,
    //     ui: &mut egui::Ui,
    //     add_contents: impl FnOnce(&mut egui::Ui, &Rect) -> R,
    // ) -> R {
    //     let rect = egui::Rect::from_min_size(self.pos, self.size);

    //     // Handle dragging logic
    //     let response = ui.interact(rect, ui.id().with(&self.drag_id), egui::Sense::drag());

    //     if response.drag_started() {
    //         self.is_dragging = true;
    //         if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
    //             self.drag_offset = self.pos - pointer_pos;
    //         }
    //     }

    //     if response.dragged() && self.is_dragging {
    //         if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
    //             self.pos = pointer_pos + self.drag_offset;
    //         }
    //     }

    //     if response.drag_stopped() {
    //         self.is_dragging = false;
    //     }

    //     // Create a child UI at the specified position
    //     // let mut child_ui = ui.child_ui(rect, egui::Layout::top_down(egui::Align::LEFT), None);

    //     let mut child_ui = ui.new_child(UiBuilder::new().max_rect(rect));

    //     // Add contents
    //     add_contents(&mut child_ui, &rect)
    // }

    pub fn get_pos(&self, center: &Pos2) -> Pos2 {
        center.clone() + self.pos
    }

    pub fn show<R>(
        &mut self,
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&mut egui::Ui, &Rect) -> R,
    ) -> R {
        // Calculate center of the clip rect
        let clip_center = ui.clip_rect().center();

        // Calculate actual position from center offset
        let center_pos = clip_center + self.pos;
        let pos = center_pos - self.size / 2.0; // Top-left corner from center
        let rect = egui::Rect::from_min_size(pos, self.size);

        // Handle dragging logic
        let response = ui.interact(rect, ui.id().with(&self.drag_id), egui::Sense::drag());

        if response.drag_started() {
            self.is_dragging = true;
            if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
                self.drag_offset = center_pos - pointer_pos;
            }
        }

        if response.dragged() && self.is_dragging {
            if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
                let new_center = pointer_pos + self.drag_offset;
                self.pos = new_center - clip_center;
            }
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
