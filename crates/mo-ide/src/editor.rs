use eframe::egui::{self, Color32, FontId, TextFormat};

/// Highlighted DSL tokens for the `.moa` language.
pub fn highlight_moa(ctx: &egui::Context, text: &str) -> egui::text::LayoutJob {
    let font_id = FontId::monospace(13.0);
    let mut job = egui::text::LayoutJob::default();

    let color_default = Color32::from_rgb(192, 202, 245);      // Soft cyan-white
    let color_keyword = Color32::from_rgb(187, 154, 247);      // Mauve / purple
    let color_type = Color32::from_rgb(88, 196, 221);          // 3b1b Cyan
    let color_anim = Color32::from_rgb(252, 98, 85);           // 3b1b Red/Amber
    let color_rate = Color32::from_rgb(125, 207, 255);         // Teal
    let color_string = Color32::from_rgb(158, 206, 106);       // Light green
    let color_number = Color32::from_rgb(255, 158, 100);       // Orange
    let color_comment = Color32::from_rgb(108, 117, 137);      // Muted slate

    let keywords = ["play", "wait", "at", "from", "to", "r", "w", "h", "fill", "stroke", "sw", "size"];
    let types = ["circle", "rect", "line", "text", "Circle", "Rect", "Line", "Text"];
    let anims = [
        "showCreation", "fadeIn", "fadeOut", "grow", "rotate", "transform",
        "ShowCreation", "FadeIn", "FadeOut", "Grow", "Rotate", "Transform",
    ];
    let rates = [
        "linear", "smooth", "rushInto", "rushFrom", "slowInto", "thereAndBack",
        "rush_into", "rush_from", "slow_into", "there_and_back", "ease_in_out",
    ];

    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let n = chars.len();

    while i < n {
        let ch = chars[i];

        // Comments (// ... or # ...)
        if (ch == '/' && i + 1 < n && chars[i + 1] == '/') || ch == '#' {
            let start = i;
            while i < n && chars[i] != '\n' {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            job.append(&s, 0.0, TextFormat {
                font_id: font_id.clone(),
                color: color_comment,
                ..Default::default()
            });
            continue;
        }

        // Hex colors (#58C4DD, #FC625533, etc.)
        if ch == '#' && i + 1 < n && chars[i + 1].is_ascii_hexdigit() {
            let start = i;
            i += 1;
            while i < n && chars[i].is_ascii_hexdigit() {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            job.append(&s, 0.0, TextFormat {
                font_id: font_id.clone(),
                color: color_string,
                ..Default::default()
            });
            continue;
        }

        // String literals ("...")
        if ch == '"' {
            let start = i;
            i += 1;
            while i < n && chars[i] != '"' && chars[i] != '\n' {
                i += 1;
            }
            if i < n && chars[i] == '"' {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            job.append(&s, 0.0, TextFormat {
                font_id: font_id.clone(),
                color: color_string,
                ..Default::default()
            });
            continue;
        }

        // Numbers (integers, floats, and durations like 2s, 0.5s)
        if ch.is_ascii_digit() || (ch == '.' && i + 1 < n && chars[i + 1].is_ascii_digit()) {
            let start = i;
            while i < n && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            if i < n && chars[i] == 's' {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            job.append(&s, 0.0, TextFormat {
                font_id: font_id.clone(),
                color: color_number,
                ..Default::default()
            });
            continue;
        }

        // Identifiers or keywords
        if ch.is_alphabetic() || ch == '_' {
            let start = i;
            while i < n && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let col = if keywords.contains(&word.as_str()) {
                color_keyword
            } else if types.contains(&word.as_str()) {
                color_type
            } else if anims.contains(&word.as_str()) {
                color_anim
            } else if rates.contains(&word.as_str()) {
                color_rate
            } else {
                color_default
            };
            job.append(&word, 0.0, TextFormat {
                font_id: font_id.clone(),
                color: col,
                ..Default::default()
            });
            continue;
        }

        // Symbols and whitespace
        let s = ch.to_string();
        job.append(&s, 0.0, TextFormat {
            font_id: font_id.clone(),
            color: color_default,
            ..Default::default()
        });
        i += 1;
    }

    let _ = ctx;
    job
}

/// Code Editor state and panel.
pub struct EditorPanel {
    pub code: String,
    pub parse_error: Option<String>,
    pub error_line: Option<usize>,
    pub is_dirty: bool,
    pub auto_compile: bool,
}

impl EditorPanel {
    pub fn new(code: String) -> Self {
        Self {
            code,
            parse_error: None,
            error_line: None,
            is_dirty: false,
            auto_compile: true,
        }
    }

    pub fn set_error(&mut self, err: Option<String>) {
        self.parse_error = err.clone();
        self.error_line = err.and_then(|e| {
            // Extract line from format: "line X, col Y" or similar
            if let Some(pos) = e.find("line ") {
                let rest = &e[pos + 5..];
                rest.split(|c: char| !c.is_ascii_digit())
                    .next()
                    .and_then(|s| s.parse::<usize>().ok())
            } else {
                None
            }
        });
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> (bool, bool) {
        let mut text_changed = false;
        let mut save_requested = false;

        ui.horizontal(|ui| {
            ui.heading("Editor");
            if self.is_dirty {
                ui.colored_label(Color32::from_rgb(252, 98, 85), "● modified");
            } else {
                ui.colored_label(Color32::from_rgb(158, 206, 106), "✓ saved");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Save (Ctrl+S)").clicked() {
                    save_requested = true;
                }
                ui.checkbox(&mut self.auto_compile, "Auto-Compile");
            });
        });

        ui.separator();

        // Diagnostics bar if there's an error
        if let Some(err) = &self.parse_error {
            egui::Frame::new()
                .fill(Color32::from_rgb(60, 20, 20))
                .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(252, 98, 85)))
                .inner_margin(6.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.colored_label(Color32::from_rgb(252, 98, 85), "⚠ Error:");
                        ui.label(err);
                    });
                });
            ui.add_space(4.0);
        }

        // Editor area with line numbers and custom syntax highlighting
        let total_lines = self.code.lines().count().max(1);
        let gutter_width = 32.0 + (total_lines.to_string().len() as f32) * 8.0;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Line numbers gutter
                    ui.allocate_ui(egui::vec2(gutter_width, ui.available_height()), |ui| {
                        let mut line_num_str = String::new();
                        for l in 1..=total_lines {
                            line_num_str.push_str(&format!("{:>4}\n", l));
                        }
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(line_num_str)
                                    .monospace()
                                    .color(Color32::from_rgb(90, 100, 120))
                                    .size(13.0),
                            )
                            .selectable(false),
                        );
                    });

                    ui.separator();

                    // Code text edit with custom syntax highlighting
                    let mut layouter = |ui: &egui::Ui, text: &str, _wrap_width: f32| {
                        let job = highlight_moa(ui.ctx(), text);
                        ui.fonts(|f| f.layout_job(job))
                    };

                    let response = ui.add_sized(
                        ui.available_size(),
                        egui::TextEdit::multiline(&mut self.code)
                            .font(FontId::monospace(13.0))
                            .code_editor()
                            .lock_focus(true)
                            .desired_width(f32::INFINITY)
                            .layouter(&mut layouter),
                    );

                    if response.changed() {
                        self.is_dirty = true;
                        text_changed = true;
                    }
                });
            });

        (text_changed, save_requested)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_moa_sections() {
        let ctx = egui::Context::default();
        let code = r#"
// Demo comment
circle c at (0,0) r=2 fill=#58C4DD33, stroke=blue, sw=6;
play showCreation(c) 2s smooth;
"#;
        let job = highlight_moa(&ctx, code);
        assert!(!job.sections.is_empty());
        assert_eq!(job.text, code);
    }

    #[test]
    fn test_set_error_line_parsing() {
        let mut editor = EditorPanel::new("scene A {}".into());
        assert!(editor.error_line.is_none());

        editor.set_error(Some("1:5: unexpected token at line 3, col 10".into()));
        assert_eq!(editor.error_line, Some(3));

        editor.set_error(None);
        assert!(editor.error_line.is_none());
        assert!(editor.parse_error.is_none());
    }
}

