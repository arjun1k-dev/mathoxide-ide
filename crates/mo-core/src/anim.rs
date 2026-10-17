//! Animations: enum-based, snapshot-lerp model.
//! Per frame: state = lerp(snapshot, target, rate(alpha)).

use crate::vgroup::VGroup;
use glam::Vec3;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Animation {
    /// Blend the whole mobject toward `target` (Transform / MoveTo in manim).
    Transform { id: String, target: VGroup },
    /// Fade in: opacity 0->1 (plus optional shift), mobject ends visible.
    FadeIn { id: String, shift: Option<Vec3> },
    FadeOut { id: String, shift: Option<Vec3> },
    /// Reveal curves left to right (ShowCreation / Write).
    ShowCreation { id: String },
    /// Scale from 0 at an anchor point (GrowFromCenter / GrowFromEdge).
    Grow { id: String, anchor: Vec3 },
    Rotate { id: String, angle: f32 },
    /// Spin in from nothing: scales from 0 to 1 while rotating 360 degrees
    SpinInFromNothing { id: String },
    /// Shrink to center: scales down to 0 at center while fading out
    ShrinkToCenter { id: String },
    /// Indicate / Flash: pulses slightly larger with a highlight color
    Indicate { id: String, scale_factor: f32, color: Option<[f32; 4]> },
    /// Wiggle: rotates back and forth around center with decaying amplitude
    Wiggle { id: String, angle: f32 },
}

