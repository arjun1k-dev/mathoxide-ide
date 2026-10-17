//! Geometry builders producing VGroups, ported from manim's geometry +
//! bezier.py math (circle approximated by 8 quadratic beziers, like manim).

use crate::color;
use crate::vgroup::VGroup;
use glam::Vec3;

/// ManimGL uses 8 quadratic segments per full circle (n_segments = 8 default).
const CIRCLE_SEGMENTS: usize = 8;

pub fn circle(radius: f32, center: Vec3, fill: [f32; 4], stroke: [f32; 4], stroke_width: f32) -> VGroup {
    arc(radius, 0.0, std::f32::consts::TAU, center, fill, stroke, stroke_width)
}

pub fn arc(
    radius: f32,
    start_angle: f32,
    end_angle: f32,
    center: Vec3,
    fill: [f32; 4],
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup {
    let mut g = VGroup::new();
    let span = end_angle - start_angle;
    let n = ((span.abs() / std::f32::consts::TAU).ceil() as usize * CIRCLE_SEGMENTS).max(1);
    let dt = span / n as f32;

    let unit = |a: f32| Vec3::new(a.cos(), a.sin(), 0.0);
    let pt = |a: f32| unit(a) * radius + center;
    let mut p = VGroup::new();
    let first = pt(start_angle);
    p.start_subpath(first);
    for i in 0..n {
        let a0 = start_angle + dt * i as f32;
        let a1 = a0 + dt;
        // control at the tangent intersection: r/cos(dt/2) along the mid angle —
        // the quadratic stays inside the true radius.
        let handle = unit(a0 + dt / 2.0) * (radius / (dt / 2.0).cos()) + center;
        p.add_curve(handle, pt(a1));
    }
    if span.abs() >= std::f32::consts::TAU - 1e-4 {
        p.points.last_mut().unwrap().point = first.to_array();
    }
    let mut p = p;
    for pt_ in &mut p.points {
        pt_.fill_rgba = fill;
        pt_.stroke_rgba = stroke;
        pt_.stroke_width = stroke_width;
    }
    g.points = p.points;
    g.subpaths = p.subpaths;
    g
}

pub fn rect(width: f32, height: f32, center: Vec3, fill: [f32; 4], stroke: [f32; 4], stroke_width: f32) -> VGroup {
    let hw = width / 2.0;
    let hh = height / 2.0;
    let mut g = VGroup::new();
    let corners = [
        Vec3::new(-hw, -hh, 0.0),
        Vec3::new(hw, -hh, 0.0),
        Vec3::new(hw, hh, 0.0),
        Vec3::new(-hw, hh, 0.0),
    ];
    g.start_subpath(corners[0] + center);
    for i in 1..=4 {
        let prev = corners[i - 1];
        let cur = corners[i % 4];
        let handle = (prev + cur) / 2.0;
        g.add_curve(handle + center, cur + center);
    }
    for p in &mut g.points {
        p.fill_rgba = fill;
        p.stroke_rgba = stroke;
        p.stroke_width = stroke_width;
    }
    g
}

pub fn line(from: Vec3, to: Vec3, stroke: [f32; 4], stroke_width: f32) -> VGroup {
    let mut g = VGroup::new();
    g.start_subpath(from);
    g.add_curve((from + to) / 2.0, to);
    for p in &mut g.points {
        p.fill_rgba = [0.0; 4];
        p.stroke_rgba = stroke;
        p.stroke_width = stroke_width;
    }
    g
}

pub fn square(side: f32, center: Vec3, fill: [f32; 4], stroke: [f32; 4], stroke_width: f32) -> VGroup {
    rect(side, side, center, fill, stroke, stroke_width)
}

pub fn dot(center: Vec3, radius: f32, fill: [f32; 4]) -> VGroup {
    circle(radius, center, fill, fill, 0.0)
}

pub fn arrow(
    from: Vec3,
    to: Vec3,
    stroke: [f32; 4],
    stroke_width: f32,
    tip_length: f32,
    tip_width: f32,
) -> VGroup {
    let mut g = VGroup::new();
    let delta = to - from;
    let dist = delta.length();
    if dist < 1e-4 {
        return g;
    }
    let dir = delta / dist;
    let perp = Vec3::new(-dir.y, dir.x, 0.0);

    let tip_len = tip_length.min(dist * 0.5);
    let shaft_end = to - dir * (tip_len * 0.8);

    // Shaft
    let start_idx = g.points.len();
    g.start_subpath(from);
    g.add_curve((from + shaft_end) * 0.5, shaft_end);
    for p in &mut g.points[start_idx..] {
        p.fill_rgba = [0.0; 4];
        p.stroke_rgba = stroke;
        p.stroke_width = stroke_width;
    }

    // Tip: filled triangle
    let tip_base = to - dir * tip_len;
    let tip_left = tip_base + perp * (tip_width * 0.5);
    let tip_right = tip_base - perp * (tip_width * 0.5);

    let tip_start_idx = g.points.len();
    g.start_subpath(tip_left);
    g.add_curve((tip_left + to) * 0.5, to);
    g.add_curve((to + tip_right) * 0.5, tip_right);
    g.add_curve((tip_right + tip_left) * 0.5, tip_left);
    for p in &mut g.points[tip_start_idx..] {
        p.fill_rgba = stroke;
        p.stroke_rgba = stroke;
        p.stroke_width = stroke_width;
    }

    g
}

pub fn polygon(
    vertices: &[Vec3],
    fill: [f32; 4],
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup {
    let mut g = VGroup::new();
    let n = vertices.len();
    if n < 3 {
        return g;
    }
    g.start_subpath(vertices[0]);
    for i in 1..=n {
        let prev = vertices[i - 1];
        let cur = vertices[i % n];
        let handle = (prev + cur) * 0.5;
        g.add_curve(handle, cur);
    }
    for p in &mut g.points {
        p.fill_rgba = fill;
        p.stroke_rgba = stroke;
        p.stroke_width = stroke_width;
    }
    g
}

pub fn text_placeholder(width: f32, height: f32) -> VGroup {
    rect(width, height, Vec3::ZERO, color::hex("#33333388"), color::WHITE, 2.0)
}

/// 3D Coordinate axes with X (red), Y (cyan), and Z (purple) axes.
pub fn axes_3d(
    x_range: (f32, f32),
    y_range: (f32, f32),
    z_range: (f32, f32),
    center: Vec3,
    stroke_width: f32,
) -> VGroup {
    let mut g = VGroup::new();

    // X axis (Red)
    let x_start = center + Vec3::new(x_range.0, 0.0, 0.0);
    let x_end = center + Vec3::new(x_range.1, 0.0, 0.0);
    g.append(arrow(x_start, x_end, color::hex("#FC6255"), stroke_width, 0.35, 0.2));

    // Y axis (Cyan)
    let y_start = center + Vec3::new(0.0, y_range.0, 0.0);
    let y_end = center + Vec3::new(0.0, y_range.1, 0.0);
    g.append(arrow(y_start, y_end, color::hex("#58C4DD"), stroke_width, 0.35, 0.2));

    // Z axis (Purple)
    let z_start = center + Vec3::new(0.0, 0.0, z_range.0);
    let z_end = center + Vec3::new(0.0, 0.0, z_range.1);
    g.append(arrow(z_start, z_end, color::hex("#9A72DE"), stroke_width, 0.35, 0.2));

    g
}

/// 3D Wireframe Cube with 12 edges.
pub fn cube(side: f32, center: Vec3, stroke: [f32; 4], stroke_width: f32) -> VGroup {
    let mut g = VGroup::new();
    let s = side * 0.5;

    let v = [
        center + Vec3::new(-s, -s, -s),
        center + Vec3::new( s, -s, -s),
        center + Vec3::new( s,  s, -s),
        center + Vec3::new(-s,  s, -s),
        center + Vec3::new(-s, -s,  s),
        center + Vec3::new( s, -s,  s),
        center + Vec3::new( s,  s,  s),
        center + Vec3::new(-s,  s,  s),
    ];

    let edges = [
        (0, 1), (1, 2), (2, 3), (3, 0),
        (4, 5), (5, 6), (6, 7), (7, 4),
        (0, 4), (1, 5), (2, 6), (3, 7),
    ];

    for &(i, j) in &edges {
        g.append(line(v[i], v[j], stroke, stroke_width));
    }
    g
}

/// 3D Wireframe Sphere with latitude/longitude circles.
pub fn sphere(radius: f32, center: Vec3, stroke: [f32; 4], stroke_width: f32) -> VGroup {
    let mut g = VGroup::new();

    // Equatorial XY ring
    g.append(arc(radius, 0.0, std::f32::consts::TAU, center, [0.0; 4], stroke, stroke_width));

    // Longitudinal XZ ring
    let mut xz_ring = arc(radius, 0.0, std::f32::consts::TAU, Vec3::ZERO, [0.0; 4], stroke, stroke_width);
    for p in &mut xz_ring.points {
        let v = Vec3::from(p.point);
        p.point = [v.x, 0.0, v.y];
    }
    xz_ring.shift(center);
    g.append(xz_ring);

    // Longitudinal YZ ring
    let mut yz_ring = arc(radius, 0.0, std::f32::consts::TAU, Vec3::ZERO, [0.0; 4], stroke, stroke_width);
    for p in &mut yz_ring.points {
        let v = Vec3::from(p.point);
        p.point = [0.0, v.x, v.y];
    }
    yz_ring.shift(center);
    g.append(yz_ring);

    g
}

/// 3D Parametric Surface wireframe grid: z = f(x, y).
pub fn surface_3d<F>(
    f: F,
    x_range: (f32, f32),
    y_range: (f32, f32),
    resolution: (usize, usize),
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup
where
    F: Fn(f32, f32) -> f32,
{
    let mut g = VGroup::new();
    let (nx, ny) = (resolution.0.max(2), resolution.1.max(2));
    let dx = (x_range.1 - x_range.0) / (nx - 1) as f32;
    let dy = (y_range.1 - y_range.0) / (ny - 1) as f32;

    for j in 0..ny {
        let y = y_range.0 + (j as f32) * dy;
        let mut line_pts = Vec::with_capacity(nx);
        for i in 0..nx {
            let x = x_range.0 + (i as f32) * dx;
            let z = f(x, y);
            line_pts.push(Vec3::new(x, y, z));
        }
        for i in 0..nx - 1 {
            g.append(line(line_pts[i], line_pts[i + 1], stroke, stroke_width));
        }
    }

    for i in 0..nx {
        let x = x_range.0 + (i as f32) * dx;
        let mut line_pts = Vec::with_capacity(ny);
        for j in 0..ny {
            let y = y_range.0 + (j as f32) * dy;
            let z = f(x, y);
            line_pts.push(Vec3::new(x, y, z));
        }
        for j in 0..ny - 1 {
            g.append(line(line_pts[j], line_pts[j + 1], stroke, stroke_width));
        }
    }

    g
}

/// 3D Parametric Surface with filled shading: each grid cell is a flat quad
/// colored by a Viridis height ramp; the wireframe is height-tinted too.
/// Keeps geometry procedural and buffers lean (iGPU-friendly flat shading).
pub fn surface_3d_filled<F>(
    f: F,
    x_range: (f32, f32),
    y_range: (f32, f32),
    resolution: (usize, usize),
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup
where
    F: Fn(f32, f32) -> f32,
{
    let (nx, ny) = (resolution.0.max(2), resolution.1.max(2));
    let dx = (x_range.1 - x_range.0) / (nx - 1) as f32;
    let dy = (y_range.1 - y_range.0) / (ny - 1) as f32;

    // Sample grid once
    let mut z = vec![0.0f32; nx * ny];
    let (mut z_min, mut z_max) = (f32::INFINITY, f32::NEG_INFINITY);
    for j in 0..ny {
        for i in 0..nx {
            let v = f(x_range.0 + i as f32 * dx, y_range.0 + j as f32 * dy);
            z[j * nx + i] = v;
            z_min = z_min.min(v);
            z_max = z_max.max(v);
        }
    }
    let z_span = (z_max - z_min).max(1e-6);
    let ramp = |v: f32| crate::color::viridis((v - z_min) / z_span);

    let at = |i: usize, j: usize| Vec3::new(x_range.0 + i as f32 * dx, y_range.0 + j as f32 * dy, z[j * nx + i]);

    let mut g = VGroup::new();

    // Filled cells (flat per-cell shading), drawn first beneath the wireframe
    for j in 0..ny - 1 {
        for i in 0..nx - 1 {
            let c00 = at(i, j);
            let c10 = at(i + 1, j);
            let c11 = at(i + 1, j + 1);
            let c01 = at(i, j + 1);
            let mean_z = 0.25 * (c00[2] + c10[2] + c11[2] + c01[2]);
            let mut fill_c = ramp(mean_z);
            fill_c[3] = 0.92; // mostly opaque so back faces don't bleed through
            let mut cell = VGroup::new();
            cell.start_subpath(c00);
            cell.add_curve(0.5 * (c00 + c10), c10);
            cell.add_curve(0.5 * (c10 + c11), c11);
            cell.add_curve(0.5 * (c11 + c01), c01);
            cell.add_curve(0.5 * (c01 + c00), c00);
            cell.close();
            cell.set_fill(fill_c);
            cell.set_stroke(fill_c, 0.0);
            g.append(cell);
        }
    }

    // Height-tinted wireframe on top (darkened so the ramp reads through)
    let wire = |a: Vec3, b: Vec3, g: &mut VGroup| {
        let mut c = ramp(0.5 * (a[2] + b[2]));
        for ch in c.iter_mut().take(3) {
            *ch *= 0.45;
        }
        g.append(line(a, b, c, stroke_width));
    };
    for j in 0..ny {
        for i in 0..nx - 1 {
            wire(at(i, j), at(i + 1, j), &mut g);
        }
    }
    for i in 0..nx {
        for j in 0..ny - 1 {
            wire(at(i, j), at(i, j + 1), &mut g);
        }
    }
    let _ = stroke;
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circle_shape() {
        let c = circle(2.0, Vec3::ZERO, [0.0; 4], color::BLUE, 4.0);
        assert_eq!(c.curve_count(), CIRCLE_SEGMENTS);
        let (min, max) = c.bounds();
        assert!((max[0] - 2.0).abs() < 0.01);
        assert!((min[0] + 2.0).abs() < 0.01);
    }

    #[test]
    fn arrow_shape() {
        let a = arrow(Vec3::ZERO, Vec3::new(3.0, 0.0, 0.0), color::RED, 4.0, 0.4, 0.25);
        assert_eq!(a.subpaths.len(), 2); // shaft + tip
        let (_, max) = a.bounds();
        assert!((max[0] - 3.0).abs() < 0.05);
    }

    #[test]
    fn dot_shape() {
        let d = dot(Vec3::new(1.0, 2.0, 0.0), 0.1, color::YELLOW);
        assert_eq!(d.curve_count(), CIRCLE_SEGMENTS);
        let c = d.center();
        assert!((c.x - 1.0).abs() < 0.01);
        assert!((c.y - 2.0).abs() < 0.01);
    }
}
