//! Scene + Player: a timeline of statements, and evaluation to per-frame state.

use crate::anim::{self, Animation, TimedAnim};
use crate::vgroup::VGroup;
use std::collections::HashMap;

/// A timeline statement — what the DSL compiles to. Serializable for .moab.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Stmt {
    /// Declare (or redeclare) a named mobject.
    Decl { id: String, obj: VGroup },
    Play(TimedAnim),
    Wait(f32),
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Timeline {
    pub stmts: Vec<Stmt>,
}

pub struct Spanned {
    pub stmt: Stmt,
    /// Byte offset in the .moa source, for IDE markers.
    pub src: usize,
}

impl Timeline {
    pub fn duration(&self) -> f32 {
        let mut d = 0.0;
        for s in &self.stmts {
            d += match s {
                Stmt::Play(a) => a.run_time,
                Stmt::Wait(t) => *t,
                Stmt::Decl { .. } => 0.0,
            };
        }
        d
    }

    /// Return all declared named mobjects and their initial geometries.
    pub fn initial_objects(&self) -> HashMap<String, VGroup> {
        let mut m = HashMap::new();
        for s in &self.stmts {
            if let Stmt::Decl { id, obj } = s {
                m.insert(id.clone(), obj.clone());
            }
        }
        m
    }

    /// Return timed animations with their absolute start time and duration.
    pub fn timed_anims(&self) -> Vec<(f32, &TimedAnim)> {
        let mut list = Vec::new();
        let mut t = 0.0;
        for s in &self.stmts {
            match s {
                Stmt::Play(a) => {
                    list.push((t, a));
                    t += a.run_time;
                }
                Stmt::Wait(dur) => {
                    t += *dur;
                }
                Stmt::Decl { .. } => {}
            }
        }
        list
    }
}

/// Evaluates a Timeline at arbitrary time t, producing visible render state.
/// Immutable + stateless: any time can be sampled, which powers scrubbing.
pub struct Player;

impl Player {
    /// State of all visible mobjects at time `t`.
    pub fn state_at(tl: &Timeline, t: f32) -> HashMap<String, VGroup> {
        let mut intro_times: HashMap<String, f32> = HashMap::new();
        let mut played_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut transform_targets: Vec<VGroup> = Vec::new();

        let mut time_cur = 0.0;
        for s in &tl.stmts {
            match s {
                Stmt::Play(ta) => {
                    let id = ta.anim.target_id().to_string();
                    played_ids.insert(id.clone());
                    if matches!(
                        ta.anim,
                        Animation::ShowCreation { .. }
                            | Animation::FadeIn { .. }
                            | Animation::Grow { .. }
                    ) {
                        intro_times.entry(id).or_insert(time_cur);
                    }
                    if let Animation::Transform { target, .. } = &ta.anim {
                        transform_targets.push(target.clone());
                    }
                    time_cur += ta.run_time;
                }
                Stmt::Wait(dur) => {
                    time_cur += dur;
                }
                Stmt::Decl { .. } => {}
            }
        }

        let mut state: HashMap<String, VGroup> = HashMap::new();
        for stmt in &tl.stmts {
            if let Stmt::Decl { id, obj } = stmt {
                // Pure transform targets (never played directly, match a transform target)
                // are shape definitions only, not initial scene objects.
                let is_pure_target = !played_ids.contains(id) && transform_targets.iter().any(|tg| tg == obj);
                if is_pure_target {
                    continue;
                }

                if let Some(&intro_t) = intro_times.get(id) {
                    if t >= intro_t {
                        state.insert(id.clone(), obj.clone());
                    }
                } else {
                    state.insert(id.clone(), obj.clone());
                }
            }
        }

        let mut t_cursor = 0.0;
        for stmt in &tl.stmts {
            match stmt {
                Stmt::Decl { .. } => {}
                Stmt::Wait(dur) => {
                    t_cursor += dur;
                    if t_cursor > t {
                        break;
                    }
                }
                Stmt::Play(ta) => {
                    let start = t_cursor;
                    let end = start + ta.run_time;
                    let id = ta.anim.target_id().to_string();
                    if t < start {
                        break;
                    }
                    match &ta.anim {
                        Animation::Transform { .. } => {
                            let target = if let Some(Animation::Transform { target, .. }) =
                                Some(&ta.anim)
                            {
                                target.clone()
                            } else {
                                unreachable!()
                            };
                            if t >= end {
                                state.insert(id, target);
                            } else {
                                let snapshot = state.get(&id).cloned().unwrap_or_default();
                                let mut scratch = snapshot.clone();
                                if let Some(out) = anim::sample(ta, &snapshot, &target, t - start) {
                                    scratch = out;
                                }
                                state.insert(id, scratch);
                            }
                            t_cursor = end;
                            if t < end {
                                break;
                            }
                        }
                        Animation::FadeIn { .. } => {
                            let target = state.get(&id).cloned().unwrap_or_default();
                            if t >= end {
                                state.insert(id, target);
                            } else {
                                let snapshot = VGroup::new();
                                let mut scratch = target.clone();
                                if let Some(out) = anim::sample(ta, &snapshot, &target, t - start) {
                                    scratch = out;
                                }
                                state.insert(id, scratch);
                            }
                            t_cursor = end;
                            if t < end {
                                break;
                            }
                        }
                        Animation::FadeOut { .. } | Animation::ShrinkToCenter { .. } => {
                            if t >= end {
                                state.remove(&id);
                            } else {
                                let target = state.get(&id).cloned().unwrap_or_default();
                                let mut scratch = target.clone();
                                if let Some(out) = anim::sample(ta, &target, &target, t - start) {
                                    scratch = out;
                                }
                                state.insert(id, scratch);
                            }
                            t_cursor = end;
                            if t < end {
                                break;
                            }
                        }
                        Animation::ShowCreation { .. }
                        | Animation::Grow { .. }
                        | Animation::Rotate { .. }
                        | Animation::SpinInFromNothing { .. }
                        | Animation::Indicate { .. }
                        | Animation::Wiggle { .. } => {
                            let target = state.get(&id).cloned().unwrap_or_default();
                            if t >= end {
                                if matches!(ta.anim, Animation::Rotate { .. }) {
                                    // rotation ends where it started visually? no — keep rotated
                                    let mut m = target.clone();
                                    if let Animation::Rotate { angle, .. } = &ta.anim {
                                        let c = m.center();
                                        m.rotate_z(*angle, c);
                                    }
                                    state.insert(id, m);
                                } else {
                                    state.insert(id, target);
                                }
                            } else {
                                let snapshot = state.get(&id).cloned().unwrap_or_default();
                                let mut scratch = snapshot.clone();
                                if let Some(out) = anim::sample(ta, &snapshot, &target, t - start) {
                                    scratch = out;
                                }
                                state.insert(id, scratch);
                            }
                            t_cursor = end;
                            if t < end {
                                break;
                            }
                        }
                        Animation::CameraOrbit { .. }
                        | Animation::CameraZoom { .. }
                        | Animation::CameraMoveTo { .. } => {
                            // No mobject effect; camera handled by camera_at.
                            t_cursor = end;
                            if t < end {
                                break;
                            }
                        }
                    }
                }
            }
        }
        state
    }

