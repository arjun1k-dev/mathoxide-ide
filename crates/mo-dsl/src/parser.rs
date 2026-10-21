//! Hand-rolled lexer + recursive-descent parser for the .moa DSL.
//! Zero parser-framework deps, exact line/col diagnostics.

use glam::Vec3;
use mo_core::anim::{Animation, TimedAnim};
use mo_core::color;
use mo_core::rate;
use mo_core::scene::{Stmt, Timeline};
use mo_core::vgroup::VGroup;

#[derive(Debug, Clone)]
pub struct Diag {
    pub line: usize,
    pub col: usize,
    pub message: String,
}

// -- Lexer --------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Num(f32),
    Str(String),
    Hash(String), // hex digits after '#'
    LParen,
    RParen,
    Comma,
    Semi,
    Eq,
    Dot,
}

struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Self { src: src.as_bytes(), pos: 0, line: 1, col: 1 }
    }

    fn bump(&mut self) -> Option<u8> {
        let c = *self.src.get(self.pos)?;
        self.pos += 1;
        if c == b'\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn peek2(&self) -> Option<u8> {
        self.src.get(self.pos + 1).copied()
    }

    fn skip_ws_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_ascii_whitespace() => {
                    self.bump();
                }
                Some(b'/') if self.peek2() == Some(b'/') => {
                    while let Some(c) = self.bump() {
                        if c == b'\n' {
                            break;
                        }
                    }
                }
                Some(b'/') if self.peek2() == Some(b'*') => {
                    self.bump();
                    self.bump();
                    while self.peek().is_some() {
                        if self.peek() == Some(b'*') && self.peek2() == Some(b'/') {
                            self.bump();
                            self.bump();
                            break;
                        }
                        self.bump();
                    }
                }
                _ => break,
            }
        }
    }

    fn next(&mut self) -> Result<Option<(Tok, usize, usize)>, Diag> {
        self.skip_ws_and_comments();
        let (line, col) = (self.line, self.col);
        let Some(c) = self.peek() else { return Ok(None) };
        let tok = if c == b'-'
            && self
                .peek2()
                .is_some_and(|d| d.is_ascii_digit() || d == b'.')
        {
            self.bump();
            let mut s = String::from("-");
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() || c == b'.' {
                    s.push(c as char);
                    self.bump();
                } else {
                    break;
                }
            }
            Tok::Num(s.parse().map_err(|_| Diag { line, col, message: format!("bad number `{s}`") })?)
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let mut s = String::new();
            while let Some(c) = self.peek() {
                if c.is_ascii_alphanumeric() || c == b'_' {
                    s.push(c as char);
                    self.bump();
                } else {
                    break;
                }
            }
            Tok::Ident(s)
        } else if c.is_ascii_digit()
            || (c == b'.' && self.peek2().is_some_and(|d| d.is_ascii_digit()))
        {
            let mut s = String::new();
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() || c == b'.' {
                    s.push(c as char);
                    self.bump();
                } else {
                    break;
                }
            }
            let f: f32 = s.parse().map_err(|_| Diag {
                line,
                col,
                message: format!("bad number `{s}`"),
            })?;
            Tok::Num(f)
        } else if c == b'.' {
            self.bump();
            Tok::Dot
        } else if c == b'#' {
            self.bump();
            let mut s = String::new();
            while let Some(c) = self.peek() {
                if c.is_ascii_hexdigit() {
                    s.push(c as char);
                    self.bump();
                } else {
                    break;
                }
            }
            Tok::Hash(s)
        } else if c == b'r' && self.peek2() == Some(b'"') {
            self.bump(); // 'r'
            self.bump(); // '"'
            let mut s = String::new();
            let mut closed = false;
            while let Some(ch) = self.bump() {
                if ch == b'"' {
                    closed = true;
                    break;
                }
                s.push(ch as char);
            }
            if !closed {
                return Err(Diag {
                    line,
                    col,
                    message: "unclosed raw string literal".to_string(),
                });
            }
            Tok::Str(s)
        } else if c == b'"' {
            self.bump();
            let mut s = String::new();
            let mut closed = false;
            while let Some(ch) = self.bump() {
                if ch == b'"' {
                    closed = true;
                    break;
                } else if ch == b'\\' {
                    if let Some(next) = self.bump() {
                        match next {
                            b'n' if self.peek().is_some_and(|c| matches!(c, b'e' | b'a' | b'u' | b'o')) => {
                                s.push('\\');
                                s.push('n');
                            }
                            b'n' => s.push('\n'),
                            b't' if self.peek().is_some_and(|c| matches!(c, b'h' | b'i' | b'a' | b'o' | b'e' | b'r')) => {
                                s.push('\\');
                                s.push('t');
                            }
                            b't' => s.push('\t'),
                            b'r' if self.peek().is_some_and(|c| matches!(c, b'i' | b'h' | b'a')) => {
                                s.push('\\');
                                s.push('r');
                            }
                            b'r' => s.push('\r'),
                            b'\\' => s.push('\\'),
                            b'"' => s.push('"'),
                            other => {
                                s.push('\\');
                                s.push(other as char);
                            }
                        }
                    }
                } else {
                    s.push(ch as char);
                }
            }
            if !closed {
                return Err(Diag {
                    line,
                    col,
                    message: "unterminated string literal".into(),
                });
            }
            Tok::Str(s)
        } else {
            self.bump();
            match c {
                b'(' => Tok::LParen,
                b')' => Tok::RParen,
                b',' => Tok::Comma,
                b';' => Tok::Semi,
                b'=' => Tok::Eq,
                other => {
                    return Err(Diag {
                        line,
                        col,
                        message: format!("unexpected character `{}`", other as char),
                    })
                }
            }
        };
        Ok(Some((tok, line, col)))
    }
}

// -- Parser -------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Val {
    F(f32),
    V(Vec3),
    C([f32; 4]),
    S(String),
    Ident(String),
    Mat2([f32; 4]),
}

use std::path::PathBuf;

#[derive(Debug)]
enum RawStmt {
    Decl { kind: String, name: String, params: Vec<(String, Val)> },
    Play { name: String, args: Vec<String>, run_time: Option<f32>, rate: Option<String> },
    CameraPlay { kind: String, params: Vec<(String, f32)>, run_time: Option<f32>, rate: Option<String> },
    Wait(f32),
    Import { path: String, alias: Option<String>, at: Option<Vec3> },
}

struct P {
    toks: Vec<(Tok, usize, usize)>,
    idx: usize,
}

