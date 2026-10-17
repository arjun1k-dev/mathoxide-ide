//! Discrete Fourier Transform and Fourier Series epicycles powered by `rustfft`.
//! Powers Fourier transform animations, harmonic analysis, and epicyclic curve tracing.

use glam::Vec3;
use num_complex::Complex32;
use rustfft::{FftPlanner, num_complex::Complex};
use std::sync::Arc;

/// A Fourier harmonic component with frequency index, amplitude, and phase offset.
#[derive(Clone, Debug, PartialEq)]
pub struct FourierCoefficient {
    pub freq: i32,
    pub amplitude: f32,
    pub phase: f32,
}

impl FourierCoefficient {
    /// Evaluate this harmonic vector at parameter $t \in [0, 1]$.
    pub fn vector_at(&self, t: f32) -> Vec3 {
        let angle = std::f32::consts::TAU * (self.freq as f32) * t + self.phase;
        Vec3::new(self.amplitude * angle.cos(), self.amplitude * angle.sin(), 0.0)
    }
}

/// Compute the 1D Discrete Fourier Transform of a complex signal using `rustfft`.
pub fn fft_1d(signal: &[Complex32]) -> Vec<Complex32> {
    let n = signal.len();
    if n == 0 {
        return Vec::new();
    }
    let mut planner = FftPlanner::new();
    let fft: Arc<dyn rustfft::Fft<f32>> = planner.plan_fft_forward(n);

    let mut buffer: Vec<Complex<f32>> = signal.iter().map(|c| Complex::new(c.re, c.im)).collect();
    fft.process(&mut buffer);

    let scale = 1.0 / (n as f32);
    buffer.into_iter().map(|c| Complex32::new(c.re * scale, c.im * scale)).collect()
}

/// Decompose a 2D closed curve (sampled as Vec3 points) into $K$ Fourier harmonic coefficients,
/// sorted by descending amplitude for epicycle animation.
pub fn fourier_series_decomposition(points: &[Vec3], max_freq: usize) -> Vec<FourierCoefficient> {
    let n = points.len();
    if n == 0 {
        return Vec::new();
    }

    let signal: Vec<Complex32> = points.iter().map(|p| Complex32::new(p.x, p.y)).collect();
    let spectrum = fft_1d(&signal);

    let mut coeffs = Vec::new();
    let max_k = max_freq.min(n / 2);

    for k in 0..=max_k {
        // Positive frequency k
        let c_pos = spectrum[k];
        coeffs.push(FourierCoefficient {
            freq: k as i32,
            amplitude: c_pos.norm(),
            phase: c_pos.arg(),
        });

        // Negative frequency -k (except k == 0)
        if k > 0 {
            let neg_idx = n - k;
            let c_neg = spectrum[neg_idx];
            coeffs.push(FourierCoefficient {
                freq: -(k as i32),
                amplitude: c_neg.norm(),
                phase: c_neg.arg(),
            });
        }
    }

    // Sort by descending amplitude (largest circles drawn first in epicycles)
    coeffs.sort_by(|a, b| b.amplitude.partial_cmp(&a.amplitude).unwrap_or(std::cmp::Ordering::Equal));
    coeffs
}

/// Evaluate the reconstructed position from Fourier coefficients at normalized parameter $t \in [0, 1]$.
pub fn evaluate_fourier_series(coeffs: &[FourierCoefficient], t: f32) -> Vec3 {
    let mut pt = Vec3::ZERO;
    for c in coeffs {
        pt += c.vector_at(t);
    }
    pt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fft_sine_recovery() {
        let n = 64;
        let mut pts = Vec::with_capacity(n);
        for i in 0..n {
            let t = (i as f32) / (n as f32) * std::f32::consts::TAU;
            pts.push(Vec3::new(t.cos(), t.sin(), 0.0));
        }

        let coeffs = fourier_series_decomposition(&pts, 5);
        assert!(!coeffs.is_empty());
        // Dominant frequency for e^{it} is freq = 1 with amplitude ~1.0
        let dom = &coeffs[0];
        assert_eq!(dom.freq, 1);
        assert!((dom.amplitude - 1.0).abs() < 1e-3);
    }
}
