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


pub fn get_font_bytes(font: Option<&str>) -> std::borrow::Cow<'static, [u8]> {
    match font.map(|s| s.to_lowercase()).as_deref() {
        Some("mono" | "hack" | "monospace" | "code") => {
            std::borrow::Cow::Borrowed(epaint_default_fonts::HACK_REGULAR)
        }
        Some("emoji") => {
            std::borrow::Cow::Borrowed(epaint_default_fonts::NOTO_EMOJI_REGULAR)
        }
        Some("serif") => {
            for path in [
                "/usr/share/fonts/noto/NotoSerif-Regular.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf",
                "/usr/share/fonts/TTF/DejaVuSerif.ttf",
            ] {
                if let Ok(bytes) = std::fs::read(path) {
                    return std::borrow::Cow::Owned(bytes);
                }
            }
            std::borrow::Cow::Borrowed(epaint_default_fonts::UBUNTU_LIGHT)
        }
        Some("noto" | "notosans" | "sans") => {
            for path in [
                "/usr/share/fonts/noto/NotoSans-Regular.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/TTF/DejaVuSans.ttf",
            ] {
                if let Ok(bytes) = std::fs::read(path) {
                    return std::borrow::Cow::Owned(bytes);
                }
            }
            std::borrow::Cow::Borrowed(epaint_default_fonts::UBUNTU_LIGHT)
        }
        Some(custom) if custom.ends_with(".ttf") || custom.ends_with(".otf") || custom.contains('/') => {
            if let Ok(bytes) = std::fs::read(custom) {
                std::borrow::Cow::Owned(bytes)
            } else {
                std::borrow::Cow::Borrowed(epaint_default_fonts::UBUNTU_LIGHT)
            }
        }
        _ => std::borrow::Cow::Borrowed(epaint_default_fonts::UBUNTU_LIGHT),
    }
}

/// Create a vector text VGroup from string `s` and `font_size` in scene units.
/// Uses embedded TrueType font outlines for crisp, scalable vector glyphs.
pub fn text(s: &str, font_size: f32) -> VGroup {
    text_styled(s, font_size, [1.0, 1.0, 1.0, 1.0], [1.0, 1.0, 1.0, 0.0], 0.0)
}

