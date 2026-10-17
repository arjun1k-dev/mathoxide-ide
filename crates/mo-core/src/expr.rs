//! Mathematical expression parser and evaluator for parametric and function graphing.

use ndarray::Array1;

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Num(f32),
    Var,  // variable 'x' or 't'
    VarY, // variable 'y'
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Pow(Box<Expr>, Box<Expr>),
    Neg(Box<Expr>),
    Sin(Box<Expr>),
    Cos(Box<Expr>),
    Tan(Box<Expr>),
    Sqrt(Box<Expr>),
    Abs(Box<Expr>),
    Exp(Box<Expr>),
    Ln(Box<Expr>),
    Erf(Box<Expr>),
    Gamma(Box<Expr>),
    Sinc(Box<Expr>),
}

impl Expr {
    pub fn eval_2d(&self, x: f32, y: f32) -> f32 {
        match self {
            Expr::Num(n) => *n,
            Expr::Var => x,
            Expr::VarY => y,
            Expr::Add(a, b) => a.eval_2d(x, y) + b.eval_2d(x, y),
            Expr::Sub(a, b) => a.eval_2d(x, y) - b.eval_2d(x, y),
            Expr::Mul(a, b) => a.eval_2d(x, y) * b.eval_2d(x, y),
            Expr::Div(a, b) => {
                let denom = b.eval_2d(x, y);
                if denom.abs() < 1e-9 {
                    0.0
                } else {
                    a.eval_2d(x, y) / denom
                }
            }
            Expr::Pow(a, b) => a.eval_2d(x, y).powf(b.eval_2d(x, y)),
            Expr::Neg(a) => -a.eval_2d(x, y),
            Expr::Sin(a) => a.eval_2d(x, y).sin(),
            Expr::Cos(a) => a.eval_2d(x, y).cos(),
            Expr::Tan(a) => a.eval_2d(x, y).tan(),
            Expr::Sqrt(a) => a.eval_2d(x, y).max(0.0).sqrt(),
            Expr::Abs(a) => a.eval_2d(x, y).abs(),
            Expr::Exp(a) => a.eval_2d(x, y).exp(),
            Expr::Ln(a) => a.eval_2d(x, y).max(1e-9).ln(),
            Expr::Erf(a) => statrs::function::erf::erf(a.eval_2d(x, y) as f64) as f32,
            Expr::Gamma(a) => statrs::function::gamma::gamma(a.eval_2d(x, y) as f64) as f32,
            Expr::Sinc(a) => {
                let v = a.eval_2d(x, y);
                if v.abs() < 1e-9 {
                    1.0
                } else {
                    v.sin() / v
                }
            }
        }
    }

    pub fn eval(&self, x: f32) -> f32 {
        self.eval_2d(x, 0.0)
    }

