//! Bézier curve fitting and smoothing algorithms ported from ManimGL (manimlib/utils/bezier.py).

use crate::vgroup::VGroup;
use glam::Vec3;

/// Computes quadratic Bézier handles connecting a sequence of anchor points
/// with C1 continuity, matching ManimGL's `approx_smooth_quadratic_bezier_handles`.
pub fn approx_smooth_quadratic_bezier_handles(points: &[Vec3]) -> Vec<Vec3> {
    let n = points.len();
    if n <= 1 {
        return Vec::new();
    }
    if n == 2 {
        return vec![(points[0] + points[1]) * 0.5];
    }

    let mut smooth_to_right = Vec::with_capacity(n - 2);
    for i in 0..n - 2 {
        let p0 = points[i];
        let p1 = points[i + 1];
        let p2 = points[i + 2];
        smooth_to_right.push(p0 * 0.25 + p1 - p2 * 0.25);
    }

    let mut smooth_to_left = Vec::with_capacity(n - 2);
    for i in (2..n).rev() {
        let p0 = points[i];
        let p1 = points[i - 1];
        let p2 = points[i - 2];
        smooth_to_left.push(p0 * 0.25 + p1 - p2 * 0.25);
    }

    let is_closed = points[0].distance_squared(*points.last().unwrap()) < 1e-6;

    let (last_str, last_stl) = if is_closed {
        let last_str = points[n - 2] * 0.25 + points[n - 1] - points[1] * 0.25;
        let last_stl = points[1] * 0.25 + points[0] - points[n - 2] * 0.25;
        (last_str, last_stl)
    } else {
        (smooth_to_left[0], smooth_to_right[0])
    };

    let mut str_full = smooth_to_right;
    str_full.push(last_str);

    let mut stl_rev: Vec<Vec3> = smooth_to_left.into_iter().rev().collect();
    let mut stl_full = vec![last_stl];
    stl_full.append(&mut stl_rev);

    let num_handles = n - 1;
    let mut handles = Vec::with_capacity(num_handles);
    for i in 0..num_handles {
        let h = (str_full[i] + stl_full[i]) * 0.5;
        handles.push(h);
    }

    handles
}

/// Builds a smooth quadratic Bézier VGroup passing through a sequence of sample points.
pub fn smooth_curve_from_points(
    points: &[Vec3],
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup {
    let mut g = VGroup::new();
    if points.is_empty() {
        return g;
    }
    if points.len() == 1 {
        g.start_subpath(points[0]);
        return g;
    }

    let handles = approx_smooth_quadratic_bezier_handles(points);
    g.start_subpath(points[0]);

    for i in 0..handles.len() {
        let h = handles[i];
        let a = points[i + 1];
        g.add_curve(h, a);
    }

    g.set_stroke(stroke, stroke_width);
    g.set_fill([0.0; 4]);
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smooth_curve_sine() {
        let n = 20;
        let mut pts = Vec::with_capacity(n);
        for i in 0..n {
            let t = (i as f32) / ((n - 1) as f32) * std::f32::consts::TAU;
            pts.push(Vec3::new(t, t.sin(), 0.0));
        }

        let g = smooth_curve_from_points(&pts, [1.0, 1.0, 1.0, 1.0], 3.0);
        assert_eq!(g.curve_count(), n - 1);
        let (min, max) = g.bounds();
        assert!((min[1] - (-1.0)).abs() < 0.15);
        assert!((max[1] - 1.0).abs() < 0.15);
    }
}
