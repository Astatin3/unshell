mod container;
mod flowchart;
mod force;
mod group;

use egui::{Color32, Stroke};
pub use flowchart::FlowChart;

const ITERATIONS: usize = 1_000;
const RESOLUTION: f32 = 0.6;

const TARGET_LINE_GAP: f32 = 80.;

const BG_STROKE: Stroke = Stroke {
    width: 0.3,
    color: Color32::GRAY,
};

const CONNECTION_STROKE: Stroke = Stroke {
    width: 3.,
    color: Color32::WHITE,
};

const GROUP_BORDER_MARGIN: f32 = 20.;

static REPULSION_STRENGTH: f32 = 100000.0; // repulsion_strength
static ATTRACTION_STRENGTH: f32 = 0.01; // attraction_strength
static CENTER_ATTRACTION_STRENGTH: f32 = 0.01; // attraction_strength
static GROUP_ATTRACTION_STRENGTH: f32 = 0.01; // attraction_strength
static REST_LENGTH: f32 = 50.0; // rest_length
static DAMPING: f32 = 0.9; // damping