    /// Evaluates the expression across an ndarray 1D buffer in a vectorized fashion.
    pub fn eval_array(&self, x: &Array1<f32>) -> Array1<f32> {
        match self {
            Expr::Num(n) => Array1::from_elem(x.len(), *n),
            Expr::Var => x.clone(),
            Expr::VarY => Array1::zeros(x.len()),
            Expr::Add(a, b) => a.eval_array(x) + b.eval_array(x),
            Expr::Sub(a, b) => a.eval_array(x) - b.eval_array(x),
            Expr::Mul(a, b) => a.eval_array(x) * b.eval_array(x),
            Expr::Div(a, b) => {
                let numer = a.eval_array(x);
                let denom = b.eval_array(x);
                ndarray::Zip::from(&numer).and(&denom).map_collect(|&num, &den| {
                    if den.abs() < 1e-9 {
                        0.0
                    } else {
                        num / den
                    }
                })
            }
            Expr::Pow(a, b) => {
                let base = a.eval_array(x);
                let exp = b.eval_array(x);
                ndarray::Zip::from(&base).and(&exp).map_collect(|&bs, &ex| bs.powf(ex))
            }
            Expr::Neg(a) => -a.eval_array(x),
            Expr::Sin(a) => a.eval_array(x).mapv(f32::sin),
            Expr::Cos(a) => a.eval_array(x).mapv(f32::cos),
            Expr::Tan(a) => a.eval_array(x).mapv(f32::tan),
            Expr::Sqrt(a) => a.eval_array(x).mapv(|v| v.max(0.0).sqrt()),
            Expr::Abs(a) => a.eval_array(x).mapv(f32::abs),
            Expr::Exp(a) => a.eval_array(x).mapv(f32::exp),
            Expr::Ln(a) => a.eval_array(x).mapv(|v| v.max(1e-9).ln()),
            Expr::Erf(a) => a.eval_array(x).mapv(|v| statrs::function::erf::erf(v as f64) as f32),
            Expr::Gamma(a) => a.eval_array(x).mapv(|v| statrs::function::gamma::gamma(v as f64) as f32),
            Expr::Sinc(a) => a.eval_array(x).mapv(|v| if v.abs() < 1e-9 { 1.0 } else { v.sin() / v }),
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        let tokens = tokenize(s)?;
        let mut pos = 0;
        let expr = parse_add_sub(&tokens, &mut pos)?;
        if pos < tokens.len() {
            return Err(format!("unexpected extra token at end: {:?}", tokens[pos]));
        }
        Ok(expr)
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Num(f32),
    VarX,
    VarY,
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
}

fn tokenize(s: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = s.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    let n = chars.len();

    while i < n {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        match c {
            '+' => { tokens.push(Token::Plus); i += 1; }
            '-' => { tokens.push(Token::Minus); i += 1; }
            '*' => { tokens.push(Token::Star); i += 1; }
            '/' => { tokens.push(Token::Slash); i += 1; }
            '^' => { tokens.push(Token::Caret); i += 1; }
            '(' => { tokens.push(Token::LParen); i += 1; }
            ')' => { tokens.push(Token::RParen); i += 1; }
            _ if c.is_ascii_digit() || c == '.' => {
                let start = i;
                while i < n && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let num_str: String = chars[start..i].iter().collect();
                let val: f32 = num_str.parse().map_err(|e| format!("invalid number: {e}"))?;
                tokens.push(Token::Num(val));
            }
            _ if c.is_alphabetic() => {
                let start = i;
                while i < n && chars[i].is_alphabetic() {
                    i += 1;
                }
                let name: String = chars[start..i].iter().collect();
                match name.as_str() {
                    "x" | "t" | "X" | "T" => tokens.push(Token::VarX),
                    "y" | "Y" => tokens.push(Token::VarY),
                    "pi" | "PI" => tokens.push(Token::Num(std::f32::consts::PI)),
                    "tau" | "TAU" => tokens.push(Token::Num(std::f32::consts::TAU)),
                    "e" | "E" => tokens.push(Token::Num(std::f32::consts::E)),
                    _ => tokens.push(Token::Ident(name.to_lowercase())),
                }
            }
            _ => return Err(format!("unexpected character `{c}` in expression")),
        }
    }
    Ok(tokens)
}

fn parse_add_sub(tokens: &[Token], pos: &mut usize) -> Result<Expr, String> {
    let mut left = parse_mul_div(tokens, pos)?;
    while *pos < tokens.len() {
        match tokens[*pos] {
            Token::Plus => {
                *pos += 1;
                let right = parse_mul_div(tokens, pos)?;
                left = Expr::Add(Box::new(left), Box::new(right));
            }
            Token::Minus => {
                *pos += 1;
                let right = parse_mul_div(tokens, pos)?;
                left = Expr::Sub(Box::new(left), Box::new(right));
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_mul_div(tokens: &[Token], pos: &mut usize) -> Result<Expr, String> {
    let mut left = parse_power(tokens, pos)?;
    while *pos < tokens.len() {
        match tokens[*pos] {
            Token::Star => {
                *pos += 1;
                let right = parse_power(tokens, pos)?;
                left = Expr::Mul(Box::new(left), Box::new(right));
            }
            Token::Slash => {
                *pos += 1;
                let right = parse_power(tokens, pos)?;
                left = Expr::Div(Box::new(left), Box::new(right));
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_power(tokens: &[Token], pos: &mut usize) -> Result<Expr, String> {
    let left = parse_unary(tokens, pos)?;
    if *pos < tokens.len() && tokens[*pos] == Token::Caret {
        *pos += 1;
        let right = parse_power(tokens, pos)?; // right associative
        return Ok(Expr::Pow(Box::new(left), Box::new(right)));
    }
    Ok(left)
}

fn parse_unary(tokens: &[Token], pos: &mut usize) -> Result<Expr, String> {
    if *pos >= tokens.len() {
        return Err("unexpected end of expression".into());
    }
    if tokens[*pos] == Token::Minus {
        *pos += 1;
        let inner = parse_unary(tokens, pos)?;
        return Ok(Expr::Neg(Box::new(inner)));
    }
    if tokens[*pos] == Token::Plus {
        *pos += 1;
        return parse_unary(tokens, pos);
    }
    parse_primary(tokens, pos)
}

fn parse_primary(tokens: &[Token], pos: &mut usize) -> Result<Expr, String> {
    if *pos >= tokens.len() {
        return Err("unexpected end of expression".into());
    }
    match &tokens[*pos] {
        Token::Num(n) => {
            let val = *n;
            *pos += 1;
            Ok(Expr::Num(val))
        }
        Token::VarX => {
            *pos += 1;
            Ok(Expr::Var)
        }
        Token::VarY => {
            *pos += 1;
            Ok(Expr::VarY)
        }
        Token::Ident(fn_name) => {
            let name = fn_name.clone();
            *pos += 1;
            if *pos >= tokens.len() || tokens[*pos] != Token::LParen {
                return Err(format!("expected `(` after function `{name}`"));
            }
            *pos += 1;
            let arg = parse_add_sub(tokens, pos)?;
            if *pos >= tokens.len() || tokens[*pos] != Token::RParen {
                return Err(format!("expected `)` after argument for `{name}`"));
            }
            *pos += 1;
            match name.as_str() {
                "sin" => Ok(Expr::Sin(Box::new(arg))),
                "cos" => Ok(Expr::Cos(Box::new(arg))),
                "tan" => Ok(Expr::Tan(Box::new(arg))),
                "sqrt" => Ok(Expr::Sqrt(Box::new(arg))),
                "abs" => Ok(Expr::Abs(Box::new(arg))),
                "exp" => Ok(Expr::Exp(Box::new(arg))),
                "ln" | "log" => Ok(Expr::Ln(Box::new(arg))),
                "erf" => Ok(Expr::Erf(Box::new(arg))),
                "gamma" => Ok(Expr::Gamma(Box::new(arg))),
                "sinc" => Ok(Expr::Sinc(Box::new(arg))),
                _ => Err(format!("unknown function `{name}`")),
            }
        }
        Token::LParen => {
            *pos += 1;
            let inner = parse_add_sub(tokens, pos)?;
            if *pos >= tokens.len() || tokens[*pos] != Token::RParen {
                return Err("missing closing parenthesis `)`".into());
            }
            *pos += 1;
            Ok(inner)
        }
        other => Err(format!("unexpected token `{:?}` in expression", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_simple() {
        let e = Expr::parse("sin(x)").unwrap();
        assert!((e.eval(0.0) - 0.0).abs() < 1e-6);
        assert!((e.eval(std::f32::consts::FRAC_PI_2) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_eval_compound() {
        let e = Expr::parse("2 * cos(x) + x^2").unwrap();
        // At x = 0: 2 * 1 + 0 = 2
        assert!((e.eval(0.0) - 2.0).abs() < 1e-6);
        // At x = PI: 2 * (-1) + PI^2 = -2 + 9.8696...
        let expected = -2.0 + std::f32::consts::PI * std::f32::consts::PI;
        assert!((e.eval(std::f32::consts::PI) - expected).abs() < 1e-4);
    }

    #[test]
    fn test_eval_vectorized_ndarray() {
        let e = Expr::parse("sin(x) + 1").unwrap();
        let xs = ndarray::Array1::from_vec(vec![0.0, std::f32::consts::FRAC_PI_2, std::f32::consts::PI]);
        let ys = e.eval_array(&xs);
        assert!((ys[0] - 1.0).abs() < 1e-6);
        assert!((ys[1] - 2.0).abs() < 1e-6);
        assert!((ys[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_eval_2d_and_special() {
        let e_2d = Expr::parse("-y + x").unwrap();
        assert_eq!(e_2d.eval_2d(3.0, 5.0), -2.0);

        let e_sinc = Expr::parse("sinc(x)").unwrap();
        assert!((e_sinc.eval(0.0) - 1.0).abs() < 1e-6);

        let e_gamma = Expr::parse("gamma(x)").unwrap();
        assert!((e_gamma.eval(5.0) - 24.0).abs() < 1e-4);
    }
}
