//! RGBA colors, [0,1] float space. Hex strings like "#58C4DD" parse here.

pub const BLUE: [f32; 4] = [0x58 as f32 / 255.0, 0xC4 as f32 / 255.0, 0xDD as f32 / 255.0, 1.0];
pub const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
pub const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
pub const YELLOW: [f32; 4] = [1.0, 1.0, 0.0, 1.0];
pub const RED: [f32; 4] = [0xFC as f32 / 255.0, 0x62 as f32 / 255.0, 0x55 as f32 / 255.0, 1.0];
pub const GREEN: [f32; 4] = [0x83 as f32 / 255.0, 0xC1 as f32 / 255.0, 0x67 as f32 / 255.0, 1.0];

pub fn hex(s: &str) -> [f32; 4] {
    let s = s.strip_prefix('#').unwrap_or(s);
    let bytes = s.as_bytes();
    let pair = |i: usize| -> f32 {
        let hi = (bytes[i] as char).to_digit(16).unwrap_or(0) as f32;
        let lo = (bytes[i + 1] as char).to_digit(16).unwrap_or(0) as f32;
        (hi * 16.0 + lo) / 255.0
    };
    let (r, g, b) = (pair(0), pair(2), pair(4));
    let a = if bytes.len() >= 8 { pair(6) } else { 1.0 };
    [r, g, b, a]
}

/// Parse a color literal: named colors or #RRGGBB / #RRGGBBAA.
pub fn parse(s: &str) -> Option<[f32; 4]> {
    let named = match s.to_ascii_lowercase().as_str() {
        "blue" => Some(BLUE),
        "white" => Some(WHITE),
        "black" => Some(BLACK),
        "yellow" => Some(YELLOW),
        "red" => Some(RED),
        "green" => Some(GREEN),
        _ => {
            if s.starts_with('#') && s.len() >= 7 {
                Some(hex(s))
            } else {
                None
            }
        }
    };
    named
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parse() {
        assert_eq!(hex("#FFFFFF")[0], 1.0);
        assert_eq!(hex("#000000")[2], 0.0);
        assert_eq!(hex("#58C4DD")[0], 0x58 as f32 / 255.0);
    }

    #[test]
    fn named() {
        assert_eq!(parse("blue"), Some(BLUE));
        assert_eq!(parse("nope"), None);
    }
}

/// Viridis-style perceptual height ramp (5-stop linear interpolation).
/// t in [0, 1]: deep purple -> blue -> teal -> green -> yellow.
pub fn viridis(t: f32) -> [f32; 4] {
    const STOPS: [[u8; 3]; 5] = [
        [68, 1, 84],
        [59, 82, 139],
        [33, 145, 140],
        [94, 201, 98],
        [253, 231, 37],
    ];
    let t = t.clamp(0.0, 1.0) * (STOPS.len() - 1) as f32;
    let i = (t as usize).min(STOPS.len() - 2);
    let f = t - i as f32;
    let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * f) / 255.0;
    [
        lerp(STOPS[i][0], STOPS[i + 1][0]),
        lerp(STOPS[i][1], STOPS[i + 1][1]),
        lerp(STOPS[i][2], STOPS[i + 1][2]),
        1.0,
    ]
}

#[cfg(test)]
mod viridis_tests {
    use super::viridis;

    #[test]
    fn ramp_endpoints_and_monotonic_g() {
        let lo = viridis(0.0);
        let hi = viridis(1.0);
        assert!(lo[2] > 0.3 && lo[0] < 0.35); // purple-ish
        assert!(hi[0] > 0.9 && hi[1] > 0.85 && hi[2] < 0.2); // yellow-ish
        let mut prev = viridis(0.0)[1];
        for k in 1..=10 {
            let g = viridis(k as f32 / 10.0)[1];
            assert!(g >= prev - 1e-4, "green channel should be near-monotonic");
            prev = g;
        }
    }
}
