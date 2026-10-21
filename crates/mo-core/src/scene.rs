//! Scene + Player: a timeline of statements, and evaluation to per-frame state.

use crate::anim::{self, Animation, TimedAnim};
use crate::vgroup::VGroup;
use glam::Vec3;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// A timeline statement — what the DSL compiles to. Serializable for .moab.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Stmt {
    /// Declare (or redeclare) a named mobject. Shared via Arc so per-frame
    /// evaluation clones refcounts, not point data.
    Decl { id: String, obj: Arc<VGroup> },
    Play(TimedAnim),
    Wait(f32),
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Timeline {
    pub stmts: Vec<Stmt>,
    /// Memo for `duration` (O(stmts) sum) — recomputed only once per timeline.
    #[serde(skip)]
    duration_cache: std::sync::OnceLock<f32>,
}

impl Clone for Timeline {
    fn clone(&self) -> Self {
        Self { stmts: self.stmts.clone(), duration_cache: std::sync::OnceLock::new() }
    }
}

/// t-invariant data extracted from a Timeline once and reused across frames.
/// Build with `Timeline::index`, pass to `Player::state_at_indexed`.
#[derive(Default)]
pub struct TimelineIndex {
    /// Pre-resolved Decl preamble: ungated objects (no intro anim), visible
    /// from t = 0. Cloned as every sample's starting state — Arc refcount
    /// bumps instead of per-frame re-inserts and point-data equality scans.
    decl_base: HashMap<String, Arc<VGroup>>,
    /// Decls gated behind an intro anim: inserted once t >= their intro time.
    intro_decls: Vec<(String, Arc<VGroup>, f32)>,
    /// Final geometry of completed Rotate/Scale/Shift anims, keyed by stmt
    /// index — deterministic per stmt, so computed once and Arc-shared.
    baked_final: std::cell::RefCell<HashMap<usize, Arc<VGroup>>>,
}

/// LRU memo of sampled scene states, keyed by (timeline epoch, time bits).
/// Values are Arc-shared maps, so each entry costs only map/key overhead —
/// the point data is shared with the Timeline's Decl/transform objects. Size
/// it to hold every frame of a long timeline; playback and scrub revisits
/// then become pure lookups.
pub struct FrameMemo {
    map: HashMap<(u64, u32), (HashMap<String, Arc<VGroup>>, u64)>,
    cap: usize,
    tick: u64,
}

impl FrameMemo {
    pub fn new(cap: usize) -> Self {
        Self { map: HashMap::new(), cap, tick: 0 }
    }

    /// Sampled state for `key`, evaluating via `f` on a miss.
    pub fn get_or_insert_with(
        &mut self,
        key: (u64, u32),
        f: impl FnOnce() -> HashMap<String, Arc<VGroup>>,
    ) -> &HashMap<String, Arc<VGroup>> {
        self.tick += 1;
        let tick = self.tick;
        if !self.map.contains_key(&key) {
            self.map.insert(key, (f(), tick));
            if self.map.len() > self.cap {
                self.evict_oldest();
            }
        } else {
            self.map.get_mut(&key).unwrap().1 = tick;
        }
        &self.map.get(&key).unwrap().0
    }

    /// Drop ~1/8 of the entries (least recently touched first) in one pass —
    /// amortized, so full-memo frames don't pay an O(n) scan per insert.
    fn evict_oldest(&mut self) {
        let batch = (self.cap / 8).max(1);
        let mut by_age: Vec<(u64, (u64, u32))> =
            self.map.iter().map(|(k, (_, t))| (*t, *k)).collect();
        by_age.sort_unstable();
        for (_, k) in by_age.into_iter().take(batch) {
            self.map.remove(&k);
        }
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }
}

pub struct Spanned {
    pub stmt: Stmt,
    /// Byte offset in the .moa source, for IDE markers.
    pub src: usize,
}

impl Timeline {
    pub fn duration(&self) -> f32 {
        *self.duration_cache.get_or_init(|| {
            self.stmts
                .iter()
                .map(|s| match s {
                    Stmt::Play(a) => a.run_time,
                    Stmt::Wait(t) => *t,
                    Stmt::Decl { .. } => 0.0,
                })
                .sum()
        })
    }

    /// Return all declared named mobjects and their initial geometries.
    pub fn initial_objects(&self) -> HashMap<String, Arc<VGroup>> {
        let mut m = HashMap::new();
        for s in &self.stmts {
            if let Stmt::Decl { id, obj } = s {
                m.insert(id.clone(), obj.clone());
            }
        }
        m
    }

