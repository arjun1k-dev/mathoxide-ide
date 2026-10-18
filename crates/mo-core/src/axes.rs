//! Coordinate axes and function graphing, matching ManimGL's CoordinateSystem.

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

    /// Plot a smooth mathematical function y = f(x) over [from_x, to_x] using NumPy-style ndarray sampling.
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
        let x_arr = crate::math::array::linspace(from_x, to_x, n);
        let y_arr = crate::math::array::eval_vectorized(&x_arr, f);
        self.plot_arrays(&x_arr, &y_arr, stroke, stroke_width)
    }

    /// Plot an analytical Expr using vectorized ndarray evaluation.
    pub fn plot_expr(
        &self,
        expr: &crate::expr::Expr,
        from_x: f32,
        to_x: f32,
        num_samples: usize,
        stroke: [f32; 4],
        stroke_width: f32,
    ) -> VGroup {
        let n = num_samples.max(4);
        let x_arr = crate::math::array::linspace(from_x, to_x, n);
        let y_arr = expr.eval_array(&x_arr);
        self.plot_arrays(&x_arr, &y_arr, stroke, stroke_width)
    }

    /// Plot directly from 1D ndarray x and y coordinate arrays.
    pub fn plot_arrays(
        &self,
        x_arr: &ndarray::Array1<f32>,
        y_arr: &ndarray::Array1<f32>,
        stroke: [f32; 4],
        stroke_width: f32,
    ) -> VGroup {
        let n = x_arr.len().min(y_arr.len());
        let mut pts = ndarray::Array2::<f32>::zeros((n, 3));
        for i in 0..n {
            let p = self.coords_to_point(x_arr[i], y_arr[i]);
            pts[[i, 0]] = p.x;
            pts[[i, 1]] = p.y;
            pts[[i, 2]] = p.z;
        }
        crate::math::bezier::smooth_curve_from_ndarray(&pts, stroke, stroke_width)
    }

    /// Plot parametric function (x(t), y(t)) over [t_min, t_max]
    pub fn plot_parametric(
        &self,
        fx: impl Fn(f32) -> f32,
        fy: impl Fn(f32) -> f32,
        t_min: f32,
        t_max: f32,
        samples: usize,
        stroke: [f32; 4],
        stroke_width: f32,
    ) -> VGroup {
        let n = samples.max(4);
        let mut pts = ndarray::Array2::<f32>::zeros((n, 3));
        let dt = (t_max - t_min) / (n - 1) as f32;
        for i in 0..n {
            let t = t_min + dt * i as f32;
            let p = self.coords_to_point(fx(t), fy(t));
            pts[[i, 0]] = p.x;
            pts[[i, 1]] = p.y;
            pts[[i, 2]] = p.z;
        }
        crate::math::bezier::smooth_curve_from_ndarray(&pts, stroke, stroke_width)
    }
}

/// 2D NumberPlane grid with major axes and background grid lines
pub fn number_plane(
    x_range: (f32, f32, f32),
    y_range: (f32, f32, f32),
    width: f32,
    height: f32,
    center: Vec3,
    axis_color: [f32; 4],
    grid_color: [f32; 4],
    axis_sw: f32,
    grid_sw: f32,
) -> VGroup {
    let axes = Axes::new(x_range, y_range, width, height, center);
    let mut g = VGroup::new();

    let (x_min, x_max, x_step) = x_range;
    let (y_min, y_max, y_step) = y_range;

    // Vertical grid lines
    if x_step > 0.0 {
        let mut x = x_min;
        while x <= x_max + 1e-4 {
            let p_bot = axes.c2p(x, y_min);
            let p_top = axes.c2p(x, y_max);
            let start_idx = g.points.len();
            g.start_subpath(p_bot);
            g.add_curve((p_bot + p_top) * 0.5, p_top);
            for p in &mut g.points[start_idx..] {
                p.fill_rgba = [0.0; 4];
                p.stroke_rgba = grid_color;
                p.stroke_width = grid_sw;
            }
            x += x_step;
        }
    }

    // Horizontal grid lines
    if y_step > 0.0 {
        let mut y = y_min;
        while y <= y_max + 1e-4 {
            let p_left = axes.c2p(x_min, y);
            let p_right = axes.c2p(x_max, y);
            let start_idx = g.points.len();
            g.start_subpath(p_left);
            g.add_curve((p_left + p_right) * 0.5, p_right);
            for p in &mut g.points[start_idx..] {
                p.fill_rgba = [0.0; 4];
                p.stroke_rgba = grid_color;
                p.stroke_width = grid_sw;
            }
            y += y_step;
        }
    }

    // Main axes on top
    g.append(axes.to_vgroup(axis_color, axis_sw));
    g
}

/// ComplexPlane with Real and Imaginary coordinate axes
pub fn complex_plane(
    real_range: (f32, f32, f32),
    imag_range: (f32, f32, f32),
    width: f32,
    height: f32,
    center: Vec3,
    axis_color: [f32; 4],
    grid_color: [f32; 4],
    axis_sw: f32,
    grid_sw: f32,
) -> VGroup {
    number_plane(real_range, imag_range, width, height, center, axis_color, grid_color, axis_sw, grid_sw)
}