impl Animation {
    pub fn target_id(&self) -> &str {
        match self {
            Animation::Transform { id, .. }
            | Animation::FadeIn { id, .. }
            | Animation::FadeOut { id, .. }
            | Animation::ShowCreation { id, .. }
            | Animation::Grow { id, .. }
            | Animation::Rotate { id, .. }
            | Animation::SpinInFromNothing { id }
            | Animation::ShrinkToCenter { id }
            | Animation::Indicate { id, .. }
            | Animation::Wiggle { id, .. } => id,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TimedAnim {
    pub anim: Animation,
    pub run_time: f32,
    /// Easing name (resolved via rate::by_name at sample time).
    pub rate: String,
}

impl TimedAnim {
    pub fn new(anim: Animation, run_time: f32, rate: &str) -> Self {
        Self { anim, run_time, rate: rate.into() }
    }
}

/// Compute the rendered VGroup for `current` under `anim` at normalized time
/// `t` in [0, run_time). Returns None if the object should not be visible yet.
pub fn sample(anim: &TimedAnim, snapshot: &VGroup, target: &VGroup, t: f32) -> Option<VGroup> {
    let alpha = (t / anim.run_time).clamp(0.0, 1.0);
    let a = (crate::rate::by_name(&anim.rate).unwrap_or(crate::rate::smooth))(alpha);
    let mut out = snapshot.clone();
    match &anim.anim {
        Animation::Transform { .. } => {
            let (mut a_pts, b_pts) = snapshot.align_with(target);
            a_pts.interpolate(&a_pts.clone(), &b_pts, a);
            Some(a_pts)
        }
        Animation::ShowCreation { .. } => {
            let has_fill = target.points.iter().any(|p| p.fill_rgba[3] > 0.0);
            let has_stroke = target.points.iter().any(|p| p.stroke_width > 0.0 && p.stroke_rgba[3] > 0.0);

            if !has_fill && has_stroke {
                // Purely stroked curves, coordinate axes, and path lines:
                // trace path progressively with exact stroke width
                let partial = target.partial(0.0, a);
                Some(partial)
            } else {
                // Text, LaTeX formulas, or filled shapes:
                let subs = target.extract_subpaths();
                if subs.len() <= 1 {
                    // Single filled shape: smooth fade in with exact styling
                    let mut out = target.clone();
                    fade(&mut out, a, false);
                    Some(out)
                } else {
                    // Multi-glyph text / equation: write glyphs sequentially from left to right
                    let m = subs.len();
                    let mut out = VGroup::new();
                    let window = 0.25_f32;
                    let step = (1.0 - window) / (m.saturating_sub(1).max(1) as f32);

                    for (k, mut sub) in subs.into_iter().enumerate() {
                        let start_k = (k as f32) * step;
                        if a <= start_k {
                            continue;
                        }
                        let p = ((a - start_k) / window).clamp(0.0, 1.0);
                        fade(&mut sub, p, false);

                        let start_idx = out.points.len();
                        out.points.extend(sub.points);
                        out.subpaths.push(crate::vgroup::SubPath {
                            start: start_idx,
                            end: out.points.len(),
                        });
                    }
                    Some(out)
                }
            }
        }
        Animation::FadeIn { shift, .. } => {
            out.interpolate(snapshot, target, 1.0);
            if let Some(sh) = shift {
                out.shift(*sh * (1.0 - a));
            }
            fade(&mut out, a, false);
            Some(out)
        }
        Animation::FadeOut { shift, .. } => {
            out = target.clone();
            if let Some(sh) = shift {
                out.shift(*sh * a);
            }
            fade(&mut out, 1.0 - a, false);
            Some(out)
        }
        Animation::Grow { anchor, .. } => {
            out.interpolate(snapshot, target, 1.0);
            let anchor = *anchor;
            let s = a;
            let c = anchor;
            for p in &mut out.points {
                let q = Vec3::from(p.point);
                p.point = (c + (q - c) * s).to_array();
            }
            fade(&mut out, a, false);
            Some(out)
        }
        Animation::Rotate { angle, .. } => {
            out.interpolate(snapshot, target, 1.0);
            let c = target.center();
            out.rotate_z(*angle * a, c);
            Some(out)
        }
        Animation::SpinInFromNothing { .. } => {
            out.interpolate(snapshot, target, 1.0);
            let c = target.center();
            let s = a;
            for p in &mut out.points {
                let q = Vec3::from(p.point);
                p.point = (c + (q - c) * s).to_array();
            }
            out.rotate_z(std::f32::consts::TAU * (1.0 - a), c);
            fade(&mut out, a, false);
            Some(out)
        }
        Animation::ShrinkToCenter { .. } => {
            out = target.clone();
            let c = target.center();
            let s = 1.0 - a;
            for p in &mut out.points {
                let q = Vec3::from(p.point);
                p.point = (c + (q - c) * s).to_array();
            }
            fade(&mut out, 1.0 - a, false);
            Some(out)
        }
        Animation::Indicate { scale_factor, color, .. } => {
            out.interpolate(snapshot, target, 1.0);
            let c = target.center();
            let pulse = (std::f32::consts::PI * a).sin();
            let s = 1.0 + (*scale_factor - 1.0) * pulse;
            for p in &mut out.points {
                let q = Vec3::from(p.point);
                p.point = (c + (q - c) * s).to_array();
            }
            if let Some(c_hi) = color {
                for p in &mut out.points {
                    for j in 0..4 {
                        p.stroke_rgba[j] = p.stroke_rgba[j] * (1.0 - pulse) + c_hi[j] * pulse;
                    }
                }
            }
            Some(out)
        }
        Animation::Wiggle { angle, .. } => {
            out.interpolate(snapshot, target, 1.0);
            let c = target.center();
            let decay = 1.0 - a;
            let wiggle_angle = *angle * (std::f32::consts::TAU * 3.0 * a).sin() * decay;
            out.rotate_z(wiggle_angle, c);
            Some(out)
        }
    }
}

/// Multiply alpha channels (approximation of manim's family fade).
fn fade(g: &mut VGroup, factor: f32, _stroke_only: bool) {
    for p in &mut g.points {
        p.fill_rgba[3] *= factor;
        p.stroke_rgba[3] *= factor;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom;

    #[test]
    fn transform_sampling() {
        let from = geom::circle(1.0, Vec3::ZERO, [0.0; 4], crate::color::BLUE, 4.0);
        let mut to = from.clone();
        to.shift(Vec3::new(3.0, 0.0, 0.0));
        let anim = TimedAnim::new(Animation::Transform { id: "c".into(), target: to.clone() }, 2.0, "smooth");
        let mid = sample(&anim, &from, &to, 1.0).unwrap();
        let cx = mid.center().x;
        assert!(cx > 1.0 && cx < 2.0, "mid x = {cx}");
        let end = sample(&anim, &from, &to, 2.0).unwrap();
        assert!((end.center().x - 3.0).abs() < 0.01);
    }

    #[test]
    fn creation_partial() {
        let c = geom::circle(2.0, Vec3::ZERO, [0.0; 4], crate::color::BLUE, 4.0);
        let anim = TimedAnim::new(Animation::ShowCreation { id: "c".into() }, 1.0, "linear");
        let half = sample(&anim, &VGroup::new(), &c, 0.5).unwrap();
        assert_eq!(half.curve_count(), c.curve_count() / 2);
    }
}