impl P {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.idx).map(|(t, _, _)| t)
    }

    fn pos(&self) -> (usize, usize) {
        self.toks.get(self.idx).map(|(_, l, c)| (*l, *c)).unwrap_or((1, 1))
    }

    fn bump(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.idx).map(|(t, _, _)| t.clone());
        if t.is_some() {
            self.idx += 1;
        }
        t
    }

    fn eat(&mut self, expect: &Tok) -> bool {
        if self.peek() == Some(expect) {
            self.idx += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, expect: &Tok, what: &str) -> Result<(), Diag> {
        let (l, c) = self.pos();
        if self.eat(expect) {
            Ok(())
        } else {
            Err(Diag { line: l, col: c, message: format!("expected {what}") })
        }
    }

    fn err_here(&self, msg: &str) -> Diag {
        let (l, c) = self.pos();
        Diag { line: l, col: c, message: msg.into() }
    }

    fn value(&mut self) -> Result<Val, Diag> {
        let (l, c) = self.pos();
        match self.bump() {
            Some(Tok::Num(f)) => Ok(Val::F(f)),
            Some(Tok::Str(s)) => Ok(Val::S(s)),
            Some(Tok::LParen) => {
                let mut xs = Vec::new();
                loop {
                    match self.bump() {
                        Some(Tok::Num(f)) => xs.push(f),
                        Some(Tok::RParen) => break,
                        Some(Tok::Comma) => continue,
                        other => {
                            return Err(Diag {
                                line: l,
                                col: c,
                                message: format!("bad vector or tuple literal, got {other:?}"),
                            })
                        }
                    }
                }
                match xs.len() {
                    2 => Ok(Val::V(Vec3::new(xs[0], xs[1], 0.0))),
                    3 => Ok(Val::V(Vec3::new(xs[0], xs[1], xs[2]))),
                    4 => Ok(Val::Mat2([xs[0], xs[1], xs[2], xs[3]])),
                    n => Err(Diag {
                        line: l,
                        col: c,
                        message: format!("tuple with {n} components, need 2, 3, or 4"),
                    }),
                }
            }
            Some(Tok::Hash(s)) => color::parse(&format!("#{s}"))
                .map(Val::C)
                .ok_or_else(|| Diag { line: l, col: c, message: format!("bad color `#{s}`") }),
            Some(Tok::Ident(s)) => {
                if let Some(c) = color::parse(&s) {
                    Ok(Val::C(c))
                } else {
                    Ok(Val::Ident(s))
                }
            }
            other => Err(Diag { line: l, col: c, message: format!("expected a value, got {other:?}") }),
        }
    }

    fn obj_decl(&mut self) -> Result<RawStmt, Diag> {
        let (kind, name) = match (self.bump(), self.bump()) {
            (Some(Tok::Ident(k)), Some(Tok::Ident(n))) => (k, n),
            _ => return Err(self.err_here("expected `<kind> <name>`")),
        };
        let mut params = Vec::new();
        loop {
            match self.peek() {
                Some(Tok::Semi) | None => break,
                Some(Tok::Str(s)) => {
                    let s = s.clone();
                    self.idx += 1;
                    params.push(("text".into(), Val::S(s)));
                    self.eat(&Tok::Comma);
                }
                Some(Tok::Ident(key)) => {
                    let key = key.clone();
                    self.idx += 1;
                    // `key = value` or the `at (x,y)` shorthand (no `=`)
                    if !self.eat(&Tok::Eq) && self.peek() != Some(&Tok::LParen) {
                        return Err(self.err_here("`=`"));
                    }
                    let v = self.value()?;
                    params.push((key, v));
                    self.eat(&Tok::Comma);
                }
                Some(t) => return Err(self.err_here(&format!("unexpected `{t:?}` in object params"))),
            }
        }
        self.expect(&Tok::Semi, "`;`")?;
        Ok(RawStmt::Decl { kind, name, params })
    }

    fn play_stmt(&mut self) -> Result<RawStmt, Diag> {
        self.idx += 1; // "play"
        let (l, c) = self.pos();
        let name = match self.bump() {
            Some(Tok::Ident(s)) => s,
            _ => {
                return Err(Diag {
                    line: l,
                    col: c,
                    message: "expected animation name after `play`".into(),
                })
            }
        };
        // camera.orbit(...) / camera.zoom(...) / camera.move_to(...)
        if name == "camera" && self.eat(&Tok::Dot) {
            let kind = match self.bump() {
                Some(Tok::Ident(s)) => s,
                _ => return Err(self.err_here("expected camera animation name (orbit/zoom/move_to)")),
            };
            if !matches!(kind.as_str(), "orbit" | "zoom" | "move_to") {
                return Err(self.err_here(&format!("unknown camera animation `camera.{kind}`")));
            }
            self.expect(&Tok::LParen, "`(`")?;
            let mut params: Vec<(String, f32)> = Vec::new();
            loop {
                match self.peek() {
                    Some(Tok::RParen) | None => break,
                    Some(Tok::Ident(k)) => {
                        let k = k.clone();
                        self.idx += 1;
                        self.expect(&Tok::Eq, "`=`")?;
                        let (l2, c2) = self.pos();
                        let v = match self.bump() {
                            Some(Tok::Num(f)) => f,
                            _ => return Err(Diag { line: l2, col: c2, message: "expected number".into() }),
                        };
                        params.push((k, v));
                        if !self.eat(&Tok::Comma) {
                            break;
                        }
                    }
                    Some(t) => return Err(self.err_here(&format!("unexpected `{t:?}` in camera animation args"))),
                }
            }
            self.expect(&Tok::RParen, "`)`")?;
            let run_time = match self.peek() {
                Some(Tok::Num(f)) => {
                    let f = *f;
                    self.idx += 1;
                    self.expect(&Tok::Ident("s".into()), "`s` after duration")?;
                    Some(f)
                }
                _ => None,
            };
            let rate = match self.peek() {
                Some(Tok::Ident(s)) if rate::by_name(s).is_some() => {
                    let s = s.clone();
                    self.idx += 1;
                    Some(s)
                }
                _ => None,
            };
            self.eat(&Tok::Comma);
            self.expect(&Tok::Semi, "`;`")?;
            return Ok(RawStmt::CameraPlay { kind, params, run_time, rate });
        }
        self.expect(&Tok::LParen, "`(`")?;
        let mut args = Vec::new();
        loop {
            match self.peek() {
                Some(Tok::RParen) | None => break,
                Some(Tok::Ident(s)) => {
                    args.push(s.clone());
                    self.idx += 1;
                    if !self.eat(&Tok::Comma) {
                        break;
                    }
                }
                // Numeric argument (e.g. the angle in `rotate(c, 1.5708)`).
                Some(Tok::Num(f)) => {
                    args.push(format!("{f}"));
                    self.idx += 1;
                    if !self.eat(&Tok::Comma) {
                        break;
                    }
                }
                Some(t) => return Err(self.err_here(&format!("unexpected `{t:?}` in animation args"))),
            }
        }
        self.expect(&Tok::RParen, "`)`")?;
        // optional `2s`
        let run_time = match self.peek() {
            Some(Tok::Num(f)) => {
                let f = *f;
                self.idx += 1;
                self.expect(&Tok::Ident("s".into()), "`s` after duration")?;
                Some(f)
            }
            _ => None,
        };
        // optional easing name
        let rate = match self.peek() {
            Some(Tok::Ident(s)) if rate::by_name(s).is_some() => {
                let s = s.clone();
                self.idx += 1;
                Some(s)
            }
            _ => None,
        };
        self.eat(&Tok::Comma);
        self.expect(&Tok::Semi, "`;`")?;
        Ok(RawStmt::Play { name, args, run_time, rate })
    }

    fn wait_stmt(&mut self) -> Result<RawStmt, Diag> {
        let (l, c) = self.pos();
        self.idx += 1; // "wait"
        let f = match self.bump() {
            Some(Tok::Num(f)) => f,
            _ => return Err(Diag { line: l, col: c, message: "expected duration after `wait`".into() }),
        };
        self.expect(&Tok::Ident("s".into()), "`s`")?;
        self.eat(&Tok::Comma);
        self.expect(&Tok::Semi, "`;`")?;
        Ok(RawStmt::Wait(f))
    }

    fn import_stmt(&mut self) -> Result<RawStmt, Diag> {
        let (l, c) = self.pos();
        self.idx += 1; // "import"
        let path = match self.bump() {
            Some(Tok::Str(s)) => s,
            _ => return Err(Diag { line: l, col: c, message: "expected file path string after `import`".into() }),
        };

        let mut alias = None;
        if let Some(Tok::Ident(k)) = self.peek() {
            if k == "as" {
                self.idx += 1;
                match self.bump() {
                    Some(Tok::Ident(name)) => alias = Some(name),
                    _ => return Err(self.err_here("expected identifier after `as`")),
                }
            }
        }

        let mut at = None;
        if let Some(Tok::Ident(k)) = self.peek() {
            if k == "at" {
                self.idx += 1;
                match self.value()? {
                    Val::V(v) => at = Some(v),
                    _ => return Err(self.err_here("expected vector after `at`")),
                }
            }
        }

        self.eat(&Tok::Comma);
        self.expect(&Tok::Semi, "`;`")?;
        Ok(RawStmt::Import { path, alias, at })
    }
}

// -- Lowering -----------------------------------------------------------------

fn get_f(params: &[(String, Val)], k: &str) -> Option<f32> {
    params.iter().find(|(key, _)| key == k).and_then(|(_, v)| match v {
        Val::F(f) => Some(*f),
        _ => None,
    })
}

fn get_v(params: &[(String, Val)], k: &str) -> Option<Vec3> {
    params.iter().find(|(key, _)| key == k).and_then(|(_, v)| match v {
        Val::V(v) => Some(*v),
        _ => None,
    })
}

fn get_pair(params: &[(String, Val)], k: &str) -> Option<(f32, f32)> {
    get_v(params, k).map(|v| (v.x, v.y))
}

fn get_c(params: &[(String, Val)], k: &str) -> Option<[f32; 4]> {
    params.iter().find(|(key, _)| key == k).and_then(|(_, v)| match v {
        Val::C(c) => Some(*c),
        _ => None,
    })
}

fn get_s(params: &[(String, Val)], k: &str) -> Option<String> {
    params.iter().find(|(key, _)| key == k).and_then(|(_, v)| match v {
        Val::S(s) | Val::Ident(s) => Some(s.clone()),
        _ => None,
    })
}

fn get_mat2(params: &[(String, Val)], k: &str) -> Option<[f32; 4]> {
    params.iter().find(|(key, _)| key == k).and_then(|(_, v)| match v {
        Val::Mat2(m) => Some(*m),
        _ => None,
    })
}

