//! Linear algebra transformations, matrix deformations, and vector fields powered by `nalgebra`.
//! Powers linear algebra visualizations (Essence of Linear Algebra, eigenvector animations, vector fields).

use crate::vgroup::VGroup;
use glam::Vec3;
use nalgebra::{Matrix2, Matrix3, Rotation2, Vector2};

/// Apply a 2D linear transformation matrix $\begin{pmatrix} a & b \\ c & d \end{pmatrix}$ to a VGroup.
pub fn apply_matrix_2d(g: &mut VGroup, mat: &Matrix2<f32>) {
    for p in &mut g.points {
        let v = Vector2::new(p.point[0], p.point[1]);
        let transformed = mat * v;
        p.point[0] = transformed.x;
        p.point[1] = transformed.y;
    }
}

/// Apply a 3D linear transformation matrix to a VGroup.
pub fn apply_matrix_3d(g: &mut VGroup, mat: &Matrix3<f32>) {
    for p in &mut g.points {
        let v = nalgebra::Vector3::new(p.point[0], p.point[1], p.point[2]);
        let transformed = mat * v;
        p.point = [transformed.x, transformed.y, transformed.z];
    }
}

/// Rotate a VGroup around origin by angle radians using `nalgebra::Rotation2`.
pub fn rotate_2d(g: &mut VGroup, angle_rad: f32) {
    let rot = Rotation2::new(angle_rad);
    apply_matrix_2d(g, &rot.into_inner());
}

/// Generate a 2D vector field visualization for $\vec{F}(x, y)$.
/// Evaluates $\vec{F}$ on a grid and creates an arrow at each point, scaled by magnitude.
pub fn vector_field_2d<F>(
    field: F,
    x_range: (f32, f32, f32), // (min, max, step)
    y_range: (f32, f32, f32), // (min, max, step)
    max_arrow_len: f32,
    stroke: [f32; 4],
    stroke_width: f32,
) -> VGroup
where
    F: Fn(f32, f32) -> (f32, f32),
{
    let mut g = VGroup::new();
    let (x_min, x_max, x_step) = x_range;
    let (y_min, y_max, y_step) = y_range;

    if x_step <= 0.0 || y_step <= 0.0 {
        return g;
    }

    let mut y = y_min;
    while y <= y_max + 1e-4 {
        let mut x = x_min;
        while x <= x_max + 1e-4 {
            let (vx, vy) = field(x, y);
            let mag = (vx * vx + vy * vy).sqrt();
            if mag > 1e-6 {
                let len = mag.min(max_arrow_len);
                let dir_x = vx / mag * len;
                let dir_y = vy / mag * len;

                let from = Vec3::new(x, y, 0.0);
                let to = Vec3::new(x + dir_x, y + dir_y, 0.0);

                let arrow = crate::geom::arrow(
                    from,
                    to,
                    stroke,
                    stroke_width,
                    len * 0.35,
                    len * 0.2,
                );
                g.append(arrow);
            }
            x += x_step;
        }
        y += y_step;
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matrix_shear_and_rotation() {
        let mut c = crate::geom::rect(2.0, 2.0, Vec3::ZERO, [0.0; 4], [1.0, 1.0, 1.0, 1.0], 2.0);
        // Shear matrix [[1, 1], [0, 1]]
        let shear = Matrix2::new(1.0, 1.0, 0.0, 1.0);
        apply_matrix_2d(&mut c, &shear);

        let (_min, max) = c.bounds();
        assert!(max[0] > 1.0); // sheared right
    }

    #[test]
    fn test_vector_field() {
        // Rotational vector field F(x, y) = (-y, x)
        let vf = vector_field_2d(
            |x, y| (-y, x),
            (-2.0, 2.0, 1.0),
            (-2.0, 2.0, 1.0),
            0.6,
            [0.345, 0.768, 0.867, 1.0],
            2.0,
        );
        assert!(!vf.points.is_empty());
        assert!(!vf.subpaths.is_empty());
    }
}
