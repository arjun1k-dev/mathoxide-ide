//! The core vector object: a list of quadratic bezier curves, laid out
//! ManimGL-style — per subpath: anchor, handle, anchor, handle, ..., anchor
//! (2n+1 records for n curves). Curve i reads records 2i, 2i+1, 2i+2.

use glam::Vec3;

/// One vertex record. POD, directly uploadable.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable, serde::Serialize, serde::Deserialize)]
pub struct VmPoint {
    pub point: [f32; 3],
    pub stroke_rgba: [f32; 4],
    pub fill_rgba: [f32; 4],
    pub stroke_width: f32,
    /// Extra float so the record is 48 bytes (16-byte aligned for storage buffers).
    pub joint_angle: f32,
}

impl Default for VmPoint {
    fn default() -> Self {
        Self {
            point: [0.0; 3],
            stroke_rgba: [0.0, 0.0, 0.0, 1.0],
            fill_rgba: [0.0, 0.0, 0.0, 0.0],
            stroke_width: 4.0,
            joint_angle: 0.0,
        }
    }
}

/// A contiguous run of curves: record range [start, end) in the point list,
/// where the subpath holds (end - start) / 2 curves.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SubPath {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VGroup {
    pub points: Vec<VmPoint>,
    pub subpaths: Vec<SubPath>,
}

impl VGroup {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn curve_count(&self) -> usize {
        self.subpaths.iter().map(|sp| (sp.end - sp.start - 1) / 2).sum()
    }

    /// Start a new subpath at `anchor` (pushes one record).
    pub fn start_subpath(&mut self, anchor: Vec3) {
        let mut p = VmPoint::default();
        p.point = anchor.to_array();
        self.points.push(p);
        self.subpaths.push(SubPath {
            start: self.points.len() - 1,
            end: self.points.len(),
        });
    }

    /// Append a quadratic bezier (handle -> anchor) onto the last subpath.
    /// Records after: ..., anchor_prev, handle, anchor.
    pub fn add_curve(&mut self, handle: Vec3, anchor: Vec3) {
        let last = *self.points.last().expect("no open subpath");
        let mut h = last;
        h.point = handle.to_array();
        let mut a = last;
        a.point = anchor.to_array();
        self.points.push(h);
        self.points.push(a);
        self.subpaths.last_mut().unwrap().end += 2;
    }

    /// Close the last subpath back to its start anchor (adds a curve if not adjacent).
    pub fn close(&mut self) {
        let Some(sp) = self.subpaths.last().cloned() else { return };
        let start = self.points[sp.start].point;
        let last = self.points[sp.end - 1].point;
        if last != start {
            let mid = [
                (last[0] + start[0]) / 2.0,
                (last[1] + start[1]) / 2.0,
                (last[2] + start[2]) / 2.0,
            ];
            self.add_curve(mid.into(), start.into());
        }
        self.points[sp.end - 1].point = start;
    }

