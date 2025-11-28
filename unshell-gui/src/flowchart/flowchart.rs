use egui::{Color32, Painter, Pos2, Rect, Scene, Shape, Ui};

use crate::flowchart::CONNECTION_STROKE;
use crate::flowchart::GROUP_BORDER_MARGIN;
use crate::flowchart::container::DraggableContainer;
use crate::flowchart::group::convex_hull;
use crate::flowchart::{BG_STROKE, TARGET_LINE_GAP};

#[derive(serde::Deserialize, serde::Serialize)]

pub struct FlowChart {
    scene_rect: Rect,
    pub containers: Vec<DraggableContainer>,
    pub connections: Vec<(usize, usize)>,
    pub groups: Vec<Vec<usize>>,
}

impl Default for FlowChart {
    fn default() -> Self {
        let mut this = Self {
            scene_rect: Rect::ZERO,
            containers: vec![
                DraggableContainer::new_zero(0),
                DraggableContainer::new_zero(1),
                DraggableContainer::new_zero(2),
                DraggableContainer::new_zero(3),
                DraggableContainer::new_zero(4),
                DraggableContainer::new_zero(5),
                DraggableContainer::new_zero(6),
                DraggableContainer::new_zero(7),
            ],
            connections: vec![(0, 1), (1, 2), (1, 3), (1, 4), (3, 5), (3, 6), (3, 7)],
            groups: vec![vec![1, 3, 5, 7]],
        };

        this.arrange_circle();

        this
    }
}

impl FlowChart {
    fn paint_bg(rect: &Rect, painter: &Painter) {
        let h_start = (rect.min.x / TARGET_LINE_GAP).round() as i32;
        let h_end = ((rect.min.x + rect.width()) / TARGET_LINE_GAP).round() as i32 + 1;
        for n in h_start..h_end {
            painter.vline(n as f32 * TARGET_LINE_GAP, rect.y_range(), BG_STROKE);
        }

        let v_start = (rect.min.y / TARGET_LINE_GAP).round() as i32;
        let v_end = ((rect.min.y + rect.height()) / TARGET_LINE_GAP).round() as i32 + 1;
        for n in v_start..v_end {
            painter.hline(rect.x_range(), n as f32 * TARGET_LINE_GAP, BG_STROKE);
        }
    }

    fn paint_groups(groups: &Vec<Vec<usize>>, containers: &Vec<DraggableContainer>, ui: &mut Ui) {
        for group in groups {
            let mut points = Vec::new();

            for n in group {
                let container = &containers[*n];
                let pos = container.pos.to_pos2();
                let size = container.size;
                points.append(&mut vec![
                    Pos2 {
                        x: pos.x - size.x / 2. - GROUP_BORDER_MARGIN,
                        y: pos.y - size.x / 2. - GROUP_BORDER_MARGIN,
                    },
                    Pos2 {
                        x: pos.x + size.x / 2. + GROUP_BORDER_MARGIN,
                        y: pos.y - size.y / 2. - GROUP_BORDER_MARGIN,
                    },
                    Pos2 {
                        x: pos.x - size.x / 2. - GROUP_BORDER_MARGIN,
                        y: pos.y + size.y / 2. + GROUP_BORDER_MARGIN,
                    },
                    Pos2 {
                        x: pos.x + size.x / 2. + GROUP_BORDER_MARGIN,
                        y: pos.y + size.y / 2. + GROUP_BORDER_MARGIN,
                    },
                ]);
            }

            let points = convex_hull(&points);

            ui.painter().add(Shape::convex_polygon(
                points,
                Color32::DEBUG_COLOR,
                BG_STROKE,
            ));
        }
    }

    pub fn paint(&mut self, ui: &mut Ui) {
        let scene = Scene::new()
            // .max_inner_size([350.0, 1000.0])
            .zoom_range(0.1..=2.0);

        let containers = &mut self.containers;
        let groups = &self.groups;

        let mut inner_rect = Rect::NAN;
        let rect = &self.scene_rect.clone();

        let response = scene
            .show(ui, &mut self.scene_rect, |mut ui| {
                Self::paint_bg(rect, ui.painter());
                Self::paint_groups(groups, containers, &mut ui);

                for (a, b) in &self.connections {
                    ui.painter().line_segment(
                        [containers[*a].pos.to_pos2(), containers[*b].pos.to_pos2()],
                        CONNECTION_STROKE,
                    );
                }

                for container in containers {
                    container.show(&mut ui, |ui, rect| {
                        ui.painter().rect(
                            // ui.top
                            *rect,
                            0.,
                            Color32::PURPLE,
                            BG_STROKE,
                            egui::StrokeKind::Outside,
                        );
                    });
                }

                inner_rect = ui.min_rect();
            })
            .response;

        if response.double_clicked() {
            self.scene_rect = inner_rect;
        }

        if ui.button("Arrange").clicked() {
            for _ in 0..1_000 {
                self.force(0.1);
            }
        }
    }
}