fn build_object(
    kind: &str,
    name: &str,
    params: &[(String, Val)],
    axes_map: &mut std::collections::HashMap<String, mo_core::Axes>,
) -> Result<VGroup, String> {
    let fill = get_c(params, "fill").or_else(|| get_c(params, "color")).unwrap_or([0.0; 4]);
    let stroke = get_c(params, "stroke").or_else(|| get_c(params, "color")).unwrap_or(color::BLUE);
    let sw = get_f(params, "sw").unwrap_or(4.0);
    let at = get_v(params, "at").unwrap_or(Vec3::ZERO);

    let mut obj = match kind {
        "circle" => mo_core::geom::circle(get_f(params, "r").unwrap_or(1.0), at, fill, stroke, sw),
        "dot" => {
            let dot_color = get_c(params, "fill").or_else(|| get_c(params, "color")).unwrap_or(color::WHITE);
            let r = get_f(params, "r").unwrap_or(0.08);
            mo_core::geom::dot(at, r, dot_color)
        }
        "arc" => {
            let r = get_f(params, "r").or_else(|| get_f(params, "radius")).unwrap_or(1.0);
            let start = get_f(params, "start").or_else(|| get_f(params, "start_angle")).unwrap_or(0.0);
            let end = get_f(params, "end").or_else(|| get_f(params, "end_angle")).unwrap_or(std::f32::consts::TAU);
            mo_core::geom::arc(r, start, end, at, fill, stroke, sw)
        }
        "rect" => mo_core::geom::rect(
            get_f(params, "w").unwrap_or(2.0),
            get_f(params, "h").unwrap_or(1.0),
            at,
            fill,
            stroke,
            sw,
        ),
        "roundedrect" | "rounded_rect" => {
            let w = get_f(params, "w").unwrap_or(2.0);
            let h = get_f(params, "h").unwrap_or(1.0);
            let r = get_f(params, "corner_radius").or_else(|| get_f(params, "r")).unwrap_or(0.3);
            mo_core::geom::rounded_rect(w, h, r, at, fill, stroke, sw)
        }
        "square" => {
            let side = get_f(params, "side")
                .or_else(|| get_f(params, "s"))
                .or_else(|| get_f(params, "r"))
                .unwrap_or(2.0);
            mo_core::geom::square(side, at, fill, stroke, sw)
        }
        "ellipse" => {
            let rx = get_f(params, "rx").or_else(|| get_f(params, "w").map(|w| w * 0.5)).unwrap_or(1.5);
            let ry = get_f(params, "ry").or_else(|| get_f(params, "h").map(|h| h * 0.5)).unwrap_or(1.0);
            mo_core::geom::ellipse(rx, ry, at, fill, stroke, sw)
        }
        "regularpolygon" | "regular_polygon" => {
            let n = get_f(params, "n").or_else(|| get_f(params, "sides")).unwrap_or(5.0) as usize;
            let r = get_f(params, "r").or_else(|| get_f(params, "radius")).unwrap_or(1.0);
            mo_core::geom::regular_polygon(n, r, at, fill, stroke, sw)
        }
        "triangle" => {
            let side = get_f(params, "side").or_else(|| get_f(params, "s")).unwrap_or(2.0);
            mo_core::geom::triangle(side, at, fill, stroke, sw)
        }
        "annulus" => {
            let inner_r = get_f(params, "inner_r").or_else(|| get_f(params, "r_inner")).unwrap_or(1.0);
            let outer_r = get_f(params, "outer_r").or_else(|| get_f(params, "r_outer")).or_else(|| get_f(params, "r")).unwrap_or(2.0);
            mo_core::geom::annulus(inner_r, outer_r, at, fill, stroke, sw)
        }
        "sector" => {
            let r = get_f(params, "r").or_else(|| get_f(params, "radius")).unwrap_or(1.5);
            let start = get_f(params, "start").or_else(|| get_f(params, "start_angle")).unwrap_or(0.0);
            let end = get_f(params, "end").or_else(|| get_f(params, "end_angle")).or_else(|| get_f(params, "angle")).unwrap_or(std::f32::consts::FRAC_PI_2);
            mo_core::geom::sector(r, start, end, at, fill, stroke, sw)
        }
        "annularsector" | "annular_sector" => {
            let inner_r = get_f(params, "inner_r").or_else(|| get_f(params, "r_inner")).unwrap_or(1.0);
            let outer_r = get_f(params, "outer_r").or_else(|| get_f(params, "r_outer")).or_else(|| get_f(params, "r")).unwrap_or(2.0);
            let start = get_f(params, "start").or_else(|| get_f(params, "start_angle")).unwrap_or(0.0);
            let end = get_f(params, "end").or_else(|| get_f(params, "end_angle")).unwrap_or(std::f32::consts::FRAC_PI_2);
            mo_core::geom::annular_sector(inner_r, outer_r, start, end, at, fill, stroke, sw)
        }
        "star" => {
            let n = get_f(params, "n").or_else(|| get_f(params, "points")).unwrap_or(5.0) as usize;
            let outer_r = get_f(params, "outer_r").or_else(|| get_f(params, "r")).unwrap_or(1.5);
            let inner_r = get_f(params, "inner_r").unwrap_or(0.6);
            mo_core::geom::star(n, outer_r, inner_r, at, fill, stroke, sw)
        }
        "brace" => {
            let from = get_v(params, "from").unwrap_or(Vec3::new(-1.5, 0.0, 0.0)) + at;
            let to = get_v(params, "to").unwrap_or(Vec3::new(1.5, 0.0, 0.0)) + at;
            let depth = get_f(params, "depth").unwrap_or(0.4);
            mo_core::geom::brace(from, to, depth, stroke, sw)
        }
        "line" => mo_core::geom::line(
            get_v(params, "from").unwrap_or(Vec3::ZERO) + at,
            get_v(params, "to").unwrap_or(Vec3::new(1.0, 0.0, 0.0)) + at,
            stroke,
            sw,
        ),
        "dashedline" | "dashed_line" => {
            let from = get_v(params, "from").unwrap_or(Vec3::ZERO) + at;
            let to = get_v(params, "to").unwrap_or(Vec3::new(1.0, 0.0, 0.0)) + at;
            let dash = get_f(params, "dash").or_else(|| get_f(params, "dash_len")).unwrap_or(0.15);
            let space = get_f(params, "space").or_else(|| get_f(params, "space_len")).unwrap_or(0.1);
            mo_core::geom::dashed_line(from, to, dash, space, stroke, sw)
        }
        "tangentline" | "tangent_line" => {
            let angle = get_f(params, "angle").unwrap_or(0.0);
            let length = get_f(params, "length").or_else(|| get_f(params, "len")).unwrap_or(2.0);
            mo_core::geom::tangent_line(at, angle, length, stroke, sw)
        }
        "arrow" => {
            let tip_len = get_f(params, "tip_len").unwrap_or(0.35);
            let tip_w = get_f(params, "tip_w").unwrap_or(0.2);
            let arrow_stroke = get_c(params, "stroke").or_else(|| get_c(params, "color")).unwrap_or(stroke);
            mo_core::geom::arrow(
                get_v(params, "from").unwrap_or(Vec3::ZERO) + at,
                get_v(params, "to").unwrap_or(Vec3::new(1.0, 0.0, 0.0)) + at,
                arrow_stroke,
                sw,
                tip_len,
                tip_w,
            )
        }
        "doublearrow" | "double_arrow" => {
            let tip_len = get_f(params, "tip_len").unwrap_or(0.35);
            let tip_w = get_f(params, "tip_w").unwrap_or(0.2);
            let arrow_stroke = get_c(params, "stroke").or_else(|| get_c(params, "color")).unwrap_or(stroke);
            mo_core::geom::double_arrow(
                get_v(params, "from").unwrap_or(Vec3::ZERO) + at,
                get_v(params, "to").unwrap_or(Vec3::new(1.0, 0.0, 0.0)) + at,
                arrow_stroke,
                sw,
                tip_len,
                tip_w,
            )
        }
        "curvedarrow" | "curved_arrow" => {
            let tip_len = get_f(params, "tip_len").unwrap_or(0.35);
            let tip_w = get_f(params, "tip_w").unwrap_or(0.2);
            let angle = get_f(params, "angle").unwrap_or(0.5);
            let arrow_stroke = get_c(params, "stroke").or_else(|| get_c(params, "color")).unwrap_or(stroke);
            mo_core::geom::curved_arrow(
                get_v(params, "from").unwrap_or(Vec3::ZERO) + at,
                get_v(params, "to").unwrap_or(Vec3::new(1.0, 0.0, 0.0)) + at,
                angle,
                arrow_stroke,
                sw,
                tip_len,
                tip_w,
            )
        }
        "curveddoublearrow" | "curved_double_arrow" => {
            let tip_len = get_f(params, "tip_len").unwrap_or(0.35);
            let tip_w = get_f(params, "tip_w").unwrap_or(0.2);
            let angle = get_f(params, "angle").unwrap_or(0.5);
            let arrow_stroke = get_c(params, "stroke").or_else(|| get_c(params, "color")).unwrap_or(stroke);
            mo_core::geom::curved_double_arrow(
                get_v(params, "from").unwrap_or(Vec3::ZERO) + at,
                get_v(params, "to").unwrap_or(Vec3::new(1.0, 0.0, 0.0)) + at,
                angle,
                arrow_stroke,
                sw,
                tip_len,
                tip_w,
            )
        }
        "elbow" => {
            let dir1 = get_v(params, "dir1").unwrap_or(Vec3::X);
            let dir2 = get_v(params, "dir2").unwrap_or(Vec3::Y);
            let size = get_f(params, "size").or_else(|| get_f(params, "s")).unwrap_or(0.4);
            mo_core::geom::elbow(at, dir1, dir2, size, stroke, sw)
        }
        "axes" => {
            let x_range = get_v(params, "x").map(|v| (v.x, v.y, v.z)).unwrap_or((-6.0, 6.0, 1.0));
            let y_range = get_v(params, "y").map(|v| (v.x, v.y, v.z)).unwrap_or((-3.0, 3.0, 1.0));
            let width = get_f(params, "w").unwrap_or(10.0);
            let height = get_f(params, "h").unwrap_or(5.0);
            let axes_stroke = get_c(params, "stroke").or_else(|| get_c(params, "color")).unwrap_or(color::hex("#555566"));
            let axes_sw = get_f(params, "sw").unwrap_or(2.0);
            let axes = mo_core::Axes::new(x_range, y_range, width, height, at);
            let vg = axes.to_vgroup(axes_stroke, axes_sw);
            axes_map.insert(name.to_string(), axes);
            vg
        }
        "numberplane" | "number_plane" => {
            let x_range = get_v(params, "x").map(|v| (v.x, v.y, v.z)).unwrap_or((-6.0, 6.0, 1.0));
            let y_range = get_v(params, "y").map(|v| (v.x, v.y, v.z)).unwrap_or((-3.0, 3.0, 1.0));
            let width = get_f(params, "w").unwrap_or(10.0);
            let height = get_f(params, "h").unwrap_or(5.0);
            let axis_color = get_c(params, "axis_color").unwrap_or(color::WHITE);
            let grid_color = get_c(params, "grid_color").or_else(|| get_c(params, "stroke")).unwrap_or(color::hex("#334466"));
            let axes = mo_core::Axes::new(x_range, y_range, width, height, at);
            axes_map.insert(name.to_string(), axes);
            mo_core::axes::number_plane(x_range, y_range, width, height, at, axis_color, grid_color, sw, sw * 0.5)
        }
        "complexplane" | "complex_plane" => {
            let r_range = get_v(params, "real").or_else(|| get_v(params, "r")).map(|v| (v.x, v.y, v.z)).unwrap_or((-5.0, 5.0, 1.0));
            let i_range = get_v(params, "imag").or_else(|| get_v(params, "i")).map(|v| (v.x, v.y, v.z)).unwrap_or((-3.0, 3.0, 1.0));
            let width = get_f(params, "w").unwrap_or(10.0);
            let height = get_f(params, "h").unwrap_or(5.0);
            let axis_color = get_c(params, "axis_color").unwrap_or(color::WHITE);
            let grid_color = get_c(params, "grid_color").or_else(|| get_c(params, "stroke")).unwrap_or(color::hex("#334466"));
            let axes = mo_core::Axes::new(r_range, i_range, width, height, at);
            axes_map.insert(name.to_string(), axes);
            mo_core::axes::complex_plane(r_range, i_range, width, height, at, axis_color, grid_color, sw, sw * 0.5)
        }
        "polarplane" | "polar_plane" => {
            let r = get_f(params, "r").or_else(|| get_f(params, "radius")).unwrap_or(3.0);
            let circles = get_f(params, "circles").unwrap_or(4.0) as usize;
            let spokes = get_f(params, "spokes").unwrap_or(12.0) as usize;
            mo_core::axes::polar_plane(r, circles, spokes, at, stroke, sw)
        }
        "numberline" | "number_line" => {
            let range = get_v(params, "range").or_else(|| get_v(params, "x")).map(|v| (v.x, v.y, v.z)).unwrap_or((-5.0, 5.0, 1.0));
            let len = get_f(params, "len").or_else(|| get_f(params, "w")).unwrap_or(8.0);
            let tick = get_f(params, "tick_size").unwrap_or(0.2);
            mo_core::axes::number_line(range, len, at, tick, stroke, sw)
        }
        "func" | "plot" | "graph" => {
            let expr_str = get_s(params, "expr")
                .or_else(|| get_s(params, "f"))
                .ok_or_else(|| "func requires `expr=\"...\"` or `f=\"...\"`".to_string())?;
            let expr = mo_core::Expr::parse(&expr_str)
                .map_err(|e| format!("expression error in `{expr_str}`: {e}"))?;

            let axes = match get_s(params, "on") {
                Some(ax_name) => axes_map
                    .get(&ax_name)
                    .ok_or_else(|| format!("axes `{ax_name}` not found for plot"))?
                    .clone(),
                None => mo_core::Axes::default(),
            };

            let from_x = get_f(params, "from").unwrap_or(axes.x_range.0);
            let to_x = get_f(params, "to").unwrap_or(axes.x_range.1);
            let samples = get_f(params, "samples").unwrap_or(120.0) as usize;
            let plot_stroke = get_c(params, "stroke").or_else(|| get_c(params, "color")).unwrap_or(color::hex("#58C4DD"));
            let plot_sw = get_f(params, "sw").unwrap_or(2.0);

            axes.plot_expr(&expr, from_x, to_x, samples, plot_stroke, plot_sw)
        }
        "parametric" | "parametric_function" => {
            let fx_str = get_s(params, "fx").or_else(|| get_s(params, "expr_x")).ok_or_else(|| "parametric requires `fx=\"...\"`".to_string())?;
            let fy_str = get_s(params, "fy").or_else(|| get_s(params, "expr_y")).ok_or_else(|| "parametric requires `fy=\"...\"`".to_string())?;
            let expr_x = mo_core::Expr::parse(&fx_str).map_err(|e| format!("parametric fx error: {e}"))?;
            let expr_y = mo_core::Expr::parse(&fy_str).map_err(|e| format!("parametric fy error: {e}"))?;
            let axes = match get_s(params, "on") {
                Some(ax_name) => axes_map.get(&ax_name).ok_or_else(|| format!("axes `{ax_name}` not found"))?.clone(),
                None => mo_core::Axes::default(),
            };
            let from_t = get_f(params, "from").or_else(|| get_f(params, "t_min")).unwrap_or(0.0);
            let to_t = get_f(params, "to").or_else(|| get_f(params, "t_max")).unwrap_or(std::f32::consts::TAU);
            let samples = get_f(params, "samples").unwrap_or(120.0) as usize;
            axes.plot_parametric(|t| expr_x.eval(t), |t| expr_y.eval(t), from_t, to_t, samples, stroke, sw)
        }
        "riemann" | "riemann_rectangles" => {
            let expr_str = get_s(params, "expr").or_else(|| get_s(params, "f")).ok_or_else(|| "riemann requires `expr=\"...\"`".to_string())?;
            let expr = mo_core::Expr::parse(&expr_str).map_err(|e| format!("riemann expression error: {e}"))?;
            let axes = match get_s(params, "on") {
                Some(ax_name) => axes_map.get(&ax_name).ok_or_else(|| format!("axes `{ax_name}` not found"))?.clone(),
                None => mo_core::Axes::default(),
            };
            let from_x = get_f(params, "from").unwrap_or(axes.x_range.0);
            let to_x = get_f(params, "to").unwrap_or(axes.x_range.1);
            let n_bars = get_f(params, "n").or_else(|| get_f(params, "bars")).unwrap_or(10.0) as usize;
            let kind_str = get_s(params, "kind").unwrap_or_else(|| "mid".into());
            let kind = match kind_str.as_str() {
                "left" => mo_core::axes::RiemannKind::Left,
                "right" => mo_core::axes::RiemannKind::Right,
                _ => mo_core::axes::RiemannKind::Midpoint,
            };
            mo_core::axes::riemann_rectangles(&axes, |x| expr.eval(x), from_x, to_x, n_bars, kind, fill, stroke, sw)
        }
        "secant" | "secant_line" => {
            let expr_str = get_s(params, "expr").or_else(|| get_s(params, "f")).ok_or_else(|| "secant requires `expr=\"...\"`".to_string())?;
            let expr = mo_core::Expr::parse(&expr_str).map_err(|e| format!("secant expression error: {e}"))?;
            let axes = match get_s(params, "on") {
                Some(ax_name) => axes_map.get(&ax_name).ok_or_else(|| format!("axes `{ax_name}` not found"))?.clone(),
                None => mo_core::Axes::default(),
            };
            let x1 = get_f(params, "x1").unwrap_or(0.0);
            let x2 = get_f(params, "x2").unwrap_or(1.0);
            let len = get_f(params, "len").or_else(|| get_f(params, "length")).unwrap_or(4.0);
            mo_core::axes::secant_line(&axes, |x| expr.eval(x), x1, x2, len, stroke, sw)
        }
        "area" | "integral_area" => {
            let expr_str = get_s(params, "expr").or_else(|| get_s(params, "f")).ok_or_else(|| "area requires `expr=\"...\"`".to_string())?;
            let expr = mo_core::Expr::parse(&expr_str).map_err(|e| format!("area expression error: {e}"))?;
            let axes = match get_s(params, "on") {
                Some(ax_name) => axes_map.get(&ax_name).ok_or_else(|| format!("axes `{ax_name}` not found"))?.clone(),
                None => mo_core::Axes::default(),
            };
            let from_x = get_f(params, "from").unwrap_or(axes.x_range.0);
            let to_x = get_f(params, "to").unwrap_or(axes.x_range.1);
            let samples = get_f(params, "samples").unwrap_or(80.0) as usize;
            mo_core::axes::area_under_curve(&axes, |x| expr.eval(x), from_x, to_x, samples, fill)
        }
        "barchart" | "bar_chart" => {
            let vals_str = get_s(params, "values").or_else(|| get_s(params, "data")).unwrap_or_else(|| "1, 2, 3, 4".into());
            let values: Vec<f32> = vals_str
                .split(',')
                .filter_map(|s| s.trim().parse::<f32>().ok())
                .collect();
            let width = get_f(params, "w").unwrap_or(6.0);
            let height = get_f(params, "h").unwrap_or(3.0);
            mo_core::axes::bar_chart(&values, width, height, at, fill, stroke, sw)
        }
        "vector_field" | "vecfield" => {
            let fx_str = get_s(params, "fx").or_else(|| get_s(params, "expr_x")).unwrap_or_else(|| "-y".to_string());
            let fy_str = get_s(params, "fy").or_else(|| get_s(params, "expr_y")).unwrap_or_else(|| "x".to_string());
            let expr_x = mo_core::Expr::parse(&fx_str).map_err(|e| format!("vecfield fx error: {e}"))?;
            let expr_y = mo_core::Expr::parse(&fy_str).map_err(|e| format!("vecfield fy error: {e}"))?;

            let x_range = get_v(params, "x").map(|v| (v.x, v.y, v.z)).unwrap_or((-4.0, 4.0, 0.8));
            let y_range = get_v(params, "y").map(|v| (v.x, v.y, v.z)).unwrap_or((-3.0, 3.0, 0.8));
            let max_arrow_len = get_f(params, "max_len").unwrap_or(0.6);
            let vf_stroke = get_c(params, "stroke").or_else(|| get_c(params, "color")).unwrap_or(color::hex("#58C4DD"));
            let vf_sw = get_f(params, "sw").unwrap_or(2.0);

            let mut g = mo_core::math::vector_field_2d(
                |x, y| (expr_x.eval_2d(x, y), expr_y.eval_2d(x, y)),
                x_range,
                y_range,
                max_arrow_len,
                vf_stroke,
                vf_sw,
            );
            g.shift(at);
            g
        }
        "complex_grid" | "complexgrid" => {
            let mapping_str = get_s(params, "mapping").or_else(|| get_s(params, "f")).unwrap_or_else(|| "z".to_string());
            let r_range = get_v(params, "real").or_else(|| get_v(params, "r")).map(|v| (v.x, v.y, v.z)).unwrap_or((-3.0, 3.0, 1.0));
            let i_range = get_v(params, "imag").or_else(|| get_v(params, "i")).map(|v| (v.x, v.y, v.z)).unwrap_or((-3.0, 3.0, 1.0));
            let grid_stroke = get_c(params, "stroke").or_else(|| get_c(params, "color")).unwrap_or(color::hex("#4488EE"));
            let grid_sw = get_f(params, "sw").unwrap_or(2.0);
            let res = get_f(params, "res").unwrap_or(50.0) as usize;

            let grid = mo_core::math::ComplexGrid {
                real_range: r_range,
                imag_range: i_range,
                resolution: res,
            };

            let mut g = match mapping_str.as_str() {
                "z^2" | "sq" | "square" => grid.apply_mapping(|z| z * z, grid_stroke, grid_sw),
                "exp" | "exp(z)" => grid.apply_mapping(|z| z.exp(), grid_stroke, grid_sw),
                "inv" | "1/z" => grid.apply_mapping(|z| if z.norm_sqr() < 1e-6 { mo_core::num_complex::Complex32::new(f32::NAN, f32::NAN) } else { z.inv() }, grid_stroke, grid_sw),
                "sin" | "sin(z)" => grid.apply_mapping(|z| z.sin(), grid_stroke, grid_sw),
                _ => grid.to_vgroup(grid_stroke, grid_sw),
            };
            g.shift(at);
            g
        }
        "text" => {
            let text_str = get_s(params, "text").or_else(|| get_s(params, "content")).unwrap_or_default();
            let size = get_f(params, "size").unwrap_or(1.0);
            let text_fill = get_c(params, "fill").or_else(|| get_c(params, "color")).unwrap_or(color::WHITE);
            let text_stroke = get_c(params, "stroke").unwrap_or([0.0; 4]);
            let text_sw = get_f(params, "sw").unwrap_or(0.0);
            let font = get_s(params, "font");
            let mut g = mo_core::text::text_styled_with_font(&text_str, size, font.as_deref(), text_fill, text_stroke, text_sw);
            g.shift(at);
            g
        }
        "decimal" | "decimal_number" | "decimalnumber" => {
            let val = get_f(params, "val").or_else(|| get_f(params, "value")).unwrap_or(0.0) as f64;
            let places = get_f(params, "places").or_else(|| get_f(params, "decimals")).unwrap_or(2.0) as usize;
            let size = get_f(params, "size").unwrap_or(1.0);
            let font = get_s(params, "font");
            let mut g = mo_core::text::decimal_number(val, places, size, font.as_deref(), fill);
            g.shift(at);
            g
        }
        "matrix" => {
            let rows_str = get_s(params, "entries").or_else(|| get_s(params, "expr")).unwrap_or_else(|| "1, 0; 0, 1".into());
            let bracket = get_s(params, "bracket").unwrap_or_else(|| "[".into());
            let size = get_f(params, "size").unwrap_or(1.0);
            let font = get_s(params, "font");
            let rows: Vec<Vec<String>> = rows_str
                .split(';')
                .map(|r| r.split(',').map(|c| c.trim().to_string()).collect())
                .collect();
            let mut g = mo_core::text::matrix(&rows, size, &bracket, font.as_deref(), fill, stroke, sw);
            g.shift(at);
            g
        }
        "tex" | "latex" | "mathtex" | "math" => {
            let formula = get_s(params, "text")
                .or_else(|| get_s(params, "content"))
                .or_else(|| get_s(params, "math"))
                .or_else(|| get_s(params, "expr"))
                .ok_or_else(|| "tex requires `text=\"...\"` or `math=\"...\"`".to_string())?;
            let size = get_f(params, "size").unwrap_or(1.0);
            let tex_fill = get_c(params, "fill").or_else(|| get_c(params, "color")).unwrap_or(color::WHITE);
            let tex_stroke = get_c(params, "stroke").unwrap_or([0.0; 4]);
            let tex_sw = get_f(params, "sw").unwrap_or(0.0);

            let mut g = mo_core::render_math(&formula, size, tex_fill, tex_stroke, tex_sw)?;
            g.shift(at);
            g
        }
        "svg" => {
            let file_path = get_s(params, "file")
                .or_else(|| get_s(params, "src"))
                .ok_or_else(|| "svg requires `file=\"...\"`".to_string())?;
            let svg_content = std::fs::read_to_string(&file_path)
                .map_err(|e| format!("Failed to read SVG file `{file_path}`: {e}"))?;
            let size = get_f(params, "size").unwrap_or(2.0);
            let svg_fill = get_c(params, "fill").or_else(|| get_c(params, "color")).unwrap_or(color::WHITE);
            let svg_stroke = get_c(params, "stroke").unwrap_or([0.0; 4]);
            let svg_sw = get_f(params, "sw").unwrap_or(0.0);

            let mut g = mo_core::parse_svg(&svg_content, size, svg_fill, svg_stroke, svg_sw)?;
            g.shift(at);
            g
        }
        "axes3d" | "three_d_axes" => {
            let xr = get_pair(params, "x").unwrap_or((-4.0, 4.0));
            let yr = get_pair(params, "y").unwrap_or((-4.0, 4.0));
            let zr = get_pair(params, "z").unwrap_or((-3.0, 3.0));
            let sw = get_f(params, "sw").unwrap_or(2.5);
            mo_core::geom::axes_3d(xr, yr, zr, at, sw)
        }
        "cube" => {
            let side = get_f(params, "side").or_else(|| get_f(params, "s")).or_else(|| get_f(params, "size")).unwrap_or(2.0);
            let c = get_c(params, "stroke").or_else(|| get_c(params, "color")).unwrap_or(color::WHITE);
            let sw = get_f(params, "sw").unwrap_or(2.0);
            mo_core::geom::cube(side, at, c, sw)
        }
        "sphere" => {
            let radius = get_f(params, "r").or_else(|| get_f(params, "radius")).unwrap_or(1.5);
            let c = get_c(params, "stroke").or_else(|| get_c(params, "color")).unwrap_or(color::WHITE);
            let sw = get_f(params, "sw").unwrap_or(2.0);
            mo_core::geom::sphere(radius, at, c, sw)
        }
        "cylinder" => {
            let r = get_f(params, "r").or_else(|| get_f(params, "radius")).unwrap_or(1.0);
            let h = get_f(params, "h").or_else(|| get_f(params, "height")).unwrap_or(2.5);
            let c = get_c(params, "stroke").or_else(|| get_c(params, "color")).unwrap_or(color::WHITE);
            let sw = get_f(params, "sw").unwrap_or(2.0);
            mo_core::geom::cylinder(r, h, at, c, sw)
        }
        "cone" => {
            let r = get_f(params, "r").or_else(|| get_f(params, "radius")).unwrap_or(1.0);
            let h = get_f(params, "h").or_else(|| get_f(params, "height")).unwrap_or(2.5);
            let c = get_c(params, "stroke").or_else(|| get_c(params, "color")).unwrap_or(color::WHITE);
            let sw = get_f(params, "sw").unwrap_or(2.0);
            mo_core::geom::cone(r, h, at, c, sw)
        }
        "line3d" | "line_3d" => {
            let from = get_v(params, "from").unwrap_or(Vec3::ZERO) + at;
            let to = get_v(params, "to").unwrap_or(Vec3::new(1.0, 1.0, 1.0)) + at;
            let c = get_c(params, "stroke").or_else(|| get_c(params, "color")).unwrap_or(color::WHITE);
            let sw = get_f(params, "sw").unwrap_or(2.0);
            mo_core::geom::line_3d(from, to, c, sw)
        }
        "dot3d" | "dot_3d" => {
            let r = get_f(params, "r").or_else(|| get_f(params, "radius")).unwrap_or(0.2);
            let c = get_c(params, "stroke").or_else(|| get_c(params, "color")).unwrap_or(color::WHITE);
            let sw = get_f(params, "sw").unwrap_or(2.0);
            mo_core::geom::dot_3d(at, r, c, sw)
        }
        "surface3d" | "surface" => {
            let expr_str = get_s(params, "expr").or_else(|| get_s(params, "f")).unwrap_or_else(|| "sin(x)*cos(y)".into());
            let xr = get_pair(params, "x").unwrap_or((-3.0, 3.0));
            let yr = get_pair(params, "y").unwrap_or((-3.0, 3.0));
            let res_val = get_f(params, "res").or_else(|| get_f(params, "samples")).unwrap_or(16.0) as usize;
            let res_val = res_val.clamp(2, 400);
            let c = get_c(params, "stroke").or_else(|| get_c(params, "color")).unwrap_or(color::hex("#58C4DD"));
            let sw = get_f(params, "sw").unwrap_or(1.5);
            let expr = mo_core::expr::Expr::parse(&expr_str)?;
            let filled = params.iter().any(|(k, v)| {
                k == "fill" && matches!(v, Val::Ident(s) if s == "true")
            });
            let mut g = if filled {
                let wire_stride = get_f(params, "wire_stride").unwrap_or(0.0) as usize;
                mo_core::geom::surface_3d_filled_with_stride(|x, y| expr.eval_2d(x, y), xr, yr, (res_val, res_val), c, sw, wire_stride)
            } else {
                mo_core::geom::surface_3d(|x, y| expr.eval_2d(x, y), xr, yr, (res_val, res_val), c, sw)
            };
            g.shift(at);
            g
        }
        other => return Err(format!("unknown object kind `{other}`")),
    };

    if let Some(mat_vals) = get_mat2(params, "matrix").or_else(|| get_mat2(params, "mat")) {
        let m = mo_core::nalgebra::Matrix2::new(mat_vals[0], mat_vals[1], mat_vals[2], mat_vals[3]);
        mo_core::math::apply_matrix_2d(&mut obj, &m);
    }

    Ok(obj)
}

