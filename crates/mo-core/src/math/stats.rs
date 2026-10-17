//! Probability distributions and special functions powered by `statrs`.
//! Used for probability visualizations, Bell curves, and statistical animations.

use statrs::distribution::{Continuous, ContinuousCDF, Exp, Gamma, Normal};
use statrs::function::erf::erf;
use statrs::function::gamma::gamma;

/// Normal (Gaussian) Probability Density Function (Bell curve).
pub fn normal_pdf(x: f64, mean: f64, std_dev: f64) -> f64 {
    if std_dev <= 0.0 {
        return 0.0;
    }
    match Normal::new(mean, std_dev) {
        Ok(dist) => dist.pdf(x),
        Err(_) => 0.0,
    }
}

/// Normal (Gaussian) Cumulative Distribution Function.
pub fn normal_cdf(x: f64, mean: f64, std_dev: f64) -> f64 {
    if std_dev <= 0.0 {
        return 0.0;
    }
    match Normal::new(mean, std_dev) {
        Ok(dist) => dist.cdf(x),
        Err(_) => 0.0,
    }
}

/// Exponential distribution PDF: $f(x) = \lambda e^{-\lambda x}$ for $x \ge 0$.
pub fn exponential_pdf(x: f64, rate: f64) -> f64 {
    if rate <= 0.0 || x < 0.0 {
        return 0.0;
    }
    match Exp::new(rate) {
        Ok(dist) => dist.pdf(x),
        Err(_) => 0.0,
    }
}

/// Gamma distribution PDF with shape $\alpha$ and rate $\beta$.
pub fn gamma_pdf(x: f64, shape: f64, rate: f64) -> f64 {
    if shape <= 0.0 || rate <= 0.0 || x <= 0.0 {
        return 0.0;
    }
    match Gamma::new(shape, rate) {
        Ok(dist) => dist.pdf(x),
        Err(_) => 0.0,
    }
}

/// The Gauss error function $\text{erf}(x) = \frac{2}{\sqrt{\pi}} \int_0^x e^{-t^2} dt$.
pub fn error_function(x: f64) -> f64 {
    erf(x)
}

/// The Euler Gamma function $\Gamma(x) = \int_0^\infty t^{x-1} e^{-t} dt$.
pub fn gamma_function(x: f64) -> f64 {
    gamma(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_distribution() {
        // Standard normal peak at x = 0 is 1 / sqrt(2 * PI) ~ 0.39894
        let peak = normal_pdf(0.0, 0.0, 1.0);
        assert!((peak - 0.39894228).abs() < 1e-5);

        // CDF at mean is 0.5
        let mid = normal_cdf(0.0, 0.0, 1.0);
        assert!((mid - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_gamma_function() {
        // Gamma(5) = 4! = 24
        let val = gamma_function(5.0);
        assert!((val - 24.0).abs() < 1e-5);
    }
}
