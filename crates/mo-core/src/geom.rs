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

/// Dashed line with configurable dash length and spacing
pub fn dashed_line(
    from: Vec3,
    to: Vec3,
    dash_length: f32,
    space_length: f32,
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup {
    let mut g = VGroup::new();
    let delta = to - from;
    let dist = delta.length();
    if dist < 1e-4 {
        return g;
    }
    let dir = delta / dist;
    let d_len = if dash_length <= 0.0 { 0.15 } else { dash_length };
    let s_len = if space_length <= 0.0 { 0.1 } else { space_length };
    let step = d_len + s_len;
    let mut cur = 0.0;
    while cur < dist {
        let p1 = from + dir * cur;
        let end_dist = (cur + d_len).min(dist);
        let p2 = from + dir * end_dist;
        let start_idx = g.points.len();
        g.start_subpath(p1);
        g.add_curve((p1 + p2) * 0.5, p2);
        for p in &mut g.points[start_idx..] {
            p.fill_rgba = [0.0; 4];
            p.stroke_rgba = stroke;
            p.stroke_width = stroke_width;
        }
        cur += step;
    }
    g
}

/// Line tangent to a point with a given angle and length
pub fn tangent_line(
    center: Vec3,
    angle: f32,
    length: f32,
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup {
    let dir = Vec3::new(angle.cos(), angle.sin(), 0.0);
    let half = dir * (length * 0.5);
    line(center - half, center + half, stroke, stroke_width)
}

/// Double-headed arrow between two points
pub fn double_arrow(
    from: Vec3,
    to: Vec3,
    stroke: [f32; 4],
    stroke_width: f32,
    tip_length: f32,
    tip_width: f32,
) -> VGroup {
    let mut g = arrow(from, to, stroke, stroke_width, tip_length, tip_width);
    let delta = from - to;
    let dist = delta.length();
    if dist >= 1e-4 {
        let dir = delta / dist;
        let perp = Vec3::new(-dir.y, dir.x, 0.0);
        let tip_len = tip_length.min(dist * 0.5);
        let tip_base = from - dir * tip_len;
        let tip_left = tip_base + perp * (tip_width * 0.5);
        let tip_right = tip_base - perp * (tip_width * 0.5);
        let tip_start_idx = g.points.len();
        g.start_subpath(tip_left);
        g.add_curve((tip_left + from) * 0.5, from);
        g.add_curve((from + tip_right) * 0.5, tip_right);
        g.add_curve((tip_right + tip_left) * 0.5, tip_left);
        for p in &mut g.points[tip_start_idx..] {
            p.fill_rgba = stroke;
            p.stroke_rgba = stroke;
            p.stroke_width = stroke_width;
        }
    }
    g
}

/// Curved arrow curving between two points by a specified angle (radians)
pub fn curved_arrow(
    from: Vec3,
    to: Vec3,
    angle: f32,
    stroke: [f32; 4],
    stroke_width: f32,
    tip_length: f32,
    tip_width: f32,
) -> VGroup {
    let chord = to - from;
    let chord_len = chord.length();
    if chord_len < 1e-4 {
        return VGroup::new();
    }
    if angle.abs() < 0.05 {
        return arrow(from, to, stroke, stroke_width, tip_length, tip_width);
    }

    let mid = (from + to) * 0.5;
    let perp = Vec3::new(-chord.y, chord.x, 0.0) / chord_len;
    let half_angle = angle * 0.5;
    let radius = (chord_len / (2.0 * half_angle.sin())).abs();
    let d = radius * half_angle.cos();
    let center = mid - perp * (angle.signum() * d);

    let start_angle = (from.y - center.y).atan2(from.x - center.x);
    let end_angle = start_angle + angle;

    let mut g = arc(radius, start_angle, end_angle, center, [0.0; 4], stroke, stroke_width);

    // Tip at `to` pointing in the direction of the tangent
    let end_tangent = Vec3::new(-end_angle.sin() * angle.signum(), end_angle.cos() * angle.signum(), 0.0);
    let tip_base = to - end_tangent * tip_length;
    let tip_perp = Vec3::new(-end_tangent.y, end_tangent.x, 0.0);
    let tip_left = tip_base + tip_perp * (tip_width * 0.5);
    let tip_right = tip_base - tip_perp * (tip_width * 0.5);

    let tip_idx = g.points.len();
    g.start_subpath(tip_left);
    g.add_curve((tip_left + to) * 0.5, to);
    g.add_curve((to + tip_right) * 0.5, tip_right);
    g.add_curve((tip_right + tip_left) * 0.5, tip_left);
    for p in &mut g.points[tip_idx..] {
        p.fill_rgba = stroke;
        p.stroke_rgba = stroke;
        p.stroke_width = stroke_width;
    }

    g
}

/// Curved arrow with arrowheads at both ends
pub fn curved_double_arrow(
    from: Vec3,
    to: Vec3,
    angle: f32,
    stroke: [f32; 4],
    stroke_width: f32,
    tip_length: f32,
    tip_width: f32,
) -> VGroup {
    let mut g = curved_arrow(from, to, angle, stroke, stroke_width, tip_length, tip_width);
    let chord = to - from;
    let chord_len = chord.length();
    if chord_len >= 1e-4 && angle.abs() >= 0.05 {
        let mid = (from + to) * 0.5;
        let perp = Vec3::new(-chord.y, chord.x, 0.0) / chord_len;
        let half_angle = angle * 0.5;
        let radius = (chord_len / (2.0 * half_angle.sin())).abs();
        let d = radius * half_angle.cos();
        let center = mid - perp * (angle.signum() * d);
        let start_angle = (from.y - center.y).atan2(from.x - center.x);
        let start_tangent = Vec3::new(start_angle.sin() * angle.signum(), -start_angle.cos() * angle.signum(), 0.0);

        let tip_base = from - start_tangent * tip_length;
        let tip_perp = Vec3::new(-start_tangent.y, start_tangent.x, 0.0);
        let tip_left = tip_base + tip_perp * (tip_width * 0.5);
        let tip_right = tip_base - tip_perp * (tip_width * 0.5);

        let tip_idx = g.points.len();
        g.start_subpath(tip_left);
        g.add_curve((tip_left + from) * 0.5, from);
        g.add_curve((from + tip_right) * 0.5, tip_right);
        g.add_curve((tip_right + tip_left) * 0.5, tip_left);
        for p in &mut g.points[tip_idx..] {
            p.fill_rgba = stroke;
            p.stroke_rgba = stroke;
            p.stroke_width = stroke_width;
        }
    }
    g
}

/// Right angle indicator / corner marker
pub fn elbow(
    corner: Vec3,
    dir1: Vec3,
    dir2: Vec3,
    size: f32,
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup {
    let u1 = dir1.normalize_or_zero() * size;
    let u2 = dir2.normalize_or_zero() * size;
    let p1 = corner + u1;
    let p2 = corner + u1 + u2;
    let p3 = corner + u2;
    let mut g = VGroup::new();
    g.start_subpath(p1);
    g.add_curve((p1 + p2) * 0.5, p2);
    g.add_curve((p2 + p3) * 0.5, p3);
    for p in &mut g.points {
        p.fill_rgba = [0.0; 4];
        p.stroke_rgba = stroke;
        p.stroke_width = stroke_width;
    }
    g
}

/// Ellipse defined by semi-major and semi-minor radii rx, ry
pub fn ellipse(
    rx: f32,
    ry: f32,
    center: Vec3,
    fill: [f32; 4],
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup {
    let mut g = circle(1.0, Vec3::ZERO, fill, stroke, stroke_width);
    for pt in &mut g.points {
        pt.point[0] = pt.point[0] * rx + center.x;
        pt.point[1] = pt.point[1] * ry + center.y;
        pt.point[2] += center.z;
    }
    g
}

/// Rectangle with rounded corners of radius `corner_radius`
pub fn rounded_rect(
    width: f32,
    height: f32,
    corner_radius: f32,
    center: Vec3,
    fill: [f32; 4],
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup {
    let r = corner_radius.min(width * 0.5).min(height * 0.5).max(0.0);
    if r < 1e-4 {
        return rect(width, height, center, fill, stroke, stroke_width);
    }
    let hw = width * 0.5;
    let hh = height * 0.5;

    let mut g = VGroup::new();
    let p_start = Vec3::new(-hw + r, hh, 0.0) + center;
    g.start_subpath(p_start);

    // Top edge
    let p_top_right = Vec3::new(hw - r, hh, 0.0) + center;
    g.add_curve((p_start + p_top_right) * 0.5, p_top_right);

    // Top-right corner
    let c_tr = Vec3::new(hw - r, hh - r, 0.0) + center;
    let tr_arc = arc(r, std::f32::consts::FRAC_PI_2, 0.0, c_tr, fill, stroke, stroke_width);
    for sp in &tr_arc.subpaths {
        for idx in sp.start..sp.end {
            if idx > sp.start && (idx - sp.start) % 2 == 0 {
                let handle = Vec3::from_array(tr_arc.points[idx - 1].point);
                let anchor = Vec3::from_array(tr_arc.points[idx].point);
                g.add_curve(handle, anchor);
            }
        }
    }

    // Right edge
    let p_bot_right = Vec3::new(hw, -hh + r, 0.0) + center;
    let p_bot_right_curve = Vec3::new(hw - r, -hh, 0.0) + center;
    g.add_curve((c_tr + Vec3::new(r, 0.0, 0.0) + p_bot_right) * 0.5, p_bot_right);

    // Bottom-right corner
    let c_br = Vec3::new(hw - r, -hh + r, 0.0) + center;
    let br_arc = arc(r, 0.0, -std::f32::consts::FRAC_PI_2, c_br, fill, stroke, stroke_width);
    for sp in &br_arc.subpaths {
        for idx in sp.start..sp.end {
            if idx > sp.start && (idx - sp.start) % 2 == 0 {
                let handle = Vec3::from_array(br_arc.points[idx - 1].point);
                let anchor = Vec3::from_array(br_arc.points[idx].point);
                g.add_curve(handle, anchor);
            }
        }
    }

    // Bottom edge
    let p_bot_left = Vec3::new(-hw + r, -hh, 0.0) + center;
    g.add_curve((p_bot_right_curve + p_bot_left) * 0.5, p_bot_left);

    // Bottom-left corner
    let c_bl = Vec3::new(-hw + r, -hh + r, 0.0) + center;
    let bl_arc = arc(r, -std::f32::consts::FRAC_PI_2, -std::f32::consts::PI, c_bl, fill, stroke, stroke_width);
    for sp in &bl_arc.subpaths {
        for idx in sp.start..sp.end {
            if idx > sp.start && (idx - sp.start) % 2 == 0 {
                let handle = Vec3::from_array(bl_arc.points[idx - 1].point);
                let anchor = Vec3::from_array(bl_arc.points[idx].point);
                g.add_curve(handle, anchor);
            }
        }
    }

    // Left edge
    let p_top_left = Vec3::new(-hw, hh - r, 0.0) + center;
    g.add_curve((c_bl - Vec3::new(r, 0.0, 0.0) + p_top_left) * 0.5, p_top_left);

    // Top-left corner back to start
    let c_tl = Vec3::new(-hw + r, hh - r, 0.0) + center;
    let tl_arc = arc(r, std::f32::consts::PI, std::f32::consts::FRAC_PI_2, c_tl, fill, stroke, stroke_width);
    for sp in &tl_arc.subpaths {
        for idx in sp.start..sp.end {
            if idx > sp.start && (idx - sp.start) % 2 == 0 {
                let handle = Vec3::from_array(tl_arc.points[idx - 1].point);
                let anchor = Vec3::from_array(tl_arc.points[idx].point);
                g.add_curve(handle, anchor);
            }
        }
    }

    for p in &mut g.points {
        p.fill_rgba = fill;
        p.stroke_rgba = stroke;
        p.stroke_width = stroke_width;
    }
    g
}

/// Regular polygon with `n` sides and circumscribed radius `radius`
pub fn regular_polygon(
    n_sides: usize,
    radius: f32,
    center: Vec3,
    fill: [f32; 4],
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup {
    let n = n_sides.max(3);
    let mut vertices = Vec::with_capacity(n);
    let dt = std::f32::consts::TAU / n as f32;
    for i in 0..n {
        let a = std::f32::consts::FRAC_PI_2 + dt * i as f32;
        vertices.push(center + Vec3::new(radius * a.cos(), radius * a.sin(), 0.0));
    }
    polygon(&vertices, fill, stroke, stroke_width)
}

/// Equilateral triangle pointing upward with side length `side`
pub fn triangle(
    side: f32,
    center: Vec3,
    fill: [f32; 4],
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup {
    let r = side / 3.0_f32.sqrt();
    regular_polygon(3, r, center, fill, stroke, stroke_width)
}

/// Annulus (ring) between inner_radius and outer_radius
pub fn annulus(
    inner_radius: f32,
    outer_radius: f32,
    center: Vec3,
    fill: [f32; 4],
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup {
    let mut g = arc(outer_radius, 0.0, std::f32::consts::TAU, center, fill, stroke, stroke_width);
    let inner = arc(inner_radius, std::f32::consts::TAU, 0.0, center, fill, stroke, stroke_width);
    g.append(inner);
    g
}

/// Sector / pie wedge of a circle from start_angle to end_angle
pub fn sector(
    radius: f32,
    start_angle: f32,
    end_angle: f32,
    center: Vec3,
    fill: [f32; 4],
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup {
    let mut g = arc(radius, start_angle, end_angle, center, fill, stroke, stroke_width);
    let start_pt = center + Vec3::new(radius * start_angle.cos(), radius * start_angle.sin(), 0.0);
    let end_pt = center + Vec3::new(radius * end_angle.cos(), radius * end_angle.sin(), 0.0);
    g.add_curve((end_pt + center) * 0.5, center);
    g.add_curve((center + start_pt) * 0.5, start_pt);
    for p in &mut g.points {
        p.fill_rgba = fill;
        p.stroke_rgba = stroke;
        p.stroke_width = stroke_width;
    }
    g
}

/// Annular sector (curved ring slice) between inner_radius and outer_radius
pub fn annular_sector(
    inner_radius: f32,
    outer_radius: f32,
    start_angle: f32,
    end_angle: f32,
    center: Vec3,
    fill: [f32; 4],
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup {
    let outer_arc = arc(outer_radius, start_angle, end_angle, center, fill, stroke, stroke_width);
    let inner_arc = arc(inner_radius, end_angle, start_angle, center, fill, stroke, stroke_width);
    let mut g = outer_arc;
    let end_outer = center + Vec3::new(outer_radius * end_angle.cos(), outer_radius * end_angle.sin(), 0.0);
    let end_inner = center + Vec3::new(inner_radius * end_angle.cos(), inner_radius * end_angle.sin(), 0.0);
    g.add_curve((end_outer + end_inner) * 0.5, end_inner);
    for sp in &inner_arc.subpaths {
        for idx in sp.start..sp.end {
            if idx > sp.start && (idx - sp.start) % 2 == 0 {
                let handle = Vec3::from_array(inner_arc.points[idx - 1].point);
                let anchor = Vec3::from_array(inner_arc.points[idx].point);
                g.add_curve(handle, anchor);
            }
        }
    }
    let start_inner = center + Vec3::new(inner_radius * start_angle.cos(), inner_radius * start_angle.sin(), 0.0);
    let start_outer = center + Vec3::new(outer_radius * start_angle.cos(), outer_radius * start_angle.sin(), 0.0);
    g.add_curve((start_inner + start_outer) * 0.5, start_outer);
    for p in &mut g.points {
        p.fill_rgba = fill;
        p.stroke_rgba = stroke;
        p.stroke_width = stroke_width;
    }
    g
}

/// Star polygon with `n_points`, alternating between outer and inner radius
pub fn star(
    n_points: usize,
    outer_radius: f32,
    inner_radius: f32,
    center: Vec3,
    fill: [f32; 4],
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup {
    let n = n_points.max(3);
    let total_pts = n * 2;
    let dt = std::f32::consts::TAU / total_pts as f32;
    let mut vertices = Vec::with_capacity(total_pts);
    for i in 0..total_pts {
        let r = if i % 2 == 0 { outer_radius } else { inner_radius };
        let a = std::f32::consts::FRAC_PI_2 + dt * i as f32;
        vertices.push(center + Vec3::new(r * a.cos(), r * a.sin(), 0.0));
    }
    polygon(&vertices, fill, stroke, stroke_width)
}

/// Surrounding bounding rectangle around an existing target mobject with margin `buff`
pub fn surrounding_rect(
    target: &VGroup,
    buff: f32,
    corner_radius: f32,
    fill: [f32; 4],
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup {
    let (min, max) = target.bounds();
    let w = (max[0] - min[0]) + 2.0 * buff;
    let h = (max[1] - min[1]) + 2.0 * buff;
    let center = Vec3::new((min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5, 0.0);
    if corner_radius > 0.001 {
        rounded_rect(w, h, corner_radius, center, fill, stroke, stroke_width)
    } else {
        rect(w, h, center, fill, stroke, stroke_width)
    }
}

/// Background rectangle behind a mobject for contrast / callout
pub fn background_rect(target: &VGroup, buff: f32, fill: [f32; 4]) -> VGroup {
    surrounding_rect(target, buff, 0.0, fill, [0.0; 4], 0.0)
}

/// Mathematical curly brace along the vector from `from` to `to` with given `depth`
pub fn brace(
    from: Vec3,
    to: Vec3,
    depth: f32,
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup {
    let mut g = VGroup::new();
    let u_vec = to - from;
    let len = u_vec.length();
    if len < 1e-4 {
        return g;
    }
    let u = u_vec / len;
    let n = Vec3::new(-u.y, u.x, 0.0); // perpendicular normal pointing in direction of brace tip
    let d = if depth <= 0.0 { 0.4 } else { depth };

    let p0 = from;
    let p_mid = (from + to) * 0.5;
    let p_tip = p_mid + n * d;
    let p3 = to;

    let q1 = from + u * (len * 0.25) + n * (d * 0.5);
    let q2 = to - u * (len * 0.25) + n * (d * 0.5);

    g.start_subpath(p0);
    g.add_curve(from + n * (d * 0.5), q1);
    g.add_curve(p_mid - u * (len * 0.1) + n * (d * 0.5), p_tip);
    g.add_curve(p_mid + u * (len * 0.1) + n * (d * 0.5), q2);
    g.add_curve(to + n * (d * 0.5), p3);

    for p in &mut g.points {
        p.fill_rgba = [0.0; 4];
        p.stroke_rgba = stroke;
        p.stroke_width = stroke_width;
    }
    g
}

/// Cross / X-mark over a target mobject
pub fn cross(target: &VGroup, buff: f32, stroke: [f32; 4], stroke_width: f32) -> VGroup {
    let (min, max) = target.bounds();
    let p1 = Vec3::new(min[0] - buff, min[1] - buff, 0.0);
    let p2 = Vec3::new(max[0] + buff, max[1] + buff, 0.0);
    let p3 = Vec3::new(min[0] - buff, max[1] + buff, 0.0);
    let p4 = Vec3::new(max[0] + buff, min[1] - buff, 0.0);
    let mut g = line(p1, p2, stroke, stroke_width);
    g.append(line(p3, p4, stroke, stroke_width));
    g
}

/// Underline drawn directly below a mobject
pub fn underline(target: &VGroup, buff: f32, stroke: [f32; 4], stroke_width: f32) -> VGroup {
    let (min, max) = target.bounds();
    let y = min[1] - buff;
    line(Vec3::new(min[0], y, 0.0), Vec3::new(max[0], y, 0.0), stroke, stroke_width)
}

/// 3D Cylinder with circular top/bottom caps and longitudinal wire ribs
pub fn cylinder(
    radius: f32,
    height: f32,
    center: Vec3,
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup {
    let mut g = VGroup::new();
    let hh = height * 0.5;
    let top_center = center + Vec3::new(0.0, 0.0, hh);
    let bot_center = center - Vec3::new(0.0, 0.0, hh);

    let mut top_cap = circle(radius, Vec3::ZERO, [0.0; 4], stroke, stroke_width);
    for pt in &mut top_cap.points {
        pt.point[2] += hh;
    }
    let mut bot_cap = circle(radius, Vec3::ZERO, [0.0; 4], stroke, stroke_width);
    for pt in &mut bot_cap.points {
        pt.point[2] -= hh;
    }
    g.append(top_cap);
    g.append(bot_cap);

    // 8 longitudinal ribs
    let n_ribs = 8;
    for i in 0..n_ribs {
        let a = std::f32::consts::TAU * (i as f32 / n_ribs as f32);
        let x = radius * a.cos();
        let y = radius * a.sin();
        let p_top = top_center + Vec3::new(x, y, 0.0);
        let p_bot = bot_center + Vec3::new(x, y, 0.0);
        g.append(line(p_bot, p_top, stroke, stroke_width));
    }
    g
}

/// 3D Cone with circular base and apex
pub fn cone(
    radius: f32,
    height: f32,
    center: Vec3,
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup {
    let mut g = VGroup::new();
    let hh = height * 0.5;
    let base_center = center - Vec3::new(0.0, 0.0, hh);
    let apex = center + Vec3::new(0.0, 0.0, hh);

    let mut base_cap = circle(radius, Vec3::ZERO, [0.0; 4], stroke, stroke_width);
    for pt in &mut base_cap.points {
        pt.point[2] -= hh;
    }
    g.append(base_cap);

    let n_ribs = 8;
    for i in 0..n_ribs {
        let a = std::f32::consts::TAU * (i as f32 / n_ribs as f32);
        let x = radius * a.cos();
        let y = radius * a.sin();
        let p_base = base_center + Vec3::new(x, y, 0.0);
        g.append(line(p_base, apex, stroke, stroke_width));
    }
    g
}

/// 3D line in space
pub fn line_3d(from: Vec3, to: Vec3, stroke: [f32; 4], stroke_width: f32) -> VGroup {
    line(from, to, stroke, stroke_width)
}

/// 3D spherical dot in space
pub fn dot_3d(center: Vec3, radius: f32, stroke: [f32; 4], stroke_width: f32) -> VGroup {
    sphere(radius, center, stroke, stroke_width)
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
///
/// `wire_stride` decimates the overlay wireframe (draw one line every k cells)
/// so `res` controls fill smoothness without exploding curve count. A stride
/// of 1 draws every cell edge; 0 picks an automatic stride that keeps the
/// wireframe near ~20k curves.
pub fn surface_3d_filled_with_stride<F>(
    f: F,
    x_range: (f32, f32),
    y_range: (f32, f32),
    resolution: (usize, usize),
    stroke: [f32; 4],
    stroke_width: f32,
    wire_stride: usize,
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

    // Height-tinted wireframe on top (darkened so the ramp reads through).
    // Strided so dense grids don't multiply curve count: the fill already
    // carries the shape, the wire only needs to read as a grid.
    let stride = if wire_stride == 0 {
        // keep total wire curves near ~20k
        let cells = (nx - 1).max(ny - 1);
        ((cells + 149) / 150).max(1)
    } else {
        wire_stride.max(1)
    };
    let wire = |a: Vec3, b: Vec3, g: &mut VGroup| {
        let mut c = ramp(0.5 * (a[2] + b[2]));
        for ch in c.iter_mut().take(3) {
            *ch *= 0.45;
        }
        g.append(line(a, b, c, stroke_width));
    };
    for j in (0..ny).step_by(stride) {
        for i in (0..nx - 1).step_by(stride) {
            wire(at(i, j), at(i + 1, j), &mut g);
        }
    }
    for i in (0..nx).step_by(stride) {
        for j in (0..ny - 1).step_by(stride) {
            wire(at(i, j), at(i, j + 1), &mut g);
        }
    }
    let _ = stroke;
    g
}

/// `surface_3d_filled` with automatic wireframe decimation (default).
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
    surface_3d_filled_with_stride(f, x_range, y_range, resolution, stroke, stroke_width, 0)
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
