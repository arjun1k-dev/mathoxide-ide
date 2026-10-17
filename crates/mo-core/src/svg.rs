//! SVG path parser and LaTeX / Typst mathematical typesetting engine.
//! Converts math equations and vector SVGs into quadratic Bézier VGroups.

use crate::vgroup::VGroup;
use glam::Vec3;
use roxmltree::Node;
use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use svgtypes::{SimplePathSegment, SimplifyingPathParser, Transform};

fn apply_transform(t: &Transform, x: f64, y: f64) -> (f64, f64) {
    (
        t.a * x + t.c * y + t.e,
        t.b * x + t.d * y + t.f,
    )
}

fn concat_transform(t1: &Transform, t2: &Transform) -> Transform {
    Transform {
        a: t1.a * t2.a + t1.c * t2.b,
        b: t1.b * t2.a + t1.d * t2.b,
        c: t1.a * t2.c + t1.c * t2.d,
        d: t1.b * t2.c + t1.d * t2.d,
        e: t1.a * t2.e + t1.c * t2.f + t1.e,
        f: t1.b * t2.e + t1.d * t2.f + t1.f,
    }
}

/// Parse an SVG path string `d="..."` into quadratic Bézier curves in `group`.
pub fn parse_svg_path_d(d: &str, transform: &Transform, group: &mut VGroup) {
    let mut current_pt = Vec3::ZERO;
    let mut start_pt = Vec3::ZERO;

    for seg_res in SimplifyingPathParser::from(d) {
        let Ok(seg) = seg_res else { continue };
        match seg {
            SimplePathSegment::MoveTo { x, y } => {
                let (tx, ty) = apply_transform(transform, x, y);
                let pt = Vec3::new(tx as f32, -ty as f32, 0.0);
                current_pt = pt;
                start_pt = pt;
                group.start_subpath(pt);
            }
            SimplePathSegment::LineTo { x, y } => {
                let (tx, ty) = apply_transform(transform, x, y);
                let pt = Vec3::new(tx as f32, -ty as f32, 0.0);
                let handle = (current_pt + pt) * 0.5;
                group.add_curve(handle, pt);
                current_pt = pt;
            }
            SimplePathSegment::Quadratic {
                x1,
                y1,
                x,
                y,
            } => {
                let (tx1, ty1) = apply_transform(transform, x1, y1);
                let (tx, ty) = apply_transform(transform, x, y);
                let handle = Vec3::new(tx1 as f32, -ty1 as f32, 0.0);
                let pt = Vec3::new(tx as f32, -ty as f32, 0.0);
                group.add_curve(handle, pt);
                current_pt = pt;
            }
            SimplePathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                let (tx1, ty1) = apply_transform(transform, x1, y1);
                let (tx2, ty2) = apply_transform(transform, x2, y2);
                let (tx, ty) = apply_transform(transform, x, y);

                let p0 = current_pt;
                let p1 = Vec3::new(tx1 as f32, -ty1 as f32, 0.0);
                let p2 = Vec3::new(tx2 as f32, -ty2 as f32, 0.0);
                let p3 = Vec3::new(tx as f32, -ty as f32, 0.0);

                // Approximate cubic Bézier with two quadratic segments (splitting at t=0.5)
                let q1 = p0 + (p1 - p0) * 0.75;
                let q2 = p3 + (p2 - p3) * 0.75;
                let mid = (q1 + q2) * 0.5;
                group.add_curve(q1, mid);
                group.add_curve(q2, p3);
                current_pt = p3;
            }
            SimplePathSegment::ClosePath => {
                group.close();
                current_pt = start_pt;
            }
        }
    }
}