/// Where a flattened `Stmt::Play` came from. `file == None` means the play was
/// written in the master source itself; `Some(path, j)` means it is the j-th
/// play (flat order) of the imported asset at `path`. Order matches the flat
/// play sequence in the lowered timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayOrigin {
    pub file: Option<std::path::PathBuf>,
    pub local_play_idx: usize,
}

fn lower(raws: Vec<RawStmt>, base_dir: &std::path::Path) -> Result<(Timeline, Vec<PlayOrigin>), String> {
    // transform(c, c2) resolves against objects declared so far.
    let mut declared: std::collections::HashMap<String, VGroup> = Default::default();
    let mut axes_map: std::collections::HashMap<String, mo_core::Axes> = Default::default();
    let mut tl = Timeline::default();
    let mut origins: Vec<PlayOrigin> = Vec::new();

    let mut referenced_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    for raw in &raws {
        match raw {
            RawStmt::Play { args, .. } => {
                for arg in args {
                    referenced_ids.insert(arg.clone());
                }
            }
            RawStmt::Decl { params, .. } => {
                for (_, v) in params {
                    if let Val::Ident(id) = v {
                        referenced_ids.insert(id.clone());
                    }
                }
            }
            _ => {}
        }
    }

    for raw in raws {
        match raw {
            RawStmt::Decl { kind, name, params } => {
                let obj = build_object(&kind, &name, &params, &mut axes_map)?;
                declared.insert(name.clone(), obj.clone());
                tl.stmts.push(Stmt::Decl { id: name, obj: std::sync::Arc::new(obj) });
            }
            RawStmt::Play { name, args, run_time, rate } => {
                let run_time = run_time.unwrap_or(1.0);
                let rate_name = rate.unwrap_or_else(|| "smooth".into());
                if rate::by_name(&rate_name).is_none() {
                    return Err(format!("unknown easing `{rate_name}`"));
                }
                let anim = match (name.as_str(), args.as_slice()) {
                    ("fadeIn", [id]) => Animation::FadeIn { id: id.clone(), shift: None },
                    ("fadeOut", [id]) => Animation::FadeOut { id: id.clone(), shift: None },
                    ("showCreation", [id]) | ("write", [id]) => {
                        Animation::ShowCreation { id: id.clone() }
                    }
                    ("growFromCenter", [id]) => {
                        Animation::Grow { id: id.clone(), anchor: Vec3::ZERO }
                    }
                    ("rotate", [id]) => {
                        Animation::Rotate { id: id.clone(), angle: std::f32::consts::PI }
                    }
                    ("rotate", [id, a]) => {
                        let angle = a.parse::<f32>().unwrap_or(std::f32::consts::PI);
                        Animation::Rotate { id: id.clone(), angle }
                    }
                    ("spinInFromNothing", [id]) => {
                        Animation::SpinInFromNothing { id: id.clone() }
                    }
                    ("shrinkToCenter", [id]) => {
                        Animation::ShrinkToCenter { id: id.clone() }
                    }
                    ("uncreate", [id]) | ("unwrite", [id]) => {
                        Animation::Uncreate { id: id.clone() }
                    }
                    ("drawBorderThenFill", [id]) => {
                        Animation::DrawBorderThenFill { id: id.clone() }
                    }
                    ("circumscribe", [id]) => {
                        Animation::Circumscribe { id: id.clone(), color: Some(color::YELLOW) }
                    }
                    ("indicate", [id]) => {
                        Animation::Indicate {
                            id: id.clone(),
                            scale_factor: 1.25,
                            color: Some(color::YELLOW),
                        }
                    }
                    ("flash", [id]) => {
                        Animation::Flash {
                            id: id.clone(),
                            line_length: 0.5,
                            color: Some(color::YELLOW),
                        }
                    }
                    ("scale", [id, factor_str]) => {
                        let factor = factor_str.parse::<f32>().unwrap_or(2.0);
                        Animation::Scale { id: id.clone(), factor }
                    }
                    ("scale", [id]) => {
                        Animation::Scale { id: id.clone(), factor: 2.0 }
                    }
                    ("shift", [id, dx_str, dy_str]) => {
                        let dx = dx_str.parse::<f32>().unwrap_or(1.0);
                        let dy = dy_str.parse::<f32>().unwrap_or(0.0);
                        Animation::Shift { id: id.clone(), delta: Vec3::new(dx, dy, 0.0) }
                    }
                    ("shift", [id]) => {
                        Animation::Shift { id: id.clone(), delta: Vec3::new(1.0, 0.0, 0.0) }
                    }
                    ("wiggle", [id]) => {
                        Animation::Wiggle {
                            id: id.clone(),
                            angle: 20.0_f32.to_radians(),
                        }
                    }
                    ("transform", [id, to]) | ("replacementTransform", [id, to]) => {
                        let target = std::sync::Arc::new(
                            declared
                                .get(to)
                                .ok_or_else(|| format!("transform target `{to}` not declared yet"))?
                                .clone(),
                        );
                        Animation::Transform { id: id.clone(), target }
                    }
                    (other, _) => return Err(format!("unknown animation `{other}`")),
                };
                tl.stmts.push(Stmt::Play(TimedAnim::new(anim, run_time, &rate_name)));
                origins.push(PlayOrigin { file: None, local_play_idx: 0 });
            }
            RawStmt::CameraPlay { kind, params, run_time, rate } => {
                let run_time = run_time.unwrap_or(1.0);
                let rate_name = rate.unwrap_or_else(|| "smooth".into());
                if rate::by_name(&rate_name).is_none() {
                    return Err(format!("unknown easing `{rate_name}`"));
                }
                let get = |k: &str| params.iter().find(|(n, _)| n == k).map(|(_, v)| *v);
                let anim = match kind.as_str() {
                    "orbit" => {
                        let phi_deg = get("phi").unwrap_or(0.0);
                        let theta_deg = get("theta").unwrap_or(0.0);
                        Animation::CameraOrbit {
                            phi: phi_deg.to_radians(),
                            theta: theta_deg.to_radians(),
                        }
                    }
                    "zoom" => {
                        let factor = get("factor").unwrap_or(1.0);
                        if factor <= 0.0 {
                            return Err("camera.zoom factor must be positive".into());
                        }
                        Animation::CameraZoom { factor }
                    }
                    "move_to" => {
                        let x = get("x").unwrap_or(0.0);
                        let y = get("y").unwrap_or(0.0);
                        Animation::CameraMoveTo { x, y }
                    }
                    _ => return Err(format!("unknown camera animation `{kind}`")),
                };
                tl.stmts.push(Stmt::Play(TimedAnim::new(anim, run_time, &rate_name)));
                origins.push(PlayOrigin { file: None, local_play_idx: 0 });
            }
            RawStmt::Wait(d) => tl.stmts.push(Stmt::Wait(d)),
            RawStmt::Import { path, alias, at } => {
                // Resolve relative to the importing file first, then legacy
                // process-CWD locations.
                let mut candidates = vec![
                    base_dir.join(&path),
                    base_dir.join("assets").join(&path),
                ];
                for extra in [
                    PathBuf::from(&path),
                    PathBuf::from("assets").join(&path),
                    PathBuf::from("examples").join(&path),
                ] {
                    candidates.push(extra);
                }
                let resolved = candidates
                    .iter()
                    .find(|p| p.exists())
                    .ok_or_else(|| format!("Could not find imported asset `{path}`"))?;

                let content = std::fs::read_to_string(resolved).map_err(|e| {
                    format!("Failed to read imported asset `{}`: {e}", resolved.display())
                })?;

                let imported_tl = parse_inner(
                    &content,
                    resolved.parent().unwrap_or(base_dir),
                ).map_err(|diags| {
                    format!(
                        "Error in imported asset `{}`: {}",
                        resolved.display(),
                        diags
                            .iter()
                            .map(|d| d.message.clone())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })?;

                let shift_vec = at.unwrap_or(Vec3::ZERO);
                let mut local_play_idx: usize = 0;

                let has_internal_plays = imported_tl.0.stmts.iter().any(|s| matches!(s, Stmt::Play(_)));
                let alias_referenced = alias.as_ref().is_some_and(|a| referenced_ids.contains(a));

                // Register the imported asset alias as a first-class composite VGroup
                // so master files can animate it directly: play showCreation(alias), play transform(alias, to), etc.
                if let Some(a) = &alias {
                    let mut composite = VGroup::new();
                    for stmt in &imported_tl.0.stmts {
                        if let Stmt::Decl { obj, .. } = stmt {
                            let mut o = obj.as_ref().clone();
                            if shift_vec != Vec3::ZERO {
                                o.shift(shift_vec);
                            }
                            composite.append(o);
                        }
                    }
                    declared.insert(a.clone(), composite.clone());
                    if alias_referenced {
                        tl.stmts.push(Stmt::Decl { id: a.clone(), obj: std::sync::Arc::new(composite) });
                    }
                }

                for stmt in imported_tl.0.stmts {
                    match stmt {
                        Stmt::Decl { id, mut obj } => {
                            if shift_vec != Vec3::ZERO {
                                std::sync::Arc::make_mut(&mut obj).shift(shift_vec);
                            }
                            let target_name = if let Some(a) = &alias {
                                format!("{a}_{id}")
                            } else {
                                id.clone()
                            };
                            declared.insert(target_name.clone(), obj.as_ref().clone());
                            if (!alias_referenced && referenced_ids.contains(&target_name)) || has_internal_plays {
                                tl.stmts.push(Stmt::Decl { id: target_name, obj });
                            }
                        }
                        Stmt::Play(mut anim) => {
                            if shift_vec != Vec3::ZERO {
                                if let Animation::Transform { ref mut target, .. } = anim.anim {
                                    std::sync::Arc::make_mut(target).shift(shift_vec);
                                }
                            }
                            if let Some(a) = &alias {
                                let old_id = anim.anim.target_id().to_string();
                                let new_id = format!("{a}_{old_id}");
                                anim.anim = match anim.anim {
                                    Animation::ShowCreation { .. } => Animation::ShowCreation { id: new_id },
                                    Animation::FadeIn { shift, .. } => Animation::FadeIn { id: new_id, shift },
                                    Animation::FadeOut { shift, .. } => Animation::FadeOut { id: new_id, shift },
                                    Animation::Grow { anchor, .. } => Animation::Grow { id: new_id, anchor },
                                    Animation::Rotate { angle, .. } => Animation::Rotate { id: new_id, angle },
                                    Animation::SpinInFromNothing { .. } => Animation::SpinInFromNothing { id: new_id },
                                    Animation::ShrinkToCenter { .. } => Animation::ShrinkToCenter { id: new_id },
                                    Animation::Uncreate { .. } => Animation::Uncreate { id: new_id },
                                    Animation::DrawBorderThenFill { .. } => Animation::DrawBorderThenFill { id: new_id },
                                    Animation::Circumscribe { color, .. } => Animation::Circumscribe { id: new_id, color },
                                    Animation::Flash { line_length, color, .. } => Animation::Flash { id: new_id, line_length, color },
                                    Animation::Scale { factor, .. } => Animation::Scale { id: new_id, factor },
                                    Animation::Shift { delta, .. } => Animation::Shift { id: new_id, delta },
                                    Animation::Indicate { scale_factor, color, .. } => Animation::Indicate { id: new_id, scale_factor, color },
                                    Animation::Wiggle { angle, .. } => Animation::Wiggle { id: new_id, angle },
                                    Animation::Transform { target, .. } => Animation::Transform { id: new_id, target },
                                    // Camera animations have no object id to alias.
                                    cam @ (Animation::CameraOrbit { .. }
                                    | Animation::CameraZoom { .. }
                                    | Animation::CameraMoveTo { .. }) => cam,
                                };
                            }
                            tl.stmts.push(Stmt::Play(anim));
                            origins.push(PlayOrigin {
                                file: Some(resolved.clone()),
                                local_play_idx,
                            });
                            local_play_idx += 1;
                        }
                        Stmt::Wait(d) => {
                            tl.stmts.push(Stmt::Wait(d));
                        }
                    }
                }
            }
        }
    }
    Ok((tl, origins))
}

/// Parse a whole .moa file into a Timeline (with play origins discarded),
/// with line/col diagnostics. Imports resolve against the process CWD.
pub fn parse(src: &str) -> Result<Timeline, Vec<Diag>> {
    parse_inner(src, std::path::Path::new(".")).map(|(tl, _)| tl)
}

/// Parse a .moa file on disk; imports resolve relative to the file's directory.
pub fn parse_file(path: &std::path::Path) -> Result<(Timeline, Vec<PlayOrigin>), Vec<Diag>> {
    let src = std::fs::read_to_string(path).map_err(|e| {
        vec![Diag { line: 1, col: 1, message: format!("failed to read {}: {e}", path.display()) }]
    })?;
    parse_inner(&src, path.parent().unwrap_or(std::path::Path::new(".")))
}

fn parse_inner(src: &str, base_dir: &std::path::Path) -> Result<(Timeline, Vec<PlayOrigin>), Vec<Diag>> {
    let mut lexer = Lexer::new(src);
    let mut toks = Vec::new();
    loop {
        match lexer.next() {
            Ok(Some(t)) => toks.push(t),
            Ok(None) => break,
            Err(d) => return Err(vec![d]),
        }
    }
    let mut p = P { toks, idx: 0 };
    let mut raws = Vec::new();
    let mut diags = Vec::new();
    while p.peek().is_some() {
        let res = match p.peek() {
            None => break,
            Some(Tok::Ident(k)) if k == "play" => p.play_stmt().map(Some),
            Some(Tok::Ident(k)) if k == "wait" => p.wait_stmt().map(Some),
            Some(Tok::Ident(k)) if k == "import" => p.import_stmt().map(Some),
            Some(Tok::Ident(_)) => p.obj_decl().map(Some),
            Some(t) => Err(p.err_here(&format!("unexpected token `{t:?}`"))),
        };
        match res {
            Ok(Some(s)) => raws.push(s),
            Ok(None) => break,
            Err(d) => {
                diags.push(d);
                sync(&mut p);
            }
        }
    }
    if !diags.is_empty() {
        return Err(diags);
    }
    match lower(raws, base_dir) {
        Ok((tl, origins)) => Ok((tl, origins)),
        Err(m) => Err(vec![Diag { line: 1, col: 1, message: m }]),
    }
}

/// Like `parse`, but also returns per-play provenance (which source file each
/// flattened play came from). Origins align with the flat play sequence.
pub fn parse_with_origins(src: &str) -> Result<(Timeline, Vec<PlayOrigin>), Vec<Diag>> {
    parse_inner(src, std::path::Path::new("."))
}

/// Same as `parse_with_origins`, but imports resolve against `base_dir`
/// (e.g. the directory of the file whose text is being edited).
pub fn parse_with_origins_in(
    src: &str,
    base_dir: &std::path::Path,
) -> Result<(Timeline, Vec<PlayOrigin>), Vec<Diag>> {
    parse_inner(src, base_dir)
}

/// Skip to just past the next `;` so parsing can continue after an error.
fn sync(p: &mut P) {
    while let Some(t) = p.bump() {
        if t == Tok::Semi {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_demo() {
        let src = r#"
            circle c at (0,0) r=2, fill=#58C4DD22, stroke=blue, sw=4;
            rect r at (3,0) w=2, h=1, stroke=red;
            play showCreation(c) 2s smooth;
            wait 0.5s;
            play fadeIn(r) 1s;
            play transform(c, r) 1.5s linear;
        "#;
        let tl = parse(src).expect("parses");
        assert_eq!(tl.duration(), 5.0);
        assert_eq!(tl.stmts.len(), 6);
    }

    #[test]
    fn diag_on_bad_syntax() {
        let diags = parse("circle c at (0,0) r=2\nbadtoken %").unwrap_err();
        assert!(!diags.is_empty());
    }

    #[test]
    fn transform_resolves_target() {
        let src = "circle a r=1; circle b at (2,0); play transform(a, b) 1s linear;";
        let tl = parse(src).expect("ok");
        match &tl.stmts[2] {
            mo_core::scene::Stmt::Play(t) => assert_eq!(t.rate, "linear"),
            other => panic!("expected play, got {other:?}"),
        }
    }

    #[test]
    fn comments_ignored() {
        let src = "// hi\ncircle a r=1; // trailing\n/* block */ play showCreation(a) 1s;";
        assert_eq!(parse(src).unwrap().duration(), 1.0);
    }

    #[test]
    fn diag_position_accurate() {
        let diags = parse("circle a r=1;\nbroken % thing;").unwrap_err();
        assert_eq!(diags[0].line, 2);
    }

    #[test]
    fn parses_graphing_and_text() {
        let src = r#"
            axes ax at (0,0) x=(-6,6,1) y=(-3,3,1) w=10 h=5 stroke=#555566 sw=2;
            func wave on=ax expr="sin(x)" from=-6 to=6 stroke=#58C4DD sw=4;
            func wave2 on=ax expr="cos(x)" from=-6 to=6 stroke=#FC6255 sw=4;
            text title text="y = sin(x)" at (0, 3.2) size=0.8 stroke=white fill=white;
            dot pt at (0,0) r=0.1 color=yellow;
            arrow arr from (0,0) to (2,2) stroke=red;
            play showCreation(ax) 1.5s smooth;
            play write(title) 1.0s smooth;
            play showCreation(wave) 2.0s smooth;
            play transform(wave, wave2) 1.5s smooth;
        "#;
        let tl = parse(src).expect("parses sine wave graphing demo");
        assert_eq!(tl.duration(), 6.0);
        assert_eq!(tl.stmts.len(), 10);
    }

    #[test]
    fn parses_vector_field_and_matrix_transforms() {
        let src = r#"
            vecfield vf at (0,0) fx="-y" fy="x" x=(-2,2,0.8) y=(-2,2,0.8) color=#58C4DD;
            complexgrid cg at (0,0) real=(-2,2,1) imag=(-2,2,1) mapping="z^2" color=#FC6255;
            square sq at (0,0) side=2 mat=(1, 1, 0, 1) stroke=yellow;
            play showCreation(vf) 2.0s smooth;
            play fadeIn(cg) 1.5s;
            play showCreation(sq) 1.0s;
        "#;
        let tl = parse(src).expect("parses vector field and matrix transforms");
        assert_eq!(tl.duration(), 4.5);
        assert_eq!(tl.stmts.len(), 6);
    }

    #[test]
    fn parses_3d_primitives() {
        let src = r#"
            axes3d a3d at (0,0,0) x=(-3,3) y=(-3,3) z=(-2,2) sw=2;
            cube cb at (1,1,1) side=2 color=#58C4DD sw=2;
            sphere sp at (-1,-1,0) r=1 color=#FC6255 sw=2;
            surface3d surf at (0,0,0) expr="x^2 - y^2" x=(-2,2) y=(-2,2) res=8 color=yellow sw=1;
            play showCreation(a3d) 1s;
            play spinInFromNothing(cb) 1s;
        "#;
        let tl = parse(src).expect("parses 3d primitives");
        assert_eq!(tl.duration(), 2.0);
        assert_eq!(tl.initial_objects().len(), 4);
    }

    #[test]
    fn parses_import_asset() {
        let temp_dir = std::env::temp_dir();
        let asset_file = temp_dir.join("test_imported_asset.moa");
        std::fs::write(&asset_file, "circle sub_c at (0,0) r=1.5 stroke=blue;").expect("write temp asset");

        let src = format!(r#"
            import "{}" as my_asset at (2, 3);
            play showCreation(my_asset_sub_c) 1s;
        "#, asset_file.to_str().unwrap());

        let tl = parse(&src).expect("parses import statement");
        assert!(tl.initial_objects().contains_key("my_asset_sub_c"));
        let obj = &tl.initial_objects()["my_asset_sub_c"];
        let c = obj.center();
        assert!((c.x - 2.0).abs() < 1e-3);
        assert!((c.y - 3.0).abs() < 1e-3);

        let _ = std::fs::remove_file(asset_file);
    }

    #[test]
    fn parses_import_composite_asset_animated_directly() {
        let temp_dir = std::env::temp_dir();
        let asset_file = temp_dir.join("test_composite_asset.moa");
        std::fs::write(&asset_file, "circle c1 at (-1, 0) r=1;\nsquare s1 at (1, 0) side=1;").expect("write temp asset");

        let src = format!(r#"
            import "{}" as my_pack at (0, 2);
            play showCreation(my_pack) 1.5s smooth;
            play fadeOut(my_pack) 1.0s;
        "#, asset_file.to_str().unwrap());

        let tl = parse(&src).expect("parses direct composite asset animation");
        assert!(tl.initial_objects().contains_key("my_pack"));
        assert!(!tl.initial_objects().contains_key("my_pack_c1"), "sub-objects should not be separate static initial objects when alias is played");
        assert_eq!(tl.duration(), 2.5);

        // At t = 1.5s (end of showCreation), only `my_pack` is visible:
        let state_mid = mo_core::scene::Player::state_at(&tl, 1.5);
        assert!(state_mid.contains_key("my_pack"));
        assert_eq!(state_mid.len(), 1);

        // At t = 2.5s (end of fadeOut), nothing remains visible:
        let state_end = mo_core::scene::Player::state_at(&tl, 2.5);
        assert!(state_end.is_empty(), "all objects should be gone after fadeOut");

        let _ = std::fs::remove_file(asset_file);
    }

    #[test]
    fn import_origins_align_with_flat_play_order() {
        let temp_dir = std::env::temp_dir();
        let asset_file = temp_dir.join("test_origin_asset.moa");
        std::fs::write(
            &asset_file,
            "circle sub_c at (0,0) r=1.5;\nplay showCreation(sub_c) 1s;\nplay fadeIn(sub_c) 0.5s;",
        )
        .expect("write temp asset");

        let src = format!(
            "import \"{}\" as a1;\ncircle m r=1;\nplay showCreation(m) 2s;\nplay showCreation(a1_sub_c) 1s;",
            asset_file.to_str().unwrap()
        );
        let (tl, origins) = parse_with_origins(&src).expect("parses");
        assert_eq!(tl.duration(), 4.5);

        // flat play order: imported showCreation, imported fadeIn, master showCreation(m), master showCreation(a1_sub_c)
        assert_eq!(origins.len(), 4);
        assert_eq!(origins[0].file.as_deref(), Some(asset_file.as_path()));
        assert_eq!(origins[0].local_play_idx, 0);
        assert_eq!(origins[1].local_play_idx, 1);
        assert_eq!(origins[2].file, None);
        assert_eq!(origins[3].file, None);

        let _ = std::fs::remove_file(asset_file);
    }

    #[test]
    fn parse_file_resolves_imports_relative_to_file() {
        // Simulates a project folder: proj/master.moa imports proj/assets/thing.moa
        let proj = std::env::temp_dir().join("moa_parse_file_test");
        let _ = std::fs::remove_dir_all(&proj);
        std::fs::create_dir_all(proj.join("assets")).expect("mkdir");
        std::fs::write(proj.join("assets/thing.moa"), "circle c r=1;\nplay showCreation(c) 1s;\n").expect("write asset");
        std::fs::write(proj.join("master.moa"), "import \"assets/thing.moa\" as thing;\nplay showCreation(thing_c) 1s;\n").expect("write master");

        let (tl, origins) = parse_file(&proj.join("master.moa")).expect("parses project master");
        assert!(tl.duration() > 0.0);
        assert_eq!(origins.len(), 2);
        assert!(origins[0].file.as_deref().unwrap().ends_with("thing.moa"));

        let _ = std::fs::remove_dir_all(&proj);
    }

    #[test]
    fn unplayed_imported_assets_do_not_appear_at_t0_or_freeze() {
        let proj = std::env::temp_dir().join("moa_unplayed_assets_test");
        let _ = std::fs::remove_dir_all(&proj);
        std::fs::create_dir_all(proj.join("assets")).expect("mkdir");
        std::fs::write(proj.join("assets/scene1.moa"), "circle c1 r=1;\n").expect("write scene1");
        std::fs::write(proj.join("assets/scene2.moa"), "square s2 side=2;\n").expect("write scene2");
        std::fs::write(proj.join("assets/scene3.moa"), "text t3 text=\"hi\";\n").expect("write scene3");

        // master imports scene1, scene2, scene3, but ONLY plays scene1 at t=0 and scene2 at t=4:
        let master_src = r#"
            import "assets/scene1.moa" as s1 at (0, 0);
            import "assets/scene2.moa" as s2 at (0, 0);
            import "assets/scene3.moa" as s3 at (0, 0);
            play showCreation(s1) 2s;
            wait 2s;
            play showCreation(s2) 2s;
        "#;
        std::fs::write(proj.join("master.moa"), master_src).expect("write master");

        let (tl, _) = parse_file(&proj.join("master.moa")).expect("parses master");

        // At t = 0s, only s1 is playing/visible. Neither s2 nor unplayed s3 should appear!
        let state_t0 = mo_core::scene::Player::state_at(&tl, 0.0);
        assert!(state_t0.contains_key("s1"), "s1 should be visible at t=0");
        assert!(!state_t0.contains_key("s2"), "s2 should NOT appear at t=0");
        assert!(!state_t0.contains_key("s3"), "unplayed s3 should NEVER appear at t=0");
        assert!(!state_t0.contains_key("s1_c1"), "sub-objects should not be duplicated when alias is played");
        assert!(!state_t0.contains_key("s2_s2"), "sub-objects of s2 should not appear at t=0");
        assert!(!state_t0.contains_key("s3_t3"), "sub-objects of unplayed s3 should not appear");

        // At t = 4s, s2 starts and is visible:
        let state_t4 = mo_core::scene::Player::state_at(&tl, 4.0);
        assert!(state_t4.contains_key("s2"), "s2 should be visible at t=4");
        assert!(!state_t4.contains_key("s3"), "unplayed s3 must still NOT appear");

        let _ = std::fs::remove_dir_all(&proj);
    }
}

#[cfg(test)]
mod camera_tests {
    use super::*;

    #[test]
    fn parses_camera_anims() {
        let src = r#"
circle c at (0,0) r=1;
play camera.orbit(phi=60, theta=-40) 2.0s smooth;
play camera.zoom(factor=1.5) 1s;
play camera.move_to(x=2, y=-1) 1.5s linear;
"#;
        let tl = parse(src).expect("camera scene parses");
        let anims: Vec<&mo_core::anim::Animation> = tl
            .stmts
            .iter()
            .filter_map(|s| match s {
                mo_core::scene::Stmt::Play(a) => Some(&a.anim),
                _ => None,
            })
            .collect();
        assert_eq!(anims.len(), 3);
        assert!(matches!(
            anims[0],
            mo_core::anim::Animation::CameraOrbit { phi, theta }
            if (phi.to_degrees() - 60.0).abs() < 1e-3 && (theta.to_degrees() + 40.0).abs() < 1e-3
        ));
        assert!(matches!(
            anims[1],
            mo_core::anim::Animation::CameraZoom { factor } if (*factor - 1.5).abs() < 1e-4
        ));
        assert!(matches!(
            anims[2],
            mo_core::anim::Animation::CameraMoveTo { x, y } if *x == 2.0 && *y == -1.0
        ));
        assert!((tl.duration() - 4.5).abs() < 1e-3);
    }

    #[test]
    fn camera_anim_error_has_position() {
        let src = "play camera.orbit(phi=abc) 1s;\n";
        let diags = parse(src).unwrap_err();
        assert!(diags[0].message.contains("expected number"));
        assert_eq!(diags[0].line, 1);
    }

    #[test]
    fn camera_at_interpolates_and_finalizes() {
        let src = "play camera.orbit(phi=60, theta=0) 2s smooth;";
        let tl = parse(src).unwrap();
        let base = mo_core::camera::Camera::default();
        let mid = mo_core::scene::Player::camera_at(&tl, 1.0, &base);
        assert!(mid.phi > 0.1 && mid.phi < 60.0_f32.to_radians());
        let end = mo_core::scene::Player::camera_at(&tl, 5.0, &base);
        assert!((end.phi - 60.0_f32.to_radians()).abs() < 1e-3);
    }

    #[test]
    fn test_manim_builtins_library() {
        let src = r#"
            arc a r=2.0 start=0.0 end=3.14;
            rounded_rect rr w=3.0 h=1.5 r=0.2;
            ellipse el rx=2.0 ry=1.0;
            regular_polygon poly n=6 r=1.5;
            triangle tri side=2.0;
            annulus ann inner_r=1.0 outer_r=2.0;
            sector sec r=2.0 angle=1.57;
            annular_sector asec inner_r=1.0 outer_r=2.0 angle=1.57;
            star st n=5 outer_r=2.0 inner_r=0.8;
            brace br from=(-2.0, 0.0) to=(2.0, 0.0) depth=0.5;
            dashed_line dl from=(-3.0, 0.0) to=(3.0, 0.0) dash=0.2 space=0.1;
            tangent_line tl_line at=(0.0, 1.0) angle=0.0 len=2.0;
            double_arrow darr from=(-2.0, 0.0) to=(2.0, 0.0);
            curved_arrow carr from=(-2.0, 0.0) to=(2.0, 0.0) angle=0.8;
            elbow elb at=(0.0, 0.0) size=0.5;
            number_plane np x=(-4.0, 4.0, 1.0) y=(-3.0, 3.0, 1.0);
            complex_plane cp real=(-3.0, 3.0, 1.0) imag=(-2.0, 2.0, 1.0);
            polar_plane pp r=3.0 circles=3 spokes=8;
            number_line nl range=(-4.0, 4.0, 1.0) len=6.0;
            parametric pf fx="cos(t)" fy="sin(t)" from=0.0 to=6.28;
            riemann rm on="np" expr="x^2" from=-2.0 to=2.0 n=8;
            secant sl on="np" expr="x^2" x1=-1.0 x2=1.0;
            area ar on="np" expr="sin(x)" from=0.0 to=3.14;
            barchart bc values="10, 20, 35, 15" w=5.0 h=3.0;
            cylinder cyl r=1.5 h=3.0;
            cone cn r=1.5 h=3.0;
            line3d l3d from=(0.0, 0.0, 0.0) to=(1.0, 2.0, 3.0);
            dot3d d3d r=0.25;
            decimal num val=3.14159 places=3 size=1.2 font="mono";
            matrix mat entries="1, 2; 3, 4" bracket="[";
            text styled_txt text="MathOxide" size=1.5 font="serif";

            play drawBorderThenFill(a) 1.5s;
            play circumscribe(rr) 1.0s;
            play flash(st) 0.8s;
            play scale(poly, 1.5) 1.0s;
            play shift(tri, 1.0, 2.0) 1.0s;
            play uncreate(sec) 1.0s;
        "#;
        let tl = parse(src).expect("all manim built-in shapes, graphing, calculus, and anims must parse successfully");
        assert!(tl.stmts.len() >= 30);
    }
}
