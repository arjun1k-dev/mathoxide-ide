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
    /// Elevation / polar tilt angle (radians). 0.0 is top-down (2D). E.g. 70 deg for 3D perspective.
    pub phi: f32,
    /// Azimuthal angle around Z axis (radians).
    pub theta: f32,
    /// Roll angle around camera view axis (radians).
    pub gamma: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            center: Vec3::ZERO,
            frame_height: 8.0,
            width: 1920.0,
            height: 1080.0,
            fov: 45.0_f32.to_radians(),
            phi: 0.0,
            theta: 0.0,
            gamma: 0.0,
        }
    }
}

impl Camera {
    pub fn uniforms(&self) -> FrameUniforms {
        let aspect = self.width / self.height;
        let focal_distance = 0.5 * self.frame_height / (self.fov / 2.0).tan();

        // 3D camera orientation: roll (gamma) * tilt (phi) * azimuth (theta)
        let rot = Mat4::from_rotation_z(self.gamma)
            * Mat4::from_rotation_x(self.phi)
            * Mat4::from_rotation_z(self.theta);

        let view = rot * Mat4::from_translation(-self.center);
        let inv_rot = rot.inverse();
        let eye = self.center + inv_rot.transform_vector3(Vec3::new(0.0, 0.0, focal_distance));

        FrameUniforms {
            view,
            frame_rescale_factors: [
                2.0 / (self.frame_height * aspect),
                2.0 / self.frame_height,
                1.0 / focal_distance,
            ],
            frame_scale: self.frame_height / 2.0,
            camera_position: eye.to_array(),
            pixel_size: self.frame_height / self.height,
            light_position: (eye + inv_rot.transform_vector3(Vec3::new(-10.0, 10.0, 10.0))).to_array(),
            _pad: 0.0,
        }
    }

    /// Rotate the 3D camera angles (d_phi in radians, d_theta in radians).
    pub fn orbit(&mut self, d_phi: f32, d_theta: f32) {
        self.phi = (self.phi + d_phi).clamp(-std::f32::consts::FRAC_PI_2 + 0.01, std::f32::consts::FRAC_PI_2 - 0.01);
        self.theta = (self.theta + d_theta) % std::f32::consts::TAU;
    }

    /// Pan the frame by scene-units delta in screen space.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.center.x -= dx;
        self.center.y -= dy;
    }

    /// Reset camera to default 2D center view.
    pub fn reset(&mut self) {
        self.center = Vec3::ZERO;
        self.frame_height = 8.0;
        self.phi = 0.0;
        self.theta = 0.0;
        self.gamma = 0.0;
    }

    pub fn zoom(&mut self, factor: f32) {
        self.frame_height = (self.frame_height / factor).clamp(0.1, 100.0);
    }

    /// Aspect ratio (width / height).
    pub fn aspect_ratio(&self) -> f32 {
        self.width / self.height
    }

    /// Convert screen pixel coords to scene coords on the z=0 plane.
    pub fn screen_to_scene(&self, x: f32, y: f32) -> Vec3 {
        let aspect = self.aspect_ratio();
        let sx = (x / self.width * 2.0 - 1.0) * self.frame_height * aspect / 2.0;
        let sy = (1.0 - y / self.height * 2.0) * self.frame_height / 2.0;
        Vec3::new(self.center.x + sx, self.center.y + sy, 0.0)
    }

    /// Convert scene coords on the z=0 plane to screen pixel coords.
    pub fn scene_to_screen(&self, x: f32, y: f32) -> glam::Vec2 {
        let aspect = self.aspect_ratio();
        let half_w = self.frame_height * aspect / 2.0;
        let half_h = self.frame_height / 2.0;
        let px = ((x - self.center.x) / half_w + 1.0) * 0.5 * self.width;
        let py = (1.0 - (y - self.center.y) / half_h) * 0.5 * self.height;
        glam::Vec2::new(px, py)
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
