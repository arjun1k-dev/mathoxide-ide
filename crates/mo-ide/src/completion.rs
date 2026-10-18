//! Autocomplete catalog + hover documentation for the `.moa` DSL.

#[derive(Clone)]
pub struct Completion {
    pub label: String,
    pub insert: String,
    pub detail: String,
    pub doc: String,
}

impl Completion {
    fn new(label: &str, insert: &str, detail: &str, doc: &str) -> Self {
        Self {
            label: label.into(),
            insert: insert.into(),
            detail: detail.into(),
            doc: doc.into(),
        }
    }
}

/// Static DSL catalog. Doc lines are the hover tooltip content.
pub fn builtin_completions() -> Vec<Completion> {
    let mut v = Vec::new();

    // Object declarations
    for (name, sig, doc) in [
        ("circle", "circle <id> at (x, y) r=<r>", "2D circle. r = radius."),
        ("rect", "rect <id> at (x, y) w=<w>, h=<h>", "Rectangle. w/h = width/height."),
        ("square", "square <id> at (x, y) s=<s>", "Square, s = side length."),
        ("line", "line <id> from (x, y) to (x, y)", "Line segment between two points."),
        ("dot", "dot <id> at (x, y)", "Small filled point."),
        ("arrow", "arrow <id> from (x, y) to (x, y)", "Arrow between two points."),
        ("polygon", "polygon <id> [(x, y), (x, y), ...]", "Closed polygon from vertex list."),
        ("text", "text <id> at (x, y) text=\"...\"", "Plain text object."),
        ("tex", "tex <id> at (x, y) text=\"...\" size=<s>", "Typst-powered LaTeX. size = scale."),
        ("axes", "axes <id> range=[xmin, xmax, ymin, ymax]", "2D coordinate axes."),
        ("plot", "plot <id> f=t expr=\"...\" range=[...]", "Plot an expression over the axes range. sw = stroke weight."),
        ("func", "func <id> expr=\"...\"", "Math function object."),
        ("axes3d", "axes3d <id>", "3D coordinate axes (orbit with right-click drag)."),
        ("cube", "cube <id> at (x, y, z) s=<s>", "3D cube, s = side length."),
        ("sphere", "sphere <id> at (x, y, z) r=<r>", "3D sphere."),
        ("surface3d", "surface3d <id> expr=\"...\" res=<n>", "3D surface z = expr. res = samples/axis (clamped to 400). wire_stride decimates the wireframe."),
    ] {
        v.push(Completion::new(name, &format!("{name} "), sig, doc));
    }

    // Animations
    for (name, sig, doc) in [
        ("showCreation", "showCreation(id)", "Reveals the object's geometry as it is drawn."),
        ("write", "write(id)", "Handwriting-style reveal (LaTeX/text)."),
        ("fadeIn", "fadeIn(id)", "Object fades in from transparent."),
        ("fadeOut", "fadeOut(id)", "Object fades out to transparent."),
        ("growFromCenter", "growFromCenter(id)", "Object scales up from its center."),
        ("rotate", "rotate(id, <angle>)", "Rotates the object by an angle (radians)."),
        ("transform", "transform(idA, idB)", "Morphs object A into object B's shape."),
        ("replacementTransform", "replacementTransform(idA, idB)", "Swaps A for B without morphing."),
        ("camera.orbit", "camera.orbit(phi=<deg>, theta=<deg>)", "Orbits the 3D camera. phi = polar, theta = azimuth."),
        ("camera.zoom", "camera.zoom(<factor>)", "Zooms the camera."),
    ] {
        v.push(Completion::new(name, name, sig, doc));
    }

    // Easing / rate functions
    for (name, doc) in [
        ("smooth", "Default ease-in-out (smoothstep-based)."),
        ("linear", "Constant speed."),
        ("there_and_back", "Goes to the target and returns to start."),
        ("rush_into", "Starts slow, accelerates into the end."),
        ("rush_from", "Starts fast, decelerates."),
        ("slow_into", "Eases out gently at the end."),
        ("double_smooth", "Two stacked smoothsteps — snappier middle."),
        ("wiggle", "Oscillates a few times before settling."),
        ("exponential_decay", "Fast start, exponential slow-down."),
    ] {
        v.push(Completion::new(name, name, "easing", doc));
    }

    // Statements
    for (name, sig, doc) in [
        ("play", "play <anim>(<args>) <dur>s <easing>;", "Runs an animation for a duration."),
        ("wait", "wait <dur>s;", "Holds the timeline for a duration."),
        ("import", "import \"assets/<name>.moa\" as <alias> at (x, y);", "Composes another scene file. Resolves relative to this file."),
    ] {
        v.push(Completion::new(name, &format!("{name} "), sig, doc));
    }

    v
}

