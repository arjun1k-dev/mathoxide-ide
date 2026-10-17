//! Complex plane geometry, conformal mappings, and Möbius transformations
//! powered by `num-complex`.

use crate::vgroup::VGroup;
use glam::Vec3;
use num_complex::Complex32;

/// A conformal grid on the complex plane, representing lines of constant real and imaginary parts.
#[derive(Clone, Debug)]
pub struct ComplexGrid {
    pub real_range: (f32, f32, f32), // (min, max, step)
    pub imag_range: (f32, f32, f32), // (min, max, step)
    pub resolution: usize,           // samples per grid line
}

impl Default for ComplexGrid {
    fn default() -> Self {
        Self {
            real_range: (-4.0, 4.0, 1.0),
            imag_range: (-3.0, 3.0, 1.0),
            resolution: 60,
        }
    }
}

impl ComplexGrid {
    /// Render the undeformed Cartesian grid as a VGroup.
    pub fn to_vgroup(&self, stroke: [f32; 4], stroke_width: f32) -> VGroup {
        self.apply_mapping(|z| z, stroke, stroke_width)
    }

    /// Apply an arbitrary complex function $w = f(z)$ to every grid line,
    /// producing a conformal mapping visualization (e.g. $z \mapsto z^2$, $e^z$, $1/z$).
    pub fn apply_mapping<F>(&self, f: F, stroke: [f32; 4], stroke_width: f32) -> VGroup
    where
        F: Fn(Complex32) -> Complex32,
    {
        let mut g = VGroup::new();
        let (r_min, r_max, r_step) = self.real_range;
        let (i_min, i_max, i_step) = self.imag_range;
        let res = self.resolution.max(4);

        // Lines of constant imaginary part (horizontal lines, varying real part)
        if i_step > 0.0 {
            let mut y = i_min;
            while y <= i_max + 1e-4 {
                let mut pts = Vec::with_capacity(res);
                for k in 0..res {
                    let x = r_min + (r_max - r_min) * (k as f32) / ((res - 1) as f32);
                    let z = Complex32::new(x, y);
                    let w = f(z);
                    if w.re.is_finite() && w.im.is_finite() {
                        pts.push(Vec3::new(w.re, w.im, 0.0));
                    }
                }
                if pts.len() >= 2 {
                    let sub = crate::curve::smooth_curve_from_points(&pts, stroke, stroke_width);
                    g.append(sub);
                }
                y += i_step;
            }
        }

        // Lines of constant real part (vertical lines, varying imaginary part)
        if r_step > 0.0 {
            let mut x = r_min;
            while x <= r_max + 1e-4 {
                let mut pts = Vec::with_capacity(res);
                for k in 0..res {
                    let y = i_min + (i_max - i_min) * (k as f32) / ((res - 1) as f32);
                    let z = Complex32::new(x, y);
                    let w = f(z);
                    if w.re.is_finite() && w.im.is_finite() {
                        pts.push(Vec3::new(w.re, w.im, 0.0));
                    }
                }
                if pts.len() >= 2 {
                    let sub = crate::curve::smooth_curve_from_points(&pts, stroke, stroke_width);
                    g.append(sub);
                }
                x += r_step;
            }
        }

        g
    }

    /// Evaluates a Möbius transformation $f(z) = \frac{az + b}{cz + d}$.
    pub fn mobius_transform(
        &self,
        a: Complex32,
        b: Complex32,
        c: Complex32,
        d: Complex32,
        stroke: [f32; 4],
        stroke_width: f32,
    ) -> VGroup {
        self.apply_mapping(
            move |z| {
                let denom = c * z + d;
                if denom.norm_sqr() < 1e-9 {
                    Complex32::new(f32::NAN, f32::NAN)
                } else {
                    (a * z + b) / denom
                }
            },
            stroke,
            stroke_width,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complex_conformal_mapping() {
        let grid = ComplexGrid {
            real_range: (-2.0, 2.0, 1.0),
            imag_range: (-2.0, 2.0, 1.0),
            resolution: 20,
        };
        // z -> z^2
        let mapped = grid.apply_mapping(|z| z * z, [1.0, 1.0, 1.0, 1.0], 2.0);
        assert!(mapped.curve_count() > 0);
    }
}