/// Polar coordinate grid with concentric circles and radial spokes
pub fn polar_plane(
    radius: f32,
    n_circles: usize,
    n_spokes: usize,
    center: Vec3,
    stroke: [f32; 4],
    sw: f32,
) -> VGroup {
    let mut g = VGroup::new();
    let num_c = n_circles.max(1);
    let dr = radius / num_c as f32;

    for i in 1..=num_c {
        let r = dr * i as f32;
        g.append(crate::geom::circle(r, center, [0.0; 4], stroke, sw));
    }

    let num_s = n_spokes.max(4);
    let dt = std::f32::consts::TAU / num_s as f32;
    for i in 0..num_s {
        let a = dt * i as f32;
        let end_pt = center + Vec3::new(radius * a.cos(), radius * a.sin(), 0.0);
        g.append(crate::geom::line(center, end_pt, stroke, sw));
    }

    g
}

/// 1D NumberLine with ticks
pub fn number_line(
    range: (f32, f32, f32),
    length: f32,
    center: Vec3,
    tick_size: f32,
    stroke: [f32; 4],
    sw: f32,
) -> VGroup {
    let (min, max, step) = range;
    let span = (max - min).max(1e-4);
    let mut g = VGroup::new();

    let half_l = length * 0.5;
    let p_left = center - Vec3::new(half_l, 0.0, 0.0);
    let p_right = center + Vec3::new(half_l, 0.0, 0.0);
    g.append(crate::geom::line(p_left, p_right, stroke, sw));

    if step > 0.0 {
        let mut val = min;
        let t_half = tick_size * 0.5;
        while val <= max + 1e-4 {
            let frac = (val - min) / span;
            let x = p_left.x + frac * length;
            let top = Vec3::new(x, center.y + t_half, 0.0);
            let bot = Vec3::new(x, center.y - t_half, 0.0);
            g.append(crate::geom::line(bot, top, stroke, sw));
            val += step;
        }
    }

    g
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum RiemannKind {
    Left,
    Right,
    Midpoint,
}

/// Approximating Riemann sum rectangles under a curve y = f(x)
pub fn riemann_rectangles(
    axes: &Axes,
    f: impl Fn(f32) -> f32,
    from_x: f32,
    to_x: f32,
    n_bars: usize,
    kind: RiemannKind,
    fill: [f32; 4],
    stroke: [f32; 4],
    sw: f32,
) -> VGroup {
    let mut g = VGroup::new();
    let n = n_bars.max(1);
    let dx = (to_x - from_x) / n as f32;

    for i in 0..n {
        let x_left = from_x + dx * i as f32;
        let x_right = x_left + dx;
        let x_sample = match kind {
            RiemannKind::Left => x_left,
            RiemannKind::Right => x_right,
            RiemannKind::Midpoint => (x_left + x_right) * 0.5,
        };
        let y_val = f(x_sample);

        let p_bl = axes.c2p(x_left, 0.0);
        let p_br = axes.c2p(x_right, 0.0);
        let p_tr = axes.c2p(x_right, y_val);
        let p_tl = axes.c2p(x_left, y_val);

        let vertices = [p_bl, p_br, p_tr, p_tl];
        g.append(crate::geom::polygon(&vertices, fill, stroke, sw));
    }

    g
}

/// Secant line through (x1, f(x1)) and (x2, f(x2)) extended by `length`
pub fn secant_line(
    axes: &Axes,
    f: impl Fn(f32) -> f32,
    x1: f32,
    x2: f32,
    length: f32,
    stroke: [f32; 4],
    sw: f32,
) -> VGroup {
    let p1 = axes.c2p(x1, f(x1));
    let p2 = axes.c2p(x2, f(x2));
    let delta = p2 - p1;
    let dist = delta.length();
    if dist < 1e-4 {
        return VGroup::new();
    }
    let dir = delta / dist;
    let mid = (p1 + p2) * 0.5;
    let half_len = length * 0.5;
    crate::geom::line(mid - dir * half_len, mid + dir * half_len, stroke, sw)
}

/// Filled area under a curve y = f(x) between [from_x, to_x]
pub fn area_under_curve(
    axes: &Axes,
    f: impl Fn(f32) -> f32,
    from_x: f32,
    to_x: f32,
    samples: usize,
    fill: [f32; 4],
) -> VGroup {
    let n = samples.max(4);
    let mut vertices = Vec::with_capacity(n + 2);
    let dt = (to_x - from_x) / (n - 1) as f32;

    vertices.push(axes.c2p(from_x, 0.0));
    for i in 0..n {
        let x = from_x + dt * i as f32;
        vertices.push(axes.c2p(x, f(x)));
    }
    vertices.push(axes.c2p(to_x, 0.0));

    crate::geom::polygon(&vertices, fill, [0.0; 4], 0.0)
}

/// Bar chart of a slice of values
pub fn bar_chart(
    values: &[f32],
    width: f32,
    height: f32,
    center: Vec3,
    bar_fill: [f32; 4],
    stroke: [f32; 4],
    sw: f32,
) -> VGroup {
    let mut g = VGroup::new();
    let n = values.len();
    if n == 0 {
        return g;
    }
    let max_val = values.iter().copied().fold(0.0_f32, |a, b| a.max(b)).max(1e-4);
    let bar_slot = width / n as f32;
    let bar_w = bar_slot * 0.75;
    let left_x = center.x - width * 0.5;
    let base_y = center.y - height * 0.5;

    for (i, &v) in values.iter().enumerate() {
        let bar_h = (v.max(0.0) / max_val) * height;
        let cx = left_x + bar_slot * (i as f32 + 0.5);
        let cy = base_y + bar_h * 0.5;
        g.append(crate::geom::rect(bar_w, bar_h, Vec3::new(cx, cy, center.z), bar_fill, stroke, sw));
    }

    g
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
