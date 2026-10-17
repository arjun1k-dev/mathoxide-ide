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
}
