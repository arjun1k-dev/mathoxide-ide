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
}

impl Animation {
    pub fn target_id(&self) -> &str {
        match self {
            Animation::Transform { id, .. }
            | Animation::FadeIn { id, .. }
            | Animation::FadeOut { id, .. }
            | Animation::ShowCreation { id, .. }
            | Animation::Grow { id, .. }
            | Animation::Rotate { id, .. } => id,
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
            out.interpolate(snapshot, target, a);
            Some(out)
        }
        Animation::ShowCreation { .. } => {
            let partial = target.partial(0.0, a);
            let mut p = partial;
            for (i, pt) in p.points.iter_mut().enumerate() {
                if i < target.points.len() {
                    pt.fill_rgba = target.points[i].fill_rgba;
                    if a < 1.0 {
                        pt.fill_rgba[3] = 0.0;
                    }
                    pt.stroke_rgba = target.points[i].stroke_rgba;
                    pt.stroke_width = target.points[i].stroke_width;
                    if (pt.stroke_width <= 0.0 || pt.stroke_rgba[3] <= 0.0) && target.points[i].fill_rgba[3] > 0.0 {
                        pt.stroke_rgba = target.points[i].fill_rgba;
                        pt.stroke_width = 2.0;
                    }
                }
            }
            Some(p)
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
