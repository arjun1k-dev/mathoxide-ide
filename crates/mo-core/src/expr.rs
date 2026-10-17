//! Mathematical expression parser and evaluator for parametric and function graphing.

#[derive(Clone, Debug)]
pub enum Expr {
    Num(f32),
    Var, // variable 'x' or 't'
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
}

impl Expr {
    pub fn eval(&self, x: f32) -> f32 {
        match self {
            Expr::Num(n) => *n,
            Expr::Var => x,
            Expr::Add(a, b) => a.eval(x) + b.eval(x),
            Expr::Sub(a, b) => a.eval(x) - b.eval(x),
            Expr::Mul(a, b) => a.eval(x) * b.eval(x),
            Expr::Div(a, b) => {
                let denom = b.eval(x);
                if denom.abs() < 1e-9 {
                    0.0
                } else {
                    a.eval(x) / denom
                }
            }
            Expr::Pow(a, b) => a.eval(x).powf(b.eval(x)),
            Expr::Neg(a) => -a.eval(x),
            Expr::Sin(a) => a.eval(x).sin(),
            Expr::Cos(a) => a.eval(x).cos(),
            Expr::Tan(a) => a.eval(x).tan(),
            Expr::Sqrt(a) => a.eval(x).max(0.0).sqrt(),
            Expr::Abs(a) => a.eval(x).abs(),
            Expr::Exp(a) => a.eval(x).exp(),
            Expr::Ln(a) => a.eval(x).max(1e-9).ln(),
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
    Var,
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
                    "x" | "t" | "X" | "T" => tokens.push(Token::Var),
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
        Token::Var => {
            *pos += 1;
            Ok(Expr::Var)
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
                "ln" => Ok(Expr::Ln(Box::new(arg))),
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
}