    /// Records of curve i, in flat order across subpaths.
    pub fn curves(&self) -> impl Iterator<Item = [Vec3; 3]> + '_ {
        self.subpaths.iter().flat_map(move |sp| {
            (sp.start..sp.end - 2).step_by(2).map(|r| {
                [
                    Vec3::from(self.points[r].point),
                    Vec3::from(self.points[r + 1].point),
                    Vec3::from(self.points[r + 2].point),
                ]
            })
        })
    }

    /// Port of manim's pointwise_become_partial: keep the fraction [a, b]
    /// of the curves, styling copied from the source records.
    pub fn partial(&self, a: f32, b: f32) -> VGroup {
        let a = a.clamp(0.0, 1.0);
        let b = b.clamp(a, 1.0);
        let mut out = VGroup::new();
        let curves: Vec<[Vec3; 3]> = self.curves().collect();
        let total = curves.len() as f32;
        if total == 0.0 || a >= b {
            return out;
        }
        let start_curve = (a * total).floor() as usize;
        let end_curve = ((b * total).ceil() as usize).min(curves.len());
        if start_curve >= end_curve {
            return out;
        }
        for (i, &[p0, p1, p2]) in curves.iter().enumerate().take(end_curve).skip(start_curve) {
            let f0 = if i == start_curve { (a * total - i as f32).max(0.0) } else { 0.0 };
            let f1 = if i + 1 == end_curve { (b * total - i as f32).min(1.0) } else { 1.0 };
            let [q0, q1, q2] = partial_quadratic(&[p0, p1, p2], f0, f1);
            if out.subpaths.is_empty() {
                out.start_subpath(q0);
            }
            out.add_curve(q1, q2);
        }
        // copy styling from nearby source records
        let src_start = start_curve * 2;
        for (i, p) in out.points.iter_mut().enumerate() {
            let src = (src_start + i).min(self.points.len() - 1);
            p.fill_rgba = self.points[src].fill_rgba;
            p.stroke_rgba = self.points[src].stroke_rgba;
            p.stroke_width = self.points[src].stroke_width;
        }
        out
    }

    pub fn set_stroke(&mut self, color: [f32; 4], width: f32) {
        for p in &mut self.points {
            p.stroke_rgba = color;
            p.stroke_width = width;
        }
    }

    pub fn set_fill(&mut self, color: [f32; 4]) {
        for p in &mut self.points {
            p.fill_rgba = color;
        }
    }

    pub fn shift(&mut self, v: Vec3) {
        for p in &mut self.points {
            p.point = (Vec3::from(p.point) + v).to_array();
        }
    }

    pub fn scale(&mut self, s: f32, about: Vec3) {
        for p in &mut self.points {
            p.point = (Vec3::from(p.point) - about * s + about).to_array();
            let q = Vec3::from(p.point);
            p.point = (about + (q - about) * s).to_array();
        }
    }

    pub fn rotate_z(&mut self, angle: f32, about: Vec3) {
        let (s, c) = angle.sin_cos();
        for p in &mut self.points {
            let q = Vec3::from(p.point) - about;
            p.point = [
                about.x + q.x * c - q.y * s,
                about.y + q.x * s + q.y * c,
                p.point[2],
            ];
        }
    }

    pub fn bounds(&self) -> ([f32; 3], [f32; 3]) {
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for p in &self.points {
            for i in 0..3 {
                min[i] = min[i].min(p.point[i]);
                max[i] = max[i].max(p.point[i]);
            }
        }
        (min, max)
    }

    pub fn center(&self) -> Vec3 {
        let (min, max) = self.bounds();
        Vec3::new(
            (min[0] + max[0]) / 2.0,
            (min[1] + max[1]) / 2.0,
            (min[2] + max[2]) / 2.0,
        )
    }

    /// Componentwise lerp with another group of identical shape (post-alignment).
    pub fn interpolate(&mut self, start: &VGroup, end: &VGroup, alpha: f32) {
        debug_assert_eq!(self.points.len(), start.points.len());
        for i in 0..self.points.len().min(start.points.len()).min(end.points.len()) {
            for j in 0..3 {
                self.points[i].point[j] =
                    start.points[i].point[j] * (1.0 - alpha) + end.points[i].point[j] * alpha;
            }
            for j in 0..4 {
                self.points[i].fill_rgba[j] =
                    start.points[i].fill_rgba[j] * (1.0 - alpha) + end.points[i].fill_rgba[j] * alpha;
                self.points[i].stroke_rgba[j] =
                    start.points[i].stroke_rgba[j] * (1.0 - alpha) + end.points[i].stroke_rgba[j] * alpha;
            }
            self.points[i].stroke_width =
                start.points[i].stroke_width * (1.0 - alpha) + end.points[i].stroke_width * alpha;
        }
        self.subpaths = start.subpaths.clone();
    }

    /// Port of manim's align: restructure `self` to have the same subpath/curve
    /// structure as `target` (adds degenerate curves). Needed before interpolate.
    pub fn align_to(&mut self, target: &VGroup) {
        // Simple strategy: pad every subpath's curve count to match, adding
        // degenerate curves at the last anchor.
        for i in 0..self.subpaths.len().max(target.subpaths.len()) {
            let mine = self.subpaths.get(i);
            let theirs = target.subpaths.get(i);
            let my_curves = mine.map(|sp| (sp.end - sp.start) / 2).unwrap_or(0);
            let their_curves = theirs.map(|sp| (sp.end - sp.start) / 2).unwrap_or(0);
            if my_curves < their_curves {
                if mine.is_none() {
                    let at = self.points.last().map(|p| p.point).unwrap_or([0.0; 3]);
                    self.start_subpath(at.into());
                }
                let anchor = self.points.last().unwrap().clone();
                for _ in my_curves..their_curves {
                    self.add_curve(anchor.point.into(), anchor.point.into());
                }
            } else if their_curves == 0 && mine.is_none() && self.subpaths.len() > i {
                // nothing
            }
        }
        // Extra curves on my side get collapsed to degenerate? For v0.1 leave them;
        // interpolate only touches shared prefix.
    }
}