/// Recursively traverses SVG DOM nodes, accumulating affine transforms and emitting curves.
fn traverse_svg_node<'a>(
    node: Node<'a, 'a>,
    parent_transform: &Transform,
    symbols: &HashMap<&str, Vec<&'a str>>,
    group: &mut VGroup,
) {
    let mut current_transform = *parent_transform;

    if let Some(t_str) = node.attribute("transform") {
        if let Ok(t) = t_str.parse::<Transform>() {
            current_transform = concat_transform(parent_transform, &t);
        }
    }

    match node.tag_name().name() {
        "path" => {
            // Skip page background rectangles
            let is_bg = node.attribute("fill").map(|f| f.eq_ignore_ascii_case("#ffffff") || f.eq_ignore_ascii_case("white")).unwrap_or(false)
                && node.parent().map(|p| p.tag_name().name() == "svg").unwrap_or(false);
            if !is_bg {
                if let Some(d) = node.attribute("d") {
                    parse_svg_path_d(d, &current_transform, group);
                }
            }
        }
        "use" => {
            let href = node.attributes().find(|a| a.name() == "href").map(|a| a.value());

            if let Some(mut target_id) = href {
                if target_id.starts_with('#') {
                    target_id = &target_id[1..];
                }

                let mut use_transform = current_transform;
                let x: f64 = node.attribute("x").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                let y: f64 = node.attribute("y").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                if x.abs() > 1e-6 || y.abs() > 1e-6 {
                    let mut tr = Transform::default();
                    tr.e = x;
                    tr.f = y;
                    use_transform = concat_transform(&current_transform, &tr);
                }

                if let Some(paths) = symbols.get(target_id) {
                    for d in paths {
                        parse_svg_path_d(d, &use_transform, group);
                    }
                }
            }
        }
        _ => {}
    }

    // Traverse children (except <defs> which are pre-indexed)
    if node.tag_name().name() != "defs" {
        for child in node.children() {
            traverse_svg_node(child, &current_transform, symbols, group);
        }
    }
}

