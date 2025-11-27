use egui::Shape;
use egui::Ui;
use egui::{Color32, Painter, Pos2, Rect};

use crate::flowchart::CONNECTION_STROKE;
use crate::flowchart::GROUP_BORDER_MARGIN;
use crate::flowchart::container::DraggableContainer;
use crate::flowchart::group::convex_hull;
use crate::flowchart::{BG_STROKE, TARGET_LINE_GAP};

#[derive(serde::Deserialize, serde::Serialize)]

pub struct FlowChart {
    pub containers: Vec<DraggableContainer>,
    pub connections: Vec<(usize, usize)>,
    pub groups: Vec<Vec<usize>>,
}

impl FlowChart {
    pub fn new() -> Self {
        let mut this = Self {
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
            groups: vec![],
        };

        this.arrange_circle();

        this
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

    fn paint_groups(&self, ui: &mut Ui) {
        let center = ui.clip_rect().center();
        for group in &self.groups {
            let mut points = Vec::new();

            for n in group {
                let container = &self.containers[*n];
                let pos = container.get_pos(&center);
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
        self.paint_bg(&ui.clip_rect(), ui.painter());
        self.paint_groups(ui);

        let center = ui.clip_rect().center();

        for (a, b) in &self.connections {
            ui.painter().line_segment(
                [
                    self.containers[*a].get_pos(&center),
                    self.containers[*b].get_pos(&center),
                ],
                CONNECTION_STROKE,
            );

            // let start = self.containers[m.clone()];
            // let end = self.containers[n.clone()];
        }

        for container in &mut self.containers {
            container.show(ui, |ui, rect| {
                ui.painter().rect(
                    // ui.top
                    *rect,
                    0.,
                    Color32::PURPLE,
                    BG_STROKE,
                    egui::StrokeKind::Outside,
                );
                // ui.label("Tests");
                // let _ = ui.button("Test");
            });
        }

        if ui.button("Arrange").clicked() {
            // let positions: Vec<Vec2> = (0..num_nodes)
            //     .map(|i| {
            //         let angle = (i as f32) * 2.0 * std::f32::consts::PI / (num_nodes as f32);
            //         Vec2::new(angle.cos() * 100.0, angle.sin() * 100.0)
            //     })
            //     .collect();

            // let node_count = self.containers.len() as f32;

            // for (i, m) in self.containers.iter_mut().enumerate() {
            //     let ang = -(i as f32 / node_count) * PI * 2.;
            //     m.pos = Vec2 {
            //         x: 1000. * ang.sin(),
            //         y: 1000. * ang.cos(),
            //     };
            //     m.vel = Vec2::ZERO;
            // }

            for _ in 0..1_000 {
                self.force(0.1);
            }
        }
    }
}