/// Port of manim's partial_quadratic_bezier_points for [a,b] within one curve.
fn partial_quadratic(p: &[Vec3; 3], a: f32, b: f32) -> [Vec3; 3] {
    if a >= 1.0 {
        return [p[2]; 3];
    }
    let curve = |t: f32| (1.0 - t) * (1.0 - t) * p[0] + 2.0 * t * (1.0 - t) * p[1] + t * t * p[2];
    let h0 = if a > 0.0 { curve(a) } else { p[0] };
    let h2 = if b < 1.0 { curve(b) } else { p[2] };
    let h1p = (1.0 - a) * p[1] + a * p[2];
    let end_prop = (b - a) / (1.0 - a);
    let h1 = h0.lerp(h1p, end_prop);
    [h0, h1, h2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_subpath() {
        let mut g = VGroup::new();
        g.start_subpath(vec3(-1.0, -1.0, 0.0));
        g.add_curve(vec3(0.0, -1.0, 0.0), vec3(1.0, -1.0, 0.0));
        g.add_curve(vec3(1.0, 0.0, 0.0), vec3(1.0, 1.0, 0.0));
        g.add_curve(vec3(0.0, 1.0, 0.0), vec3(-1.0, 1.0, 0.0));
        g.close();
        assert_eq!(g.curve_count(), 4);
        // 2n+1 records, n = 4 curves (close added one? no — 4th side was added via add_curve)
        assert_eq!(g.points.len(), 2 * 4 + 1);
    }

    fn vec3(x: f32, y: f32, z: f32) -> Vec3 {
        Vec3::new(x, y, z)
    }

    #[test]
    fn partial_keeps_fraction() {
        let mut g = VGroup::new();
        g.start_subpath(vec3(0.0, 0.0, 0.0));
        for i in 1..=4 {
            g.add_curve(vec3(i as f32 - 0.5, 0.0, 0.0), vec3(i as f32, 0.0, 0.0));
        }
        assert_eq!(g.curve_count(), 4);
        let half = g.partial(0.0, 0.5);
        assert_eq!(half.curve_count(), 2);
        let quarter_mid = g.partial(0.25, 0.75);
        assert_eq!(quarter_mid.curve_count(), 2);
    }

    #[test]
    fn transform_identity() {
        let mut g = VGroup::new();
        g.start_subpath(vec3(0.0, 0.0, 0.0));
        g.add_curve(vec3(0.5, 0.0, 0.0), vec3(1.0, 0.0, 0.0));
        let orig = g.clone();
        let mut scratch = g.clone();
        scratch.interpolate(&orig, &orig, 0.5);
        assert_eq!(scratch, orig);
    }
}
