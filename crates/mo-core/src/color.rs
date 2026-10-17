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