/// Parse a raw SVG document string into a normalized VGroup.
pub fn parse_svg(svg_str: &str, size: f32, fill: [f32; 4], stroke: [f32; 4], stroke_width: f32) -> Result<VGroup, String> {
    let doc = roxmltree::Document::parse(svg_str).map_err(|e| format!("SVG XML parse error: {e}"))?;
    let mut symbols: HashMap<&str, Vec<&str>> = HashMap::new();

    // Index all symbols in defs
    for node in doc.descendants() {
        if node.tag_name().name() == "symbol" {
            if let Some(id) = node.attribute("id") {
                let mut paths = Vec::new();
                for path_node in node.descendants() {
                    if path_node.tag_name().name() == "path" {
                        if let Some(d) = path_node.attribute("d") {
                            paths.push(d);
                        }
                    }
                }
                symbols.insert(id, paths);
            }
        }
    }

    let mut group = VGroup::new();
    let initial_transform = Transform::default();
    traverse_svg_node(doc.root(), &initial_transform, &symbols, &mut group);

    if !group.points.is_empty() {
        let (min, max) = group.bounds();
        let center = Vec3::new((min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5, 0.0);
        group.shift(-center);

        let h = (max[1] - min[1]).max(1e-4);
        let scale = size / h;
        group.scale(scale, Vec3::ZERO);
    }

    group.set_fill(fill);
    group.set_stroke(stroke, stroke_width);
    Ok(group)
}

/// Convert standard LaTeX syntax into Typst math syntax.
pub fn latex_to_typst(latex: &str) -> String {
    let mut s = latex.trim().to_string();

    // Remove enclosing $ or $$ if present
    if s.starts_with('$') && s.ends_with('$') {
        s = s.trim_matches('$').trim().to_string();
    }

    // Common LaTeX replacements
    s = s.replace(r"\left(", "(")
         .replace(r"\right)", ")")
         .replace(r"\left[", "[")
         .replace(r"\right]", "]")
         .replace(r"\left\{", "{")
         .replace(r"\right\}", "}")
         .replace(r"\int", "integral")
         .replace(r"\sum", "sum")
         .replace(r"\prod", "product")
         .replace(r"\infty", " oo ")
         .replace(r"\alpha", " alpha ")
         .replace(r"\beta", " beta ")
         .replace(r"\gamma", " gamma ")
         .replace(r"\theta", " theta ")
         .replace(r"\pi", " pi ")
         .replace(r"\sigma", " sigma ")
         .replace(r"\omega", " omega ")
         .replace(r"\mu", " mu ")
         .replace(r"\lambda", " lambda ")
         .replace(r"\phi", " phi ")
         .replace(r"\cdot", "dot")
         .replace(r"\times", "times")
         .replace(r"\approx", "approx")
         .replace(r"\neq", "!=")
         .replace(r"\ne", "!=")
         .replace(r"\leq", "<=")
         .replace(r"\le", "<=")
         .replace(r"\geq", ">=")
         .replace(r"\ge", ">=")
         .replace(r"\to", "->")
         .replace(r"\rightarrow", "->")
         .replace(r"\sin", "sin")
         .replace(r"\cos", "cos")
         .replace(r"\tan", "tan")
         .replace(r"\ln", "ln")
         .replace(r"\log", "log")
         .replace(r"\exp", "exp")
         .replace(r"\,", " ")
         .replace(r"\;", " ")
         .replace(r"\quad", " ")
         .replace(r"dx", "dif x")
         .replace(r"dt", "dif t")
         .replace(r"dy", "dif y");

    // Replace \frac{a}{b} -> (a)/(b)
    while let Some(idx) = s.find(r"\frac{") {
        let after = &s[idx + 6..];
        if let Some(close1) = after.find('}') {
            let num = &after[..close1];
            let rest = &after[close1 + 1..];
            if rest.starts_with('{') {
                if let Some(close2) = rest[1..].find('}') {
                    let den = &rest[1..1 + close2];
                    let replacement = format!("(({num})/({den}))");
                    let total_len = 6 + close1 + 1 + 1 + close2 + 1;
                    s.replace_range(idx..idx + total_len, &replacement);
                    continue;
                }
            }
        }
        break;
    }

    // Replace \sqrt{a} or sqrt{a} -> sqrt(a)
    for prefix in &[r"\sqrt{", "sqrt{"] {
        while let Some(idx) = s.find(prefix) {
            let p_len = prefix.len();
            let after = &s[idx + p_len..];
            if let Some(close) = after.find('}') {
                let inner = &after[..close];
                let replacement = format!("sqrt({inner})");
                s.replace_range(idx..idx + p_len + close + 1, &replacement);
                continue;
            }
            break;
        }
    }

    // Replace ^{...} -> ^(...) and _{...} -> _(...)
    for (prefix, open, close) in &[("^{", "^(", ")"), ("_{", "_(", ")")] {
        while let Some(idx) = s.find(prefix) {
            let p_len = prefix.len();
            let mut depth = 1;
            let mut close_idx = None;
            for (i, c) in s[idx + p_len..].char_indices() {
                if c == '{' {
                    depth += 1;
                } else if c == '}' {
                    depth -= 1;
                    if depth == 0 {
                        close_idx = Some(idx + p_len + i);
                        break;
                    }
                }
            }
            if let Some(c_idx) = close_idx {
                s.replace_range(c_idx..c_idx + 1, close);
                s.replace_range(idx..idx + p_len, open);
            } else {
                break;
            }
        }
    }

    // Additional common math fallbacks
    s = s.replace(r"\partial", "diff")
         .replace(r"\nabla", "nabla")
         .replace("int_", "integral_")
         .replace("infty", "oo");

    s
}

/// Render a LaTeX or Typst math formula into a vector VGroup.
pub fn render_math(
    math_expr: &str,
    size: f32,
    fill: [f32; 4],
    stroke: [f32; 4],
    stroke_width: f32,
) -> Result<VGroup, String> {
    let typst_math = latex_to_typst(math_expr);

    let typst_src = format!(
        "#set page(width: auto, height: auto, margin: 0pt)\n$ {} $\n",
        typst_math
    );

    let mut child = Command::new("typst")
        .args(["compile", "--format", "svg", "-", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn typst compiler: {e}. Ensure typst is installed."))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(typst_src.as_bytes())
            .map_err(|e| format!("Failed to pipe code to typst: {e}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Typst process error: {e}"))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Typst compilation error: {err_msg}"));
    }

    let svg_content = String::from_utf8(output.stdout)
        .map_err(|e| format!("Typst produced invalid UTF-8: {e}"))?;

    parse_svg(&svg_content, size, fill, stroke, stroke_width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latex_to_typst_conversion() {
        let tex = r"\int_0^\infty e^{-x^2} dx = \frac{\sqrt{\pi}}{2}";
        let conv = latex_to_typst(tex);
        assert!(conv.contains("integral"));
        assert!(conv.contains("sqrt( pi )") || conv.contains("sqrt(pi)"));
        assert!(conv.contains("dif x"));
    }

    #[test]
    fn test_render_math_typst() {
        let res = render_math("e^(i pi) + 1 = 0", 1.0, [1.0; 4], [0.0; 4], 0.0);
        assert!(res.is_ok(), "Typst math rendering should succeed: {:?}", res.err());
        let vg = res.unwrap();
        assert!(!vg.points.is_empty(), "Math should generate vector points");
        assert!(vg.curve_count() > 5, "Math formula should have multiple curves");
    }
}