/// Documentation lookup for hover (builtins only; user symbols handled separately).
pub fn doc_for(word: &str) -> Option<String> {
    for c in builtin_completions() {
        if c.label == word {
            return Some(format!("{}\n\n{}", c.detail, c.doc));
        }
    }
    None
}

/// Scan the code for user-declared symbols: object ids and import aliases.
pub fn user_symbols(code: &str) -> Vec<Completion> {
    let mut out = Vec::new();
    for line in code.lines() {
        let t = line.trim();
        if t.starts_with("//") {
            continue;
        }
        let words: Vec<&str> = t.split_whitespace().collect();
        // Object declaration: "<type> <id> ..."
        if words.len() >= 2 {
            let is_decl = matches!(
                words[0],
                "circle" | "rect" | "square" | "line" | "dot" | "arrow" | "polygon"
                    | "text" | "tex" | "axes" | "plot" | "func" | "axes3d" | "cube"
                    | "sphere" | "surface3d"
            ) && words[1].chars().all(|c| c.is_alphanumeric() || c == '_');
            if is_decl {
                let id = words[1].trim_end_matches(|c| c == ';');
                if !out.iter().any(|c: &Completion| c.label == id) {
                    out.push(Completion::new(id, id, &format!("{} (user object)", words[0]), &format!("`{id}` — a `{}` declared in this scene.", words[0])));
                }
            }
        }
        // Import alias: import "..." as <alias>
        if words.first() == Some(&"import") {
            if let Some(pos) = words.iter().position(|w| *w == "as") {
                if let Some(alias) = words.get(pos + 1) {
                    let alias = alias.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_');
                    if !alias.is_empty() && !out.iter().any(|c: &Completion| c.label == alias) {
                        out.push(Completion::new(alias, alias, "import (user asset)", &format!("`{alias}` — imported scene asset.")));
                    }
                }
            }
        }
    }
    out
}

/// All completions: builtins + user symbols from the current buffer.
pub fn all_completions(code: &str) -> Vec<Completion> {
    let mut v = builtin_completions();
    v.extend(user_symbols(code));
    v
}

/// The word prefix directly before the caret (identifier chars only).
/// Returns (byte_start_of_prefix, prefix).
pub fn prefix_before_caret(text: &str, caret_char_idx: usize) -> (usize, String) {
    let chars: Vec<char> = text.chars().collect();
    let mut start = caret_char_idx.min(chars.len());
    while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
        start -= 1;
    }
    let prefix: String = chars[start..caret_char_idx.min(chars.len())].iter().collect();
    // byte offset for replace_range
    let byte_start: usize = chars[..start].iter().map(|c| c.len_utf8()).sum();
    (byte_start, prefix)
}

/// Byte span of the identifier prefix ending at the caret: (start, end).
pub fn prefix_span_at(text: &str, caret_char_idx: usize) -> (usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    let caret = caret_char_idx.min(chars.len());
    let mut start = caret;
    while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
        start -= 1;
    }
    let byte_start: usize = chars[..start].iter().map(|c| c.len_utf8()).sum();
    let byte_end: usize = chars[..caret].iter().map(|c| c.len_utf8()).sum();
    (byte_start, byte_end)
}

/// Line and column (char-based) of a caret char index.
pub fn line_col_of(text: &str, caret_char_idx: usize) -> (usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    let idx = caret_char_idx.min(chars.len());
    let mut line = 0;
    let mut col = 0;
    for c in &chars[..idx] {
        if *c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// The identifier under a (line, col) point, if any.
pub fn word_at_line_col(text: &str, line: usize, col: usize) -> Option<String> {
    let line_text = text.lines().nth(line)?;
    let chars: Vec<char> = line_text.chars().collect();
    let col = col.min(chars.len());
    if col >= chars.len() || !(chars[col].is_alphanumeric() || chars[col] == '_') {
        return None;
    }
    let mut start = col;
    while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
        start -= 1;
    }
    let mut end = col;
    while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
        end += 1;
    }
    Some(chars[start..end].iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_span_is_byte_based() {
        // plain ascii
        // caret char index 8 sits right after the id "c" in "circle c ..."
        let (s, e) = prefix_span_at("circle c at (0,0)", 8);
        assert_eq!(&"circle c at (0,0)"[s..e], "c");

        // multibyte before the prefix must not shift the span
        let text = "tex ✓ lab";
        let caret = text.chars().count(); // end
        let (s, e) = prefix_span_at(text, caret);
        assert_eq!(&text[s..e], "lab");
    }
}
