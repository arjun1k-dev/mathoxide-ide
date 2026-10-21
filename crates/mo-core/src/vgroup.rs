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
        self.subpaths
            .iter()
            .map(|sp| (sp.end.saturating_sub(sp.start).saturating_sub(1)) / 2)
            .sum()
    }

    /// Start a new subpath at `anchor` (pushes one record).
    pub fn start_subpath(&mut self, anchor: Vec3) {
        let mut p = VmPoint::default();
        p.point = anchor.to_array();
        self.points.push(p);
        self.subpaths.push(SubPath {
            start: self.points.len().saturating_sub(1),
            end: self.points.len(),
        });
    }

    /// Append another VGroup into self, correctly offsetting subpath indices.
    pub fn append(&mut self, other: VGroup) {
        let offset = self.points.len();
        self.points.extend(other.points);
        for sp in other.subpaths {
            self.subpaths.push(SubPath {
                start: sp.start + offset,
                end: sp.end + offset,
            });
        }
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
        if sp.end <= sp.start || self.points.is_empty() {
            return;
        }
        let start = self.points[sp.start].point;
        let last_idx = sp.end.saturating_sub(1);
        let last = self.points[last_idx].point;
        if last != start {
            let mid = [
                (last[0] + start[0]) / 2.0,
                (last[1] + start[1]) / 2.0,
                (last[2] + start[2]) / 2.0,
            ];
            self.add_curve(mid.into(), start.into());
        }
        if let Some(sp) = self.subpaths.last_mut() {
            if sp.end > 0 {
                self.points[sp.end - 1].point = start;
            }
        }
    }

    /// Records of curve i, in flat order across subpaths.
    pub fn curves(&self) -> impl Iterator<Item = [Vec3; 3]> + '_ {
        self.subpaths.iter().flat_map(move |sp| {
            let limit = sp.end.saturating_sub(2);
            (sp.start..limit).step_by(2).map(|r| {
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
        let total_curves = self.curve_count();
        let total = total_curves as f32;
        if total == 0.0 || a >= b {
            return out;
        }
        let start_curve = (a * total).floor() as usize;
        let end_curve = ((b * total).ceil() as usize).min(total_curves);
        if start_curve >= end_curve {
            return out;
        }
        for (i, [p0, p1, p2]) in
            self.curves().enumerate().skip(start_curve).take(end_curve - start_curve)
        {
            let f0 = if i == start_curve { (a * total - i as f32).max(0.0) } else { 0.0 };
            let f1 = if i + 1 == end_curve { (b * total - i as f32).min(1.0) } else { 1.0 };
            let [q0, q1, q2] = partial_quadratic(&[p0, p1, p2], f0, f1);
            let need_new_subpath = match out.points.last() {
                None => true,
                Some(last) => Vec3::from(last.point).distance_squared(q0) > 1e-4,
            };
            if need_new_subpath {
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

    /// Extract each subpath as an independent VGroup.
    pub fn extract_subpaths(&self) -> Vec<VGroup> {
        let sps = if self.subpaths.is_empty() {
            if self.points.is_empty() {
                vec![]
            } else {
                vec![SubPath {
                    start: 0,
                    end: self.points.len(),
                }]
            }
        } else {
            self.subpaths.clone()
        };

        sps.into_iter()
            .filter_map(|sp| {
                if sp.end > sp.start && sp.end <= self.points.len() {
                    let pts = self.points[sp.start..sp.end].to_vec();
                    let mut g = VGroup::new();
                    let p_len = pts.len();
                    g.points = pts;
                    g.subpaths = vec![SubPath { start: 0, end: p_len }];
                    Some(g)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Resamples or pads points within a single contour to reach `target_len`.
    pub fn pad_to_length(&mut self, target_len: usize) {
        if self.points.len() >= target_len || self.points.is_empty() {
            return;
        }

        let needed = target_len - self.points.len();
        let old_len = self.points.len();
        let mut new_points = Vec::with_capacity(target_len);

        for (i, p) in self.points.iter().enumerate() {
            new_points.push(*p);
            let dup_count = (needed * (i + 1)) / old_len - (needed * i) / old_len;
            for _ in 0..dup_count {
                new_points.push(*p);
            }
        }

        self.points = new_points;
        self.subpaths = vec![SubPath {
            start: 0,
            end: self.points.len(),
        }];
    }

    /// Aligns `self` and `target` so that both have an identical number of subpaths
    /// and points per subpath, enabling smooth Manim-grade interpolation between
    /// any two shapes, text strings, or LaTeX equations without spurious triangle connections.
    pub fn align_with(&self, target: &VGroup) -> (VGroup, VGroup) {
        let mut subs_a = self.extract_subpaths();
        let mut subs_b = target.extract_subpaths();

        if subs_a.is_empty() && subs_b.is_empty() {
            return (self.clone(), target.clone());
        }

        if subs_a.is_empty() {
            let center = target.center();
            let mut g = VGroup::new();
            g.start_subpath(center);
            subs_a.push(g);
        }

        if subs_b.is_empty() {
            let center = self.center();
            let mut g = VGroup::new();
            g.start_subpath(center);
            subs_b.push(g);
        }

        // Equalize the number of subpaths
        let max_subs = subs_a.len().max(subs_b.len());
        fn pad_sub_list(list: &mut Vec<VGroup>, target_count: usize) {
            if list.len() >= target_count || list.is_empty() {
                return;
            }
            let old_len = list.len();
            let needed = target_count - old_len;
            let mut new_list = Vec::with_capacity(target_count);
            for (i, item) in list.iter().enumerate() {
                new_list.push(item.clone());
                let dup_count = (needed * (i + 1)) / old_len - (needed * i) / old_len;
                for _ in 0..dup_count {
                    let mut collapsed = item.clone();
                    let c = item.center();
                    for pt in &mut collapsed.points {
                        pt.point = c.to_array();
                        pt.fill_rgba[3] = 0.0;
                        pt.stroke_rgba[3] = 0.0;
                        pt.stroke_width = 0.0;
                    }
                    new_list.push(collapsed);
                }
            }
            *list = new_list;
        }

        pad_sub_list(&mut subs_a, max_subs);
        pad_sub_list(&mut subs_b, max_subs);

        // Within each subpath pair, pad points to match
        let mut out_a = VGroup::new();
        let mut out_b = VGroup::new();

        for (mut sa, mut sb) in subs_a.into_iter().zip(subs_b.into_iter()) {
            let max_pts = sa.points.len().max(sb.points.len());
            sa.pad_to_length(max_pts);
            sb.pad_to_length(max_pts);

            let start_a = out_a.points.len();
            out_a.points.extend(sa.points);
            out_a.subpaths.push(SubPath {
                start: start_a,
                end: out_a.points.len(),
            });

            let start_b = out_b.points.len();
            out_b.points.extend(sb.points);
            out_b.subpaths.push(SubPath {
                start: start_b,
                end: out_b.points.len(),
            });
        }

        (out_a, out_b)
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

    #[test]
    fn scale_about_point_preserves_center() {
        let mut g = VGroup::new();
        g.start_subpath(vec3(1.0, 2.0, 0.0));
        g.add_curve(vec3(3.0, 2.0, 0.0), vec3(2.0, 4.0, 0.0));
        let c = g.center();
        g.scale(0.5, c);
        let c2 = g.center();
        assert!((c2.x - c.x).abs() < 1e-5, "center.x drifted: {c2} vs {c}");
        assert!((c2.y - c.y).abs() < 1e-5, "center.y drifted: {c2} vs {c}");
        // extents halve about the center
        let (mn, mx) = g.bounds();
        assert!(((mx[0] - mn[0]) - 1.0).abs() < 1e-5, "width should halve: {}", mx[0] - mn[0]);
    }
}
