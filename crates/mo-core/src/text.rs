//! Text: v0.1 stub rendering glyphs as placeholder boxes.
//! Phase 3 replaces this with cosmic-text glyph outlines -> beziers.

use crate::color;
use crate::geom;
use crate::vgroup::VGroup;
use glam::Vec3;

/// Approximate monospaced box text. Returns a VGroup of one rect per char,
/// sufficient for layout demos until real glyph outlines land.
pub fn text(s: &str, font_size: f32) -> VGroup {
    let w = font_size * 0.6;
    let total = w * s.len() as f32;
    let mut g = VGroup::new();
    let _ = total;
    let box_ = geom::rect(w * s.len() as f32, font_size, Vec3::ZERO,
        color::hex("#22222244"), color::WHITE, 2.0);
    g.points = box_.points;
    g.subpaths = box_.subpaths;
    g
}
