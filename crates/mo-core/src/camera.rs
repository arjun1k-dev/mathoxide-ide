//! Camera: frame center/scale feeding the frame uniforms block.
//! Mirrors manim's FrameUniforms WGSL struct (frame_uniforms.wgsl).

use glam::{Mat4, Vec3};

/// std140 layout matching shaders/inserts/frame_uniforms.wgsl.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FrameUniforms {
    pub view: Mat4,
    pub frame_rescale_factors: [f32; 3],
    pub frame_scale: f32,
    pub camera_position: [f32; 3],
    pub pixel_size: f32,
    pub light_position: [f32; 3],
    pub _pad: f32,
}

#[derive(Clone, Debug)]
pub struct Camera {
    pub center: Vec3,
    /// Height of the frame in scene units.
    pub frame_height: f32,
    pub width: f32,
    pub height: f32,
    pub fov: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            center: Vec3::ZERO,
            frame_height: 8.0,
            width: 1920.0,
            height: 1080.0,
            fov: 45.0_f32.to_radians(),
        }
    }
}

impl Camera {
    pub fn uniforms(&self) -> FrameUniforms {
        let aspect = self.width / self.height;
        let eye = self.center + Vec3::Z * 20.0;
        let view = Mat4::look_at_rh(eye, self.center, Vec3::Y);
        FrameUniforms {
            view,
            frame_rescale_factors: [2.0 / (self.frame_height * aspect), 2.0 / self.frame_height, 1.0],
            frame_scale: self.frame_height / 2.0,
            camera_position: eye.to_array(),
            pixel_size: self.frame_height / self.height,
            light_position: [10.0, 10.0, 20.0],
            _pad: 0.0,
        }
    }

    /// Pan the frame by scene-units delta in screen space.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.center.x -= dx;
        self.center.y -= dy;
    }

    pub fn zoom(&mut self, factor: f32) {
        self.frame_height = (self.frame_height / factor).clamp(0.1, 100.0);
    }

    /// Convert screen pixel coords to scene coords on the z=0 plane.
    pub fn screen_to_scene(&self, x: f32, y: f32) -> Vec3 {
        let aspect = self.width / self.height;
        let sx = (x / self.width * 2.0 - 1.0) * self.frame_height * aspect / 2.0;
        let sy = (1.0 - y / self.height * 2.0) * self.frame_height / 2.0;
        Vec3::new(self.center.x + sx, self.center.y + sy, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_mapping_roundtrip_center() {
        let cam = Camera::default();
        let c = cam.screen_to_scene(cam.width / 2.0, cam.height / 2.0);
        assert!((c.x - cam.center.x).abs() < 1e-5);
        assert!((c.y - cam.center.y).abs() < 1e-5);
    }
}