/// Create vector text with custom fill, stroke, and stroke_width using default font.
pub fn text_styled(
    s: &str,
    font_size: f32,
    fill: [f32; 4],
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup {
    text_styled_with_font(s, font_size, None, fill, stroke, stroke_width)
}

/// Create vector text with specific font selection (e.g. "ubuntu", "mono", "hack", "serif", "sans", or file path).
pub fn text_styled_with_font(
    s: &str,
    font_size: f32,
    font: Option<&str>,
    fill: [f32; 4],
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup {
    let font_bytes = get_font_bytes(font);
    let Ok(face) = ttf_parser::Face::parse(&font_bytes, 0) else {
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

/// Create a DecimalNumber mobject displaying a formatted floating-point number
pub fn decimal_number(
    val: f64,
    num_decimal_places: usize,
    font_size: f32,
    font: Option<&str>,
    fill: [f32; 4],
) -> VGroup {
    let s = format!("{:.1$}", val, num_decimal_places);
    text_styled_with_font(&s, font_size, font, fill, [0.0; 4], 0.0)
}

/// Create an Integer mobject displaying a formatted integer
pub fn integer_number(
    val: i64,
    font_size: f32,
    font: Option<&str>,
    fill: [f32; 4],
) -> VGroup {
    let s = format!("{}", val);
    text_styled_with_font(&s, font_size, font, fill, [0.0; 4], 0.0)
}

/// Create a Matrix mobject with surrounding brackets from a 2D array of string entries
pub fn matrix(
    entries: &[Vec<String>],
    font_size: f32,
    bracket_type: &str, // "[", "(", "{", "|"
    font: Option<&str>,
    fill: [f32; 4],
    stroke: [f32; 4],
    sw: f32,
) -> VGroup {
    let mut g = VGroup::new();
    let num_rows = entries.len();
    if num_rows == 0 {
        return g;
    }
    let num_cols = entries[0].len();
    if num_cols == 0 {
        return g;
    }

    // Render each cell and measure cell bounds
    let mut rendered: Vec<Vec<VGroup>> = Vec::new();
    let mut col_widths = vec![0.0_f32; num_cols];
    let mut row_heights = vec![0.0_f32; num_rows];

    for r in 0..num_rows {
        let mut row_group = Vec::new();
        for c in 0..num_cols {
            let s = entries.get(r).and_then(|row| row.get(c)).map(|s| s.as_str()).unwrap_or("");
            let item = text_styled_with_font(s, font_size, font, fill, [0.0; 4], 0.0);
            let (min, max) = item.bounds();
            let w = (max[0] - min[0]).max(font_size * 0.4);
            let h = (max[1] - min[1]).max(font_size * 0.6);
            if w > col_widths[c] {
                col_widths[c] = w;
            }
            if h > row_heights[r] {
                row_heights[r] = h;
            }
            row_group.push(item);
        }
        rendered.push(row_group);
    }

    let h_buff = font_size * 0.5;
    let v_buff = font_size * 0.4;

    // Total dimensions
    let total_width: f32 = col_widths.iter().sum::<f32>() + (num_cols.saturating_sub(1) as f32) * h_buff;
    let total_height: f32 = row_heights.iter().sum::<f32>() + (num_rows.saturating_sub(1) as f32) * v_buff;

    let start_x = -total_width * 0.5;
    let mut cur_y = total_height * 0.5 - row_heights[0] * 0.5;

    for r in 0..num_rows {
        let mut cur_x = start_x;
        for c in 0..num_cols {
            let center_x = cur_x + col_widths[c] * 0.5;
            let mut item = rendered[r][c].clone();
            item.shift(Vec3::new(center_x, cur_y, 0.0));
            g.append(item);
            cur_x += col_widths[c] + h_buff;
        }
        if r + 1 < num_rows {
            cur_y -= (row_heights[r] + row_heights[r + 1]) * 0.5 + v_buff;
        }
    }

    // Add brackets
    let (min, max) = g.bounds();
    let margin = font_size * 0.25;
    let bracket_sw = if sw <= 0.0 { 2.5 } else { sw };
    let left_x = min[0] - margin;
    let right_x = max[0] + margin;
    let top_y = max[1] + margin;
    let bot_y = min[1] - margin;
    let bracket_hook = font_size * 0.25;

    match bracket_type {
        "[" => {
            // Left bracket [
            let mut left_b = VGroup::new();
            left_b.start_subpath(Vec3::new(left_x + bracket_hook, top_y, 0.0));
            left_b.add_curve(Vec3::new(left_x, top_y, 0.0), Vec3::new(left_x, top_y, 0.0));
            left_b.add_curve(Vec3::new(left_x, (top_y + bot_y) * 0.5, 0.0), Vec3::new(left_x, bot_y, 0.0));
            left_b.add_curve(Vec3::new(left_x, bot_y, 0.0), Vec3::new(left_x + bracket_hook, bot_y, 0.0));
            for p in &mut left_b.points {
                p.stroke_rgba = stroke;
                p.stroke_width = bracket_sw;
            }
            g.append(left_b);

            // Right bracket ]
            let mut right_b = VGroup::new();
            right_b.start_subpath(Vec3::new(right_x - bracket_hook, top_y, 0.0));
            right_b.add_curve(Vec3::new(right_x, top_y, 0.0), Vec3::new(right_x, top_y, 0.0));
            right_b.add_curve(Vec3::new(right_x, (top_y + bot_y) * 0.5, 0.0), Vec3::new(right_x, bot_y, 0.0));
            right_b.add_curve(Vec3::new(right_x, bot_y, 0.0), Vec3::new(right_x - bracket_hook, bot_y, 0.0));
            for p in &mut right_b.points {
                p.stroke_rgba = stroke;
                p.stroke_width = bracket_sw;
            }
            g.append(right_b);
        }
        "|" => {
            // Determinant | ... |
            g.append(crate::geom::line(Vec3::new(left_x, bot_y, 0.0), Vec3::new(left_x, top_y, 0.0), stroke, bracket_sw));
            g.append(crate::geom::line(Vec3::new(right_x, bot_y, 0.0), Vec3::new(right_x, top_y, 0.0), stroke, bracket_sw));
        }
        _ => {
            // Default parentheses ( ... )
            let mut left_p = VGroup::new();
            let p_top = Vec3::new(left_x, top_y, 0.0);
            let p_mid = Vec3::new(left_x - bracket_hook, (top_y + bot_y) * 0.5, 0.0);
            let p_bot = Vec3::new(left_x, bot_y, 0.0);
            left_p.start_subpath(p_top);
            left_p.add_curve(p_mid, p_bot);
            for p in &mut left_p.points {
                p.stroke_rgba = stroke;
                p.stroke_width = bracket_sw;
            }
            g.append(left_p);

            let mut right_p = VGroup::new();
            let p_rtop = Vec3::new(right_x, top_y, 0.0);
            let p_rmid = Vec3::new(right_x + bracket_hook, (top_y + bot_y) * 0.5, 0.0);
            let p_rbot = Vec3::new(right_x, bot_y, 0.0);
            right_p.start_subpath(p_rtop);
            right_p.add_curve(p_rmid, p_rbot);
            for p in &mut right_p.points {
                p.stroke_rgba = stroke;
                p.stroke_width = bracket_sw;
            }
            g.append(right_p);
        }
    }

    g
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
