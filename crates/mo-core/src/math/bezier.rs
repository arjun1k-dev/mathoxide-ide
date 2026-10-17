//! Vectorized Bézier handle fitting operating directly on `ndarray::Array2<f32>`.
//! Faithful high-performance port of ManimGL's `approx_smooth_quadratic_bezier_handles`.

use glam::Vec3;
use ndarray::{s, Array2};
use crate::vgroup::VGroup;

/// Computes quadratic Bézier handles connecting points using vectorized `ndarray` arithmetic,
/// matching ManimGL's `manimlib/utils/bezier.py`.
pub fn approx_smooth_quadratic_bezier_handles_ndarray(points: &Array2<f32>) -> Array2<f32> {
    let n = points.shape()[0];
    if n <= 1 {
        return Array2::<f32>::zeros((0, 3));
    }
    if n == 2 {
        let p0 = points.slice(s![0..1, ..]);
        let p1 = points.slice(s![1..2, ..]);
        return (&p0 + &p1) * 0.5;
    }

    // smooth_to_right = 0.25 * points[:-2] + points[1:-1] - 0.25 * points[2:]
    let p_start = points.slice(s![..n - 2, ..]);
    let p_mid = points.slice(s![1..n - 1, ..]);
    let p_end = points.slice(s![2..n, ..]);

    let smooth_to_right = &p_start * 0.25 + &p_mid - &p_end * 0.25;
    let smooth_to_left = &p_end * 0.25 + &p_mid - &p_start * 0.25;

    let p0 = points.slice(s![0, ..]);
    let p_last = points.slice(s![n - 1, ..]);
    let d2 = (&p0 - &p_last).mapv(|x| x * x).sum();
    let is_closed = d2 < 1e-6;

    let (last_str, last_stl) = if is_closed {
        let str_val = &points.slice(s![n - 2, ..]) * 0.25 + &points.slice(s![n - 1, ..]) - &points.slice(s![1, ..]) * 0.25;
        let stl_val = &points.slice(s![1, ..]) * 0.25 + &points.slice(s![0, ..]) - &points.slice(s![n - 2, ..]) * 0.25;
        (str_val, stl_val)
    } else {
        (
            smooth_to_left.slice(s![n - 3, ..]).to_owned(),
            smooth_to_right.slice(s![0, ..]).to_owned(),
        )
    };

    let num_handles = n - 1;
    let mut handles = Array2::<f32>::zeros((num_handles, 3));

    for i in 0..num_handles {
        let str_row = if i < n - 2 {
            smooth_to_right.slice(s![i, ..]).to_owned()
        } else {
            last_str.clone()
        };

        let stl_row = if i == 0 {
            last_stl.clone()
        } else {
            smooth_to_left.slice(s![i - 1, ..]).to_owned()
        };

        let h = (&str_row + &stl_row) * 0.5;
        for c in 0..3 {
            handles[[i, c]] = h[c];
        }
    }

    handles
}

/// Construct a smooth quadratic Bézier VGroup directly from an `(N, 3)` ndarray point buffer.
pub fn smooth_curve_from_ndarray(
    points: &Array2<f32>,
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup {
    let n = points.shape()[0];
    let mut g = VGroup::new();
    if n == 0 {
        return g;
    }
    let p0 = Vec3::new(points[[0, 0]], points[[0, 1]], points[[0, 2]]);
    if n == 1 {
        g.start_subpath(p0);
        return g;
    }

    let handles = approx_smooth_quadratic_bezier_handles_ndarray(points);
    g.start_subpath(p0);

    for i in 0..handles.shape()[0] {
        let h = Vec3::new(handles[[i, 0]], handles[[i, 1]], handles[[i, 2]]);
        let a = Vec3::new(points[[i + 1, 0]], points[[i + 1, 1]], points[[i + 1, 2]]);
        g.add_curve(h, a);
    }

    g.set_stroke(stroke, stroke_width);
    g.set_fill([0.0; 4]);
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::array::linspace;

    #[test]
    fn test_bezier_handles_ndarray() {
        let x = linspace(0.0, std::f32::consts::TAU, 30);
        let y = x.mapv(|v| v.sin());
        let pts = crate::math::array::stack_points_2d(&x, &y);

        let handles = approx_smooth_quadratic_bezier_handles_ndarray(&pts);
        assert_eq!(handles.shape(), &[29, 3]);

        let g = smooth_curve_from_ndarray(&pts, [1.0, 1.0, 1.0, 1.0], 3.0);
        assert_eq!(g.curve_count(), 29);
    }
}
