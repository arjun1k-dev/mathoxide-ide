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
    Hash(String), // hex digits after '#'
    LParen,
    RParen,
    Comma,
    Semi,
    Eq,
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

#[derive(Debug)]
enum Val {
    F(f32),
    V(Vec3),
    C([f32; 4]),
}

#[derive(Debug)]
enum RawStmt {
    Decl { kind: String, name: String, params: Vec<(String, Val)> },
    Play { name: String, args: Vec<String>, run_time: Option<f32>, rate: Option<String> },
    Wait(f32),
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
                                message: format!("bad vector literal, got {other:?}"),
                            })
                        }
                    }
                }
                Ok(Val::V(match xs.len() {
                    2 => Vec3::new(xs[0], xs[1], 0.0),
                    3 => Vec3::new(xs[0], xs[1], xs[2]),
                    n => {
                        return Err(Diag {
                            line: l,
                            col: c,
                            message: format!("vector with {n} components, need 2 or 3"),
                        })
                    }
                }))
            }
            Some(Tok::Hash(s)) => color::parse(&format!("#{s}"))
                .map(Val::C)
                .ok_or_else(|| Diag { line: l, col: c, message: format!("bad color `#{s}`") }),
            Some(Tok::Ident(s)) => color::parse(&s)
                .map(Val::C)
                .ok_or_else(|| Diag { line: l, col: c, message: format!("unknown color `{s}`") }),
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

fn get_c(params: &[(String, Val)], k: &str) -> Option<[f32; 4]> {
    params.iter().find(|(key, _)| key == k).and_then(|(_, v)| match v {
        Val::C(c) => Some(*c),
        _ => None,
    })
}

fn build_object(kind: &str, params: &[(String, Val)]) -> Result<VGroup, String> {
    let fill = get_c(params, "fill").unwrap_or([0.0; 4]);
    let stroke = get_c(params, "stroke").unwrap_or(color::BLUE);
    let sw = get_f(params, "sw").unwrap_or(4.0);
    let at = get_v(params, "at").unwrap_or(Vec3::ZERO);
    Ok(match kind {
        "circle" => mo_core::geom::circle(get_f(params, "r").unwrap_or(1.0), at, fill, stroke, sw),
        "rect" => mo_core::geom::rect(
            get_f(params, "w").unwrap_or(2.0),
            get_f(params, "h").unwrap_or(1.0),
            at,
            fill,
            stroke,
            sw,
        ),
        "line" => mo_core::geom::line(
            get_v(params, "from").unwrap_or(Vec3::ZERO),
            get_v(params, "to").unwrap_or(Vec3::new(1.0, 0.0, 0.0)),
            stroke,
            sw,
        ),
        "text" => mo_core::text::text("", get_f(params, "size").unwrap_or(1.0)),
        other => return Err(format!("unknown object kind `{other}`")),
    })
}

fn lower(raws: Vec<RawStmt>) -> Result<Timeline, String> {
    // transform(c, c2) resolves against objects declared so far.
    let mut declared: std::collections::HashMap<String, VGroup> = Default::default();
    let mut tl = Timeline { stmts: Vec::new() };
    for raw in raws {
        match raw {
            RawStmt::Decl { kind, name, params } => {
                let obj = build_object(&kind, &params)?;
                declared.insert(name.clone(), obj.clone());
                tl.stmts.push(Stmt::Decl { id: name, obj });
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
                    ("transform", [id, to]) => {
                        let target = declared
                            .get(to)
                            .ok_or_else(|| format!("transform target `{to}` not declared yet"))?
                            .clone();
                        Animation::Transform { id: id.clone(), target }
                    }
                    (other, _) => return Err(format!("unknown animation `{other}`")),
                };
                tl.stmts.push(Stmt::Play(TimedAnim::new(anim, run_time, &rate_name)));
            }
            RawStmt::Wait(d) => tl.stmts.push(Stmt::Wait(d)),
        }
    }
    Ok(tl)
}

/// Parse a whole .moa file into a Timeline, with line/col diagnostics.
pub fn parse(src: &str) -> Result<Timeline, Vec<Diag>> {
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
    match lower(raws) {
        Ok(tl) => Ok(tl),
        Err(m) => Err(vec![Diag { line: 1, col: 1, message: m }]),
    }
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
}
