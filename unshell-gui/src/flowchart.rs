use egui::Ui;
use egui::{Color32, Painter, Pos2, Rect, Stroke, UiBuilder, Vec2};

const TARGET_LINE_GAP: f32 = 80.;

const BG_STROKE: Stroke = Stroke {
    width: 0.3,
    color: Color32::GRAY,
};

#[derive(serde::Deserialize, serde::Serialize)]

pub struct FlowChart {
    // frame: Frame,
    container: DraggableContainer,
}

impl FlowChart {
    pub fn new() -> Self {
        Self {
            container: DraggableContainer::new(
                Pos2 { x: 100., y: 100. },
                Vec2 { x: 100., y: 100. },
            ),
        }
    }

    fn paint_bg(&self, rect: &Rect, painter: &Painter) {
        let h_count = (rect.width() / TARGET_LINE_GAP).round() as usize;
        let h_spacing = rect.width() / h_count as f32;
        for n in 0..h_count {
            painter.vline(rect.min.x + n as f32 * h_spacing, rect.y_range(), BG_STROKE);
        }

        let v_count = (rect.height() / TARGET_LINE_GAP).round() as usize;
        let v_spacing = rect.height() / v_count as f32;
        for n in 0..v_count {
            painter.hline(rect.x_range(), rect.min.y + n as f32 * v_spacing, BG_STROKE);
        }
    }

    pub fn paint(&mut self, ui: &mut Ui) {
        self.paint_bg(&ui.clip_rect(), ui.painter());
        self.container.show(ui, |ui, rect| {
            ui.painter().rect(
                // ui.top
                *rect,
                0.,
                Color32::PURPLE,
                BG_STROKE,
                egui::StrokeKind::Outside,
            );
            ui.label("Tests");
            let _ = ui.button("Test");
        })
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct DraggableContainer {
    pub pos: egui::Pos2,
    pub size: egui::Vec2,
    is_dragging: bool,
    drag_offset: egui::Vec2,
}

impl DraggableContainer {
    pub fn new(pos: egui::Pos2, size: egui::Vec2) -> Self {
        Self {
            pos,
            size,
            is_dragging: false,
            drag_offset: egui::Vec2::ZERO,
        }
    }

    pub fn show<R>(
        &mut self,
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&mut egui::Ui, &Rect) -> R,
    ) -> R {
        let rect = egui::Rect::from_min_size(self.pos, self.size);

        // Handle dragging logic
        let response = ui.interact(rect, ui.id().with("drag_area"), egui::Sense::drag());

        if response.drag_started() {
            self.is_dragging = true;
            if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
                self.drag_offset = self.pos - pointer_pos;
            }
        }

        if response.dragged() && self.is_dragging {
            if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
                self.pos = pointer_pos + self.drag_offset;
            }
        }

        if response.drag_stopped() {
            self.is_dragging = false;
        }

        // Create a child UI at the specified position
        // let mut child_ui = ui.child_ui(rect, egui::Layout::top_down(egui::Align::LEFT), None);

        let mut child_ui = ui.new_child(UiBuilder::new().max_rect(rect));

        // Add contents
        add_contents(&mut child_ui, &rect)
    }
}