    /// Precompute the t-invariant evaluation preamble. Cheap relative to
    /// point data; do this once per timeline, not once per frame.
    pub fn index(&self) -> TimelineIndex {
        let mut idx = TimelineIndex::default();
        let mut played_ids: HashSet<String> = HashSet::new();
        let mut intro_times: HashMap<String, f32> = HashMap::new();
        let mut transform_targets: Vec<Arc<VGroup>> = Vec::new();
        let mut time_cur = 0.0;
        for s in &self.stmts {
            match s {
                Stmt::Play(ta) => {
                    let id = ta.anim.target_id().to_string();
                    played_ids.insert(id.clone());
                    if matches!(
                        ta.anim,
                        Animation::ShowCreation { .. }
                            | Animation::FadeIn { .. }
                            | Animation::Grow { .. }
                            | Animation::SpinInFromNothing { .. }
                            | Animation::DrawBorderThenFill { .. }
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

        // Resolve the Decl preamble once: per-id last declaration wins, and
        // the pure-target test (a point-data equality scan) moves off the
        // per-frame path entirely.
        enum Gate {
            Always,
            After(f32),
        }
        let mut resolved: HashMap<&str, (Arc<VGroup>, Gate)> = HashMap::new();
        for s in &self.stmts {
            if let Stmt::Decl { id, obj } = s {
                let gate = match intro_times.get(id) {
                    Some(&it) => Gate::After(it),
                    None => Gate::Always,
                };
                resolved.insert(id.as_str(), (obj.clone(), gate));
            }
        }
        for (id, (obj, gate)) in resolved {
            let is_pure_target = !played_ids.contains(id)
                && transform_targets.iter().any(|tg| tg == &obj);
            if is_pure_target {
                continue;
            }
            match gate {
                Gate::Always => {
                    idx.decl_base.insert(id.to_string(), obj);
                }
                Gate::After(t) => {
                    idx.intro_decls.push((id.to_string(), obj, t));
                }
            }
        }
        idx
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
    /// State of all visible mobjects at time `t`. Builds the timeline index
    /// on the fly; hot loops should use `state_at_indexed` with a reused index.
    pub fn state_at(tl: &Timeline, t: f32) -> HashMap<String, Arc<VGroup>> {
        let idx = tl.index();
        Player::state_at_indexed(tl, &idx, t)
    }

    /// State of all visible mobjects at time `t`, reusing a precomputed index.
    pub fn state_at_indexed(
        tl: &Timeline,
        idx: &TimelineIndex,
        t: f32,
    ) -> HashMap<String, Arc<VGroup>> {
        // Decl preamble resolved at index time: clone bumps refcounts, then
        // only intro-gated objects are inserted conditionally.
        let mut state: HashMap<String, Arc<VGroup>> = idx.decl_base.clone();
        for (id, obj, intro_t) in &idx.intro_decls {
            if t >= *intro_t {
                state.insert(id.clone(), obj.clone());
            }
        }

        let mut t_cursor = 0.0;
        let empty = VGroup::new();
        for (stmt_i, stmt) in tl.stmts.iter().enumerate() {
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
                        Animation::Transform { target, .. } => {
                            if t >= end {
                                state.insert(id, target.clone());
                            } else if let Some(out) = anim::sample(
                                ta,
                                state.get(&id).map(Arc::as_ref).unwrap_or(&empty),
                                target,
                                t - start,
                            ) {
                                state.insert(id, Arc::new(out));
                            }
                            t_cursor = end;
                            if t < end {
                                break;
                            }
                        }
                        Animation::FadeIn { .. } => {
                            if t < end {
                                let target =
                                    state.get(&id).map(Arc::as_ref).unwrap_or(&empty);
                                if let Some(out) = anim::sample(ta, &empty, target, t - start) {
                                    state.insert(id, Arc::new(out));
                                }
                            }
                            t_cursor = end;
                            if t < end {
                                break;
                            }
                        }
                        Animation::FadeOut { .. } | Animation::ShrinkToCenter { .. } | Animation::Uncreate { .. } => {
                            if t >= end {
                                state.remove(&id);
                            } else if let Some(target) = state.get(&id).cloned() {
                                if let Some(out) = anim::sample(ta, &target, &target, t - start) {
                                    state.insert(id, Arc::new(out));
                                }
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
                        | Animation::Wiggle { .. }
                        | Animation::DrawBorderThenFill { .. }
                        | Animation::Circumscribe { .. }
                        | Animation::Flash { .. }
                        | Animation::Scale { .. }
                        | Animation::Shift { .. } => {
                            if t >= end {
                                // Final geometry is deterministic per stmt — bake
                                // once and Arc-share on every later frame.
                                if matches!(
                                    &ta.anim,
                                    Animation::Rotate { .. } | Animation::Scale { .. } | Animation::Shift { .. }
                                ) {
                                    let mut baked = idx.baked_final.borrow_mut();
                                    let m = baked
                                        .entry(stmt_i)
                                        .or_insert_with(|| {
                                            let mut m = state
                                                .get(&id)
                                                .map(|g| g.as_ref().clone())
                                                .unwrap_or_else(VGroup::new);
                                            match &ta.anim {
                                                Animation::Rotate { angle, .. } => {
                                                    let c = m.center();
                                                    m.rotate_z(*angle, c);
                                                }
                                                Animation::Scale { factor, .. } => {
                                                    let c = m.center();
                                                    for p in &mut m.points {
                                                        let q = Vec3::from(p.point);
                                                        p.point = (c + (q - c) * factor).to_array();
                                                    }
                                                }
                                                Animation::Shift { delta, .. } => {
                                                    m.shift(*delta);
                                                }
                                                _ => unreachable!(),
                                            }
                                            Arc::new(m)
                                        })
                                        .clone();
                                    drop(baked);
                                    state.insert(id, m);
                                }
                                // Other intro anims: the Decl pass already put the
                                // final object in `state` — nothing to do.
                            } else if let Some(out) = anim::sample(
                                ta,
                                state.get(&id).map(Arc::as_ref).unwrap_or(&empty),
                                state.get(&id).map(Arc::as_ref).unwrap_or(&empty),
                                t - start,
                            ) {
                                state.insert(id, Arc::new(out));
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
                Stmt::Decl { id: "c".into(), obj: std::sync::Arc::new(c1) },
                Stmt::Play(crate::anim::TimedAnim::new(Animation::Transform { id: "c".into(), target: std::sync::Arc::new(c2) }, 2.0, "linear")),
                Stmt::Wait(1.0),
            ],
            duration_cache: std::sync::OnceLock::new(),
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
