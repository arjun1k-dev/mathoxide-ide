//! Coordinate axes and function graphing, matching ManimGL's CoordinateSystem.

use crate::curve::smooth_curve_from_points;
use crate::vgroup::VGroup;
use glam::Vec3;

#[derive(Clone, Debug)]
pub struct Axes {
    pub x_range: (f32, f32, f32), // (min, max, step)
    pub y_range: (f32, f32, f32), // (min, max, step)
    pub width: f32,
    pub height: f32,
    pub center: Vec3,
}

impl Default for Axes {
    fn default() -> Self {
        Self {
            x_range: (-6.0, 6.0, 1.0),
            y_range: (-3.0, 3.0, 1.0),
            width: 10.0,
            height: 5.0,
            center: Vec3::ZERO,
        }
    }
}

impl Axes {
    pub fn new(
        x_range: (f32, f32, f32),
        y_range: (f32, f32, f32),
        width: f32,
        height: f32,
        center: Vec3,
    ) -> Self {
        Self {
            x_range,
            y_range,
            width,
            height,
            center,
        }
    }

    /// Map graph coordinates (x, y) to scene space Vec3.
    pub fn coords_to_point(&self, x: f32, y: f32) -> Vec3 {
        let (x_min, x_max, _) = self.x_range;
        let (y_min, y_max, _) = self.y_range;

        let x_span = (x_max - x_min).max(1e-6);
        let y_span = (y_max - y_min).max(1e-6);

        let px = self.center.x + ((x - x_min) / x_span - 0.5) * self.width;
        let py = self.center.y + ((y - y_min) / y_span - 0.5) * self.height;

        Vec3::new(px, py, 0.0)
    }

    /// Map scene space Vec3 to graph coordinates (x, y).
    pub fn point_to_coords(&self, p: Vec3) -> (f32, f32) {
        let (x_min, x_max, _) = self.x_range;
        let (y_min, y_max, _) = self.y_range;

        let x_span = (x_max - x_min).max(1e-6);
        let y_span = (y_max - y_min).max(1e-6);

        let x = x_min + ((p.x - self.center.x) / self.width + 0.5) * x_span;
        let y = y_min + ((p.y - self.center.y) / self.height + 0.5) * y_span;

        (x, y)
    }

    /// Alias matching Manim's c2p
    pub fn c2p(&self, x: f32, y: f32) -> Vec3 {
        self.coords_to_point(x, y)
    }

    /// Alias matching Manim's p2c
    pub fn p2c(&self, p: Vec3) -> (f32, f32) {
        self.point_to_coords(p)
    }

    /// Build a VGroup representing the coordinate axes, ticks, and arrow tips.
    pub fn to_vgroup(&self, color: [f32; 4], stroke_width: f32) -> VGroup {
        let mut g = VGroup::new();

        let (x_min, x_max, x_step) = self.x_range;
        let (y_min, y_max, y_step) = self.y_range;

        // X-axis baseline
        let x_start = self.c2p(x_min, 0.0);
        let x_end = self.c2p(x_max, 0.0);
        g.start_subpath(x_start);
        g.add_curve((x_start + x_end) * 0.5, x_end);

        // X-axis arrow tip
        let tip_len = 0.2_f32;
        let tip_w = 0.12_f32;
        let x_arrow_top = x_end + Vec3::new(-tip_len, tip_w, 0.0);
        let x_arrow_bot = x_end + Vec3::new(-tip_len, -tip_w, 0.0);
        g.start_subpath(x_arrow_top);
        g.add_curve((x_arrow_top + x_end) * 0.5, x_end);
        g.start_subpath(x_arrow_bot);
        g.add_curve((x_arrow_bot + x_end) * 0.5, x_end);

        // Y-axis baseline
        let y_start = self.c2p(0.0, y_min);
        let y_end = self.c2p(0.0, y_max);
        g.start_subpath(y_start);
        g.add_curve((y_start + y_end) * 0.5, y_end);

        // Y-axis arrow tip
        let y_arrow_left = y_end + Vec3::new(-tip_w, -tip_len, 0.0);
        let y_arrow_right = y_end + Vec3::new(tip_w, -tip_len, 0.0);
        g.start_subpath(y_arrow_left);
        g.add_curve((y_arrow_left + y_end) * 0.5, y_end);
        g.start_subpath(y_arrow_right);
        g.add_curve((y_arrow_right + y_end) * 0.5, y_end);

        // X ticks
        let tick_half = 0.08_f32;
        if x_step > 0.0 {
            let mut x = x_min;
            while x <= x_max + 1e-4 {
                if x.abs() > 1e-4 { // skip origin
                    let pt = self.c2p(x, 0.0);
                    let top = pt + Vec3::Y * tick_half;
                    let bot = pt - Vec3::Y * tick_half;
                    g.start_subpath(bot);
                    g.add_curve((bot + top) * 0.5, top);
                }
                x += x_step;
            }
        }

        // Y ticks
        if y_step > 0.0 {
            let mut y = y_min;
            while y <= y_max + 1e-4 {
                if y.abs() > 1e-4 { // skip origin
                    let pt = self.c2p(0.0, y);
                    let right = pt + Vec3::X * tick_half;
                    let left = pt - Vec3::X * tick_half;
                    g.start_subpath(left);
                    g.add_curve((left + right) * 0.5, right);
                }
                y += y_step;
            }
        }

        g.set_stroke(color, stroke_width);
        g.set_fill([0.0; 4]);
        g
    }

    /// Plot a smooth mathematical function y = f(x) over [from_x, to_x].
    pub fn plot(
        &self,
        f: impl Fn(f32) -> f32,
        from_x: f32,
        to_x: f32,
        num_samples: usize,
        stroke: [f32; 4],
        stroke_width: f32,
    ) -> VGroup {
        let n = num_samples.max(4);
        let mut pts = Vec::with_capacity(n);
        let dx = (to_x - from_x) / ((n - 1) as f32);

        for i in 0..n {
            let x = from_x + dx * (i as f32);
            let y = f(x);
            pts.push(self.coords_to_point(x, y));
        }

        smooth_curve_from_points(&pts, stroke, stroke_width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_axes_mapping() {
        let axes = Axes::default(); // x: -6..6, y: -3..3, w: 10, h: 5
        let origin = axes.coords_to_point(0.0, 0.0);
        assert!((origin.x - axes.center.x).abs() < 1e-5);
        assert!((origin.y - axes.center.y).abs() < 1e-5);

        let roundtrip = axes.point_to_coords(origin);
        assert!((roundtrip.0 - 0.0).abs() < 1e-5);
        assert!((roundtrip.1 - 0.0).abs() < 1e-5);
    }

    #[test]
    fn test_axes_plot_sine() {
        let axes = Axes::default();
        let sine_graph = axes.plot(|x| x.sin(), -3.14159, 3.14159, 30, [1.0, 1.0, 1.0, 1.0], 3.0);
        assert!(sine_graph.curve_count() > 10);
    }
}
