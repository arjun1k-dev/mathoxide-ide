//! Offscreen wgpu renderer.
//! Implements ManimGL's real WGSL shaders:
//! - Stencil winding number counting (WINDING_COUNT) + border AA (FILL_BORDER) + cover (WINDING_COVER)
//! - GPU procedural stroke generation with analytic AA and miter joins
//! - Native resolution presenting and sRGB handling

mod pipelines;

pub use pipelines::{GpuPointRecord, MobjectUniforms, Renderer};

/// Render a scene state to an RGBA8 image.
pub fn render_state_to_image(
    renderer: &Renderer,
    state: &std::collections::HashMap<String, mo_core::vgroup::VGroup>,
) -> image::RgbaImage {
    renderer.render(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;
    use mo_core::geom;
    use std::collections::HashMap;

    #[test]
    fn test_renderer_initialization_and_render() {
        let renderer = Renderer::offscreen(320, 240).expect("offscreen renderer");
        let mut state = HashMap::new();

        let c = geom::circle(1.5, Vec3::ZERO, [0.345, 0.768, 0.867, 0.5], [1.0, 1.0, 1.0, 1.0], 4.0);
        state.insert("circle".to_string(), c);

        let img = renderer.render(&state);
        assert_eq!(img.width(), 320);
        assert_eq!(img.height(), 240);

        // Center pixel should be filled with circle color (non-black)
        let center_pixel = img.get_pixel(160, 120);
        assert!(center_pixel[3] > 0, "Center pixel should have alpha > 0");
        assert!(center_pixel[0] > 0 || center_pixel[1] > 0 || center_pixel[2] > 0, "Center should not be black");

        // Corner pixel should be black background
        let corner_pixel = img.get_pixel(10, 10);
        assert_eq!(corner_pixel[0], 0);
        assert_eq!(corner_pixel[1], 0);
        assert_eq!(corner_pixel[2], 0);
    }
}
