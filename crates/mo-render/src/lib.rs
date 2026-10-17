//! Offscreen wgpu renderer. Fill uses manim's winding-stencil technique:
//! per bezier, a fan triangle (interior) + a curve-hugging triangle whose
//! outside is discarded in uv space (curve coincides with y = x^2).
//! Stencil winding counts inside-ness; a cover pass shades where nonzero.

mod pipelines;

pub use pipelines::Renderer;

/// Render a scene state to an RGBA8 image.
pub fn render_state_to_image(
    renderer: &Renderer,
    state: &std::collections::HashMap<String, mo_core::vgroup::VGroup>,
) -> image::RgbaImage {
    renderer.render(state)
}
