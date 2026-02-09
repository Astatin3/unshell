use egui::Pos2;

/// Calculate the convex hull of a set of points using Graham scan
pub fn convex_hull(points: &[Pos2]) -> Vec<Pos2> {
    if points.len() < 3 {
        return points.to_vec();
    }

    let mut pts = points.to_vec();

    // Find the point with lowest y-coordinate (and leftmost if tie)
    let start_idx = pts
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            a.y.partial_cmp(&b.y)
                .unwrap()
                .then(a.x.partial_cmp(&b.x).unwrap())
        })
        .unwrap()
        .0;

    pts.swap(0, start_idx);
    let start = pts[0];

    // Sort points by polar angle with respect to start point
    pts[1..].sort_by(|a, b| {
        let angle_a = polar_angle_to(&start, a);
        let angle_b = polar_angle_to(&start, b);
        angle_a.partial_cmp(&angle_b).unwrap()
    });

    // Build convex hull
    let mut hull = Vec::new();
    hull.push(pts[0]);
    hull.push(pts[1]);

    for i in 2..pts.len() {
        while hull.len() > 1
            && cross_product(&hull[hull.len() - 2], &hull[hull.len() - 1], &pts[i]) <= 0.0
        {
            hull.pop();
        }
        hull.push(pts[i]);
    }

    hull
}

/// Calculate cross product of vectors (self->p2) and (self->p3)
/// Positive if counter-clockwise, negative if clockwise, zero if collinear
fn cross_product(p1: &Pos2, p2: &Pos2, p3: &Pos2) -> f32 {
    (p2.x - p1.x) * (p3.y - p1.y) - (p2.y - p1.y) * (p3.x - p1.x)
}

/// Calculate polar angle from self to other point
fn polar_angle_to(a: &Pos2, b: &Pos2) -> f32 {
    (b.y - a.y).atan2(b.x - a.x)
}
