//! Rate functions, ported verbatim from manimlib/utils/rate_functions.py.

pub type RateFn = fn(f32) -> f32;

pub fn linear(t: f32) -> f32 {
    t
}

pub fn smooth(t: f32) -> f32 {
    let s = 1.0 - t;
    t.powi(3) * (10.0 * s * s + 5.0 * s * t + t * t)
}

pub fn rush_into(t: f32) -> f32 {
    2.0 * smooth(0.5 * t)
}

pub fn rush_from(t: f32) -> f32 {
    2.0 * smooth(0.5 * (t + 1.0)) - 1.0
}

pub fn slow_into(t: f32) -> f32 {
    (1.0 - (1.0 - t) * (1.0 - t)).sqrt()
}

pub fn double_smooth(t: f32) -> f32 {
    if t < 0.5 {
        0.5 * smooth(2.0 * t)
    } else {
        0.5 * (1.0 + smooth(2.0 * t - 1.0))
    }
}

pub fn there_and_back(t: f32) -> f32 {
    let new_t = if t < 0.5 { 2.0 * t } else { 2.0 * (1.0 - t) };
    smooth(new_t)
}

pub fn wiggle(t: f32) -> f32 {
    there_and_back(t) * (2.0 * std::f32::consts::PI * t).sin()
}

pub fn exponential_decay(t: f32) -> f32 {
    1.0 - (-t / 0.1).exp()
}

pub fn by_name(name: &str) -> Option<RateFn> {
    Some(match name {
        "linear" => linear,
        "smooth" => smooth,
        "rush_into" => rush_into,
        "rush_from" => rush_from,
        "slow_into" => slow_into,
        "double_smooth" => double_smooth,
        "there_and_back" => there_and_back,
        "wiggle" => wiggle,
        "exponential_decay" => exponential_decay,
        _ => return None,
    })
}

pub fn names() -> &'static [&'static str] {
    &[
        "linear", "smooth", "rush_into", "rush_from", "slow_into", "double_smooth",
        "there_and_back", "wiggle", "exponential_decay",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints() {
        for f in [linear, smooth, rush_into, rush_from, slow_into, double_smooth] {
            assert_eq!(f(0.0), 0.0);
            assert!((f(1.0) - 1.0).abs() < 1e-6);
        }
        // returns to its start
        assert!(there_and_back(1.0).abs() < 1e-6);
    }

    #[test]
    fn smooth_midpoint() {
        assert!((smooth(0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn there_and_back_returns() {
        assert!(there_and_back(1.0).abs() < 1e-6);
        assert!((there_and_back(0.25) - 0.5).abs() < 1e-6);
    }
}
