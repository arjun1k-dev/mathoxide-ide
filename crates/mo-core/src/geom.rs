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