    /// Camera state at time `t`, animated by CameraOrbit/CameraZoom/CameraMoveTo
    /// statements. `base` carries the interactive/user camera (pan, zoom, size);
    /// animated values override it where the timeline says so.
    pub fn camera_at(tl: &Timeline, t: f32, base: &crate::camera::Camera) -> crate::camera::Camera {
        let mut cam = base.clone();
        let mut t_cursor = 0.0;
        for stmt in &tl.stmts {
            let Stmt::Play(ta) = stmt else {
                if let Stmt::Wait(dur) = stmt {
                    t_cursor += dur;
                    if t_cursor > t {
                        break;
                    }
                }
                continue;
            };
            let start = t_cursor;
            let end = start + ta.run_time;
            let alpha = ((t - start) / ta.run_time.max(1e-6)).clamp(0.0, 1.0);
            let a = (crate::rate::by_name(&ta.rate).unwrap_or(crate::rate::smooth))(alpha);
            let done = t >= end;
            match &ta.anim {
                Animation::CameraOrbit { phi, theta } => {
                    // Interpolate from the value in effect at anim start.
                    let (p0, th0) = if done { (*phi, *theta) } else { (cam.phi, cam.theta) };
                    let interp = |target: f32, from: f32| from + (target - from) * a;
                    cam.phi = interp(*phi, p0).clamp(
                        -std::f32::consts::FRAC_PI_2 + 0.01,
                        std::f32::consts::FRAC_PI_2 - 0.01,
                    );
                    cam.theta = interp(*theta, th0);
                }
                Animation::CameraZoom { factor } => {
                    if done {
                        cam.frame_height = (base.frame_height / factor).clamp(0.1, 100.0);
                    } else {
                        // Zoom blends the frame height toward its target.
                        let target = base.frame_height / factor;
                        cam.frame_height = (cam.frame_height + (target - cam.frame_height) * a)
                            .clamp(0.1, 100.0);
                    }
                }
                Animation::CameraMoveTo { x, y } => {
                    let (x0, y0) = if done { (*x, *y) } else { (cam.center.x, cam.center.y) };
                    cam.center.x = x0 + (*x - x0) * a;
                    cam.center.y = y0 + (*y - y0) * a;
                }
                _ => {}
            }
            t_cursor = end;
            if t < end {
                break;
            }
        }
        cam
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color;
    use crate::geom;
    use glam::Vec3;

    fn tl() -> Timeline {
        let mut c1 = geom::circle(1.0, Vec3::ZERO, [0.0; 4], color::BLUE, 4.0);
        c1.set_fill([0.0, 0.0, 0.0, 0.0]);
        let mut c2 = c1.clone();
        c2.shift(Vec3::new(2.0, 0.0, 0.0));
        Timeline {
            stmts: vec![
                Stmt::Decl { id: "c".into(), obj: c1 },
                Stmt::Play(crate::anim::TimedAnim::new(Animation::Transform { id: "c".into(), target: c2 }, 2.0, "linear")),
                Stmt::Wait(1.0),
            ],
        }
    }

    #[test]
    fn scrubbing_is_stateless() {
        let t = tl();
        assert_eq!(t.duration(), 3.0);
        let a = Player::state_at(&t, 0.0);
        assert!(a["c"].center().x.abs() < 0.01);
        let b = Player::state_at(&t, 1.0);
        assert!((b["c"].center().x - 1.0).abs() < 0.01);
        let c = Player::state_at(&t, 5.0);
        assert!((c["c"].center().x - 2.0).abs() < 0.01);
    }
}
