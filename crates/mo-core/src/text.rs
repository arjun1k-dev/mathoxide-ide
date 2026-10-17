//! Vector text: converts TrueType glyph outlines into quadratic Bézier VGroups.
//! True vector math animations for text, just like ManimGL.

use crate::vgroup::VGroup;
use glam::Vec3;

struct VGroupOutlineBuilder {
    group: VGroup,
    scale: f32,
    offset: Vec3,
    current_pt: Vec3,
    start_pt: Vec3,
}

impl ttf_parser::OutlineBuilder for VGroupOutlineBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.offset + Vec3::new(x * self.scale, y * self.scale, 0.0);
        self.current_pt = p;
        self.start_pt = p;
        self.group.start_subpath(p);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.offset + Vec3::new(x * self.scale, y * self.scale, 0.0);
        let handle = (self.current_pt + p) * 0.5;
        self.group.add_curve(handle, p);
        self.current_pt = p;
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let handle = self.offset + Vec3::new(x1 * self.scale, y1 * self.scale, 0.0);
        let p = self.offset + Vec3::new(x * self.scale, y * self.scale, 0.0);
        self.group.add_curve(handle, p);
        self.current_pt = p;
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        // Approximate cubic curve by splitting into two quadratic segments
        let p0 = self.current_pt;
        let p1 = self.offset + Vec3::new(x1 * self.scale, y1 * self.scale, 0.0);
        let p2 = self.offset + Vec3::new(x2 * self.scale, y2 * self.scale, 0.0);
        let p3 = self.offset + Vec3::new(x * self.scale, y * self.scale, 0.0);

        let q1 = p0 + (p1 - p0) * 0.75;
        let q2 = p3 + (p2 - p3) * 0.75;
        let mid = (q1 + q2) * 0.5;
        self.group.add_curve(q1, mid);
        self.group.add_curve(q2, p3);
        self.current_pt = p3;
    }

    fn close(&mut self) {
        if self.current_pt.distance_squared(self.start_pt) > 1e-6 {
            let handle = (self.current_pt + self.start_pt) * 0.5;
            self.group.add_curve(handle, self.start_pt);
            self.current_pt = self.start_pt;
        }
    }
}

/// Create a vector text VGroup from string `s` and `font_size` in scene units.
/// Uses embedded TrueType font outlines for crisp, scalable vector glyphs.
pub fn text(s: &str, font_size: f32) -> VGroup {
    text_styled(s, font_size, [1.0, 1.0, 1.0, 1.0], [1.0, 1.0, 1.0, 0.0], 0.0)
}

/// Create vector text with custom fill, stroke, and stroke_width.
pub fn text_styled(
    s: &str,
    font_size: f32,
    fill: [f32; 4],
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup {
    let font_bytes = epaint_default_fonts::UBUNTU_LIGHT;
    let Ok(face) = ttf_parser::Face::parse(font_bytes, 0) else {
        return VGroup::new();
    };

    let units_per_em = face.units_per_em() as f32;
    let scale = font_size / units_per_em;
    let line_height = (face.ascender() - face.descender() + face.line_gap()) as f32 * scale;

    let mut builder = VGroupOutlineBuilder {
        group: VGroup::new(),
        scale,
        offset: Vec3::ZERO,
        current_pt: Vec3::ZERO,
        start_pt: Vec3::ZERO,
    };

    let mut cursor_x = 0.0_f32;
    let mut cursor_y = 0.0_f32;

    for ch in s.chars() {
        if ch == '\n' {
            cursor_x = 0.0;
            cursor_y -= line_height;
            continue;
        }

        if let Some(glyph_id) = face.glyph_index(ch) {
            builder.offset = Vec3::new(cursor_x, cursor_y, 0.0);
            face.outline_glyph(glyph_id, &mut builder);
            if let Some(advance) = face.glyph_hor_advance(glyph_id) {
                cursor_x += advance as f32 * scale;
            }
        } else if ch == ' ' {
            cursor_x += font_size * 0.3;
        }
    }

    let mut result = builder.group;
    result.set_fill(fill);
    result.set_stroke(stroke, stroke_width);

    if !result.points.is_empty() {
        // Center the text around origin
        let (min, max) = result.bounds();
        let center = Vec3::new(
            (min[0] + max[0]) * 0.5,
            (min[1] + max[1]) * 0.5,
            0.0,
        );
        result.shift(-center);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_text_generation() {
        let g = text("MathOxide", 1.0);
        assert!(!g.points.is_empty(), "Text should produce vector points");
        assert!(!g.subpaths.is_empty(), "Text should produce subpaths");
        let (min, max) = g.bounds();
        let w = max[0] - min[0];
        let h = max[1] - min[1];
        assert!(w > 1.0, "Width should be positive and reasonable");
        assert!(h > 0.3 && h < 2.0, "Height should be reasonable for 1.0 font size");
    }
}
