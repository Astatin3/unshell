use std::f32::consts::TAU;

use egui::Vec2;

use crate::flowchart::{
    ATTRACTION_STRENGTH, CENTER_ATTRACTION_STRENGTH, DAMPING, FlowChart, GROUP_ATTRACTION_STRENGTH,
    REPULSION_STRENGTH, REST_LENGTH,
};

pub fn normalize(v: &Vec2) -> Vec2 {
    let len = v.length();
    if len > 0.0 {
        Vec2 {
            x: v.x / len,
            y: v.y / len,
        }
    } else {
        Vec2 { x: 0.0, y: 0.0 }
    }
}

impl FlowChart {
    pub fn force(&mut self, delta_time: f32) {
        let num_nodes = self.containers.len();
        let mut forces = vec![Vec2::new(0.0, 0.0); num_nodes];

        // Calculate repulsive forces between all nodes
        for i in 0..num_nodes {
            for j in (i + 1)..num_nodes {
                let diff = self.containers[i].pos - self.containers[j].pos;
                let dist = diff.length().max(0.1); // Prevent division by zero
                let force = normalize(&diff) * (REPULSION_STRENGTH / (dist * dist));

                forces[i] = forces[i] + force;
                forces[j] = forces[j] + (force * -1.0);
            }
        }

        // Calculate attractive forces along connections
        for &(i, j) in &self.connections {
            let diff = self.containers[j].pos - self.containers[i].pos;
            let dist = diff.length();
            let displacement = dist - REST_LENGTH;
            let force = normalize(&diff) * (displacement * ATTRACTION_STRENGTH);

            forces[i] = forces[i] + force;
            forces[j] = forces[j] + (force * -1.0);
        }

        // Apply force to center
        for i in 0..num_nodes {
            let diff = self.containers[i].pos;
            let dist = diff.length();
            let displacement = dist - REST_LENGTH;
            let force = normalize(&diff) * (displacement * CENTER_ATTRACTION_STRENGTH);

            forces[i] = forces[i] + force * -1.;
        }

        let group_avg = &self
            .groups
            .iter()
            .map(|group| {
                let mut sum = Vec2::ZERO;
                for n in group {
                    sum += self.containers[*n].pos;
                }
                sum / group.len() as f32
            })
            .collect::<Vec<Vec2>>();

        for (group, group_avg) in self.groups.iter().zip(group_avg) {
            for i in 0..num_nodes {
                let diff = self.containers[i].pos - *group_avg;
                let dist = diff.length();
                let displacement = dist - REST_LENGTH;
                let force = normalize(&diff) * (displacement * GROUP_ATTRACTION_STRENGTH);

                if group.contains(&i) {
                    forces[i] = forces[i] + force * -1.;
                } else {
                    forces[i] = forces[i] + force;
                }
            }
        }

        // Update velocities and positions
        for i in 0..num_nodes {
            let c = &mut self.containers[i];
            c.vel = (c.vel + forces[i] * delta_time) * DAMPING;
            c.pos += c.vel * delta_time;
        }
    }

    pub fn arrange_circle(&mut self) {
        let node_count = self.containers.len() as f32;
        for (i, m) in self.containers.iter_mut().enumerate() {
            let ang = -(i as f32 / node_count) * TAU;
            m.pos = Vec2 {
                x: 300. * ang.sin(),
                y: 300. * ang.cos(),
            };
            m.vel = Vec2::ZERO;
        }
    }
}
