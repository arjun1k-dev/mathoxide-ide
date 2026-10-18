use eframe::egui::{self, Color32, FontId, TextFormat};

/// Highlighted DSL tokens for the `.moa` language.
pub fn highlight_moa(ctx: &egui::Context, text: &str) -> egui::text::LayoutJob {
    let font_id = FontId::monospace(13.0);
    let mut job = egui::text::LayoutJob::default();

    // Cyberpunk palette
    let color_default = Color32::from_rgb(0x00, 0xFF, 0x9D);   // #00FF9D neon green (variables/identifiers)
    let color_keyword = Color32::from_rgb(0xFF, 0x00, 0xA8);   // #FF00A8 hot magenta
    let color_type = Color32::from_rgb(0xC7, 0x24, 0xFF);      // #C724FF neon violet (types)
    let color_anim = Color32::from_rgb(0xF5, 0xFF, 0x00);      // #F5FF00 acid yellow (functions/anims)
    let color_rate = Color32::from_rgb(0x00, 0xFF, 0x9D);      // #00FF9D neon green (easings)
    let color_string = Color32::from_rgb(0x00, 0xFF, 0xF7);    // #00FFF7 electric cyan
    let color_hex = Color32::from_rgb(0xFF, 0xD1, 0x66);       // #FFD166 warm amber gold (hex colors)
    let color_number = Color32::from_rgb(0xFF, 0x6B, 0x35);    // #FF6B35 neon orange
    let color_comment = Color32::from_rgb(0x8A, 0x2B, 0xE2);   // #8A2BE2 bright violet
    let color_operator = Color32::from_rgb(0x00, 0xD4, 0xFF);  // #00D4FF deep cyan

    let keywords = [
        "play", "wait", "at", "from", "to", "r", "w", "h", "fill", "stroke", "sw", "size",
        "on", "expr", "f", "samples", "side", "tip_len", "tip_w", "color", "import", "as",
    ];
    let types = [
        "circle", "rect", "line", "text", "tex", "axes", "func", "plot", "graph", "dot", "square", "arrow", "polygon",
        "axes3d", "cube", "sphere", "surface3d",
        "Circle", "Rect", "Line", "Text", "Axes", "Func", "Plot", "Graph", "Dot", "Square", "Arrow", "Polygon",
    ];
    let anims = [
        "showCreation", "write", "fadeIn", "fadeOut", "grow", "growFromCenter", "rotate", "transform", "replacementTransform",
        "orbit", "zoom", "camera",
        "ShowCreation", "Write", "FadeIn", "FadeOut", "Grow", "GrowFromCenter", "Rotate", "Transform", "ReplacementTransform",
    ];
    let rates = [
        "linear", "smooth", "rushInto", "rushFrom", "slowInto", "thereAndBack",
        "rush_into", "rush_from", "slow_into", "there_and_back", "ease_in_out",
        "double_smooth", "wiggle", "exponential_decay",
    ];

    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let n = chars.len();

    while i < n {
        let ch = chars[i];

        // Comments (// ...)
        if ch == '/' && i + 1 < n && chars[i + 1] == '/' {
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
                color: color_hex,
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

        // Operators and punctuation
        let s = ch.to_string();
        job.append(&s, 0.0, TextFormat {
            font_id: font_id.clone(),
            color: color_operator,
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
    // Autocomplete state
    comp_open: bool,
    completions: Vec<crate::completion::Completion>,
    comp_start_byte: usize,
    comp_selected: usize,
    // Hover-doc state: last pointer position + when it settled there
    hover_point: Option<(egui::Pos2, std::time::Instant)>,
    /// True while the code text edit holds keyboard focus this frame.
    pub focus_held: bool,
}

impl EditorPanel {
    pub fn new(code: String) -> Self {
        Self {
            code,
            parse_error: None,
            error_line: None,
            is_dirty: false,
            auto_compile: true,
            comp_open: false,
            completions: Vec::new(),
            comp_start_byte: 0,
            comp_selected: 0,
            hover_point: None,
            focus_held: false,
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
                ui.colored_label(Color32::from_rgb(0xFF, 0x00, 0x3C), "● modified");
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
                .fill(Color32::from_rgb(0x33, 0x00, 0x0F))
                .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(0xFF, 0x00, 0x3C)))
                .inner_margin(6.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.colored_label(Color32::from_rgb(0xFF, 0x00, 0x3C), "⚠ Error:");
                        ui.label(err);
                    });
                });
            ui.add_space(4.0);
        }

        // Editor area with line numbers and custom syntax highlighting
        let total_lines = self.code.lines().count().max(1);
        let gutter_width = 32.0 + (total_lines.to_string().len() as f32) * 8.0;

        // Autocomplete key handling: consume nav keys before the TextEdit sees
        // them, so Enter/Tab accept instead of inserting a newline.
        let popup_active = self.comp_open && !self.completions.is_empty();
        let mut accept_selection = false;
        if popup_active {
            let max = self.completions.len() - 1;
            ui.ctx().input_mut(|i| {
                if i.consume_key(egui::Modifiers::default(), egui::Key::ArrowDown) {
                    self.comp_selected = (self.comp_selected + 1).min(max);
                }
                if i.consume_key(egui::Modifiers::default(), egui::Key::ArrowUp) {
                    self.comp_selected = self.comp_selected.saturating_sub(1);
                }
                if i.consume_key(egui::Modifiers::default(), egui::Key::Escape) {
                    self.comp_open = false;
                }
            });
            let enter = ui.ctx().input_mut(|i| i.consume_key(egui::Modifiers::default(), egui::Key::Enter));
            let tab = ui.ctx().input_mut(|i| i.consume_key(egui::Modifiers::default(), egui::Key::Tab));
            accept_selection = enter || tab;
        }

        egui::ScrollArea::both()
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
                                    .color(Color32::from_rgb(0x4A, 0x00, 0x99))
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

                    // Fixed wide text area — panel narrower than the code
                    // h-scrolls instead of wrapping.
                    let width = ui.available_width().max(700.0);
                    let height = ui.available_height().max(400.0);
                    let response = ui.add_sized(
                        [width, height],
                        egui::TextEdit::multiline(&mut self.code)
                            .font(FontId::monospace(13.0))
                            .code_editor()
                            .background_color(Color32::from_rgb(0x05, 0x00, 0x1A))
                            .lock_focus(true)
                            .desired_width(width)
                            .layouter(&mut layouter),
                    );

                    if response.changed() {
                        self.is_dirty = true;
                        text_changed = true;
                    }

                    // Keyboard focus sticks to the editor: reclaim it whenever
                    // no other widget holds it (e.g. after layout shifts from
                    // the error bar appearing, or a stray click).
                    let nothing_focused = ui.memory(|m| m.focused().is_none());
                    if !response.has_focus() && nothing_focused {
                        response.request_focus();
                    }
                    self.focus_held = response.has_focus();

                    // ---- Autocomplete ----
                    let font_id = FontId::monospace(13.0);
                    let line_h = ui.fonts(|f| f.row_height(&font_id)).max(15.0);
                    let char_w = ui.fonts(|f| {
                        let galley = f.layout_no_wrap("0".to_string(), font_id.clone(), Color32::WHITE);
                        galley.size().x.max(1.0)
                    });

                    let caret_char = egui::widgets::text_edit::TextEditState::load(ui.ctx(), response.id)
                        .and_then(|st| st.cursor.char_range())
                        .map(|r| r.primary.index)
                        .unwrap_or(0);

                    if response.changed() {
                        // Recompute the completion list for the word being typed.
                        let (start, prefix) = crate::completion::prefix_before_caret(&self.code, caret_char);
                        let items: Vec<_> = crate::completion::all_completions(&self.code)
                            .into_iter()
                            .filter(|c| {
                                !prefix.is_empty()
                                    && c.label.to_lowercase().starts_with(&prefix.to_lowercase())
                                    && c.label != prefix
                            })
                            .collect();
                        if items.is_empty() {
                            self.comp_open = false;
                        } else {
                            self.completions = items;
                            self.comp_start_byte = start;
                            self.comp_selected = 0;
                            self.comp_open = true;
                        }
                    } else if self.comp_open {
                        // Caret moved without typing (arrows/click) — validate prefix.
                        let (_, prefix) = crate::completion::prefix_before_caret(&self.code, caret_char);
                        if prefix.is_empty() {
                            self.comp_open = false;
                        }
                    }

                    if accept_selection && popup_active && self.comp_open {
                        if let Some(c) = self.completions.get(self.comp_selected).cloned() {
                            let (end, insert_end) = crate::completion::prefix_span_at(&self.code, caret_char);
                            self.code.replace_range(end..insert_end, &c.insert);
                            self.comp_open = false;
                            text_changed = true;
                            self.is_dirty = true;
                            // Put the caret after the inserted text.
                            let new_idx = self.code[..end + c.insert.len()].chars().count();
                            if let Some(mut st) = egui::widgets::text_edit::TextEditState::load(ui.ctx(), response.id) {
                                let c = egui::text::CCursor::new(new_idx);
                                st.cursor.set_char_range(Some(egui::text_selection::CCursorRange { primary: c, secondary: c }));
                                st.store(ui.ctx(), response.id);
                            }
                        }
                    }

                    if self.comp_open && !self.completions.is_empty() {
                        let (line, col) = crate::completion::line_col_of(&self.code, caret_char);
                        let pos = egui::pos2(
                            response.rect.left() + 4.0 + col as f32 * char_w,
                            (response.rect.top() + (line + 1) as f32 * line_h)
                                .min(ui.ctx().screen_rect().bottom() - 200.0),
                        );
                        let selected = self.comp_selected;
                        let items = self.completions.clone();
                        let mut clicked: Option<usize> = None;
                        egui::Area::new(egui::Id::new("completion_popup"))
                            .fixed_pos(pos)
                            .order(egui::Order::Foreground)
                            .show(ui.ctx(), |ui| {
                                egui::Frame::popup(ui.style()).show(ui, |ui| {
                                    egui::ScrollArea::vertical().max_height(190.0).show(ui, |ui| {
                                        for (i, c) in items.iter().enumerate() {
                                            let row = ui.horizontal(|ui| {
                                                let color = match c.detail.as_str() {
                                                    d if d.contains("easing") => Color32::from_rgb(0x00, 0xFF, 0x9D),
                                                    d if d.contains("user") => Color32::from_rgb(0x00, 0xFF, 0x9D),
                                                    _ => Color32::from_rgb(0xF5, 0xFF, 0x00),
                                                };
                                                ui.label(egui::RichText::new(&c.label).monospace().color(color));
                                                ui.label(
                                                    egui::RichText::new(&c.detail)
                                                        .size(10.5)
                                                        .color(Color32::from_rgb(140, 150, 175)),
                                                );
                                            });
                                            if ui
                                                .interact(row.response.rect, egui::Id::new("comp").with(i), egui::Sense::click())
                                                .clicked()
                                            {
                                                clicked = Some(i);
                                            }
                                            if i == selected {
                                                ui.painter().rect_stroke(
                                                    row.response.rect,
                                                    2.0,
                                                    egui::Stroke::new(1.0_f32, Color32::from_rgb(0x00, 0xFF, 0xF7)),
                                                    egui::StrokeKind::Inside,
                                                );
                                            }
                                        }
                                    });
                                });
                            });
                        if let Some(i) = clicked {
                            self.comp_selected = i;
                            if let Some(c) = self.completions.get(i).cloned() {
                                let (end, insert_end) = crate::completion::prefix_span_at(&self.code, caret_char);
                                self.code.replace_range(end..insert_end, &c.insert);
                                self.comp_open = false;
                                text_changed = true;
                                self.is_dirty = true;
                                let new_idx = self.code[..end + c.insert.len()].chars().count();
                                if let Some(mut st) = egui::widgets::text_edit::TextEditState::load(ui.ctx(), response.id) {
                                    let c = egui::text::CCursor::new(new_idx);
                                    st.cursor.set_char_range(Some(egui::text_selection::CCursorRange { primary: c, secondary: c }));
                                    st.store(ui.ctx(), response.id);
                                }
                            }
                        }
                    }

                    // ---- Hover docs (>0.6s stationary) ----
                    let pointer = ui.input(|i| i.pointer.hover_pos());
                    let mut hover_doc: Option<(egui::Pos2, String)> = None;
                    if let Some(p) = pointer {
                        if response.rect.contains(p) {
                            match self.hover_point {
                                Some((last, t0)) if (last - p).length() < 3.0 => {
                                    if t0.elapsed().as_secs_f32() > 0.6 {
                                        let col = ((p.x - response.rect.left() - 4.0) / char_w) as usize;
                                        let line = ((p.y - response.rect.top()) / line_h) as usize;
                                        if let Some(word) =
                                            crate::completion::word_at_line_col(&self.code, line, col)
                                        {
                                            if let Some(doc) = crate::completion::doc_for(&word) {
                                                hover_doc = Some((p + egui::vec2(12.0, 16.0), doc));
                                            }
                                        }
                                    }
                                }
                                _ => {
                                    self.hover_point = Some((p, std::time::Instant::now()));
                                }
                            }
                        } else {
                            self.hover_point = None;
                        }
                    } else {
                        self.hover_point = None;
                    }
                    if let Some((pos, doc)) = hover_doc {
                        egui::Area::new(egui::Id::new("hover_doc"))
                            .fixed_pos(pos)
                            .order(egui::Order::Tooltip)
                            .show(ui.ctx(), |ui| {
                                egui::Frame::popup(ui.style()).show(ui, |ui| {
                                    ui.set_max_width(340.0);
                                    for line in doc.lines() {
                                        let styled = if line.lines().count() == 0 || !line.starts_with('`') && !line.contains('`') && line.len() < 40 && line.contains('(') {
                                            egui::RichText::new(line).monospace().color(Color32::from_rgb(0x00, 0xFF, 0xF7)).size(11.5)
                                        } else {
                                            egui::RichText::new(line).size(11.5)
                                        };
                                        ui.label(styled);
                                    }
                                });
                            });
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

        // Verify that // is recognized as comment
        let comment_section = job.sections.iter().find(|s| {
            let start = s.byte_range.start;
            let end = s.byte_range.end;
            &job.text[start..end] == "// Demo comment"
        });
        assert!(comment_section.is_some(), "Expected '// Demo comment' section");
        assert_eq!(comment_section.unwrap().format.color, Color32::from_rgb(0x8A, 0x2B, 0xE2));

        // Verify that #58C4DD33 is recognized as hex color, NOT comment
        let hex_section = job.sections.iter().find(|s| {
            let start = s.byte_range.start;
            let end = s.byte_range.end;
            &job.text[start..end] == "#58C4DD33"
        });
        assert!(hex_section.is_some(), "Expected '#58C4DD33' hex section");
        assert_eq!(hex_section.unwrap().format.color, Color32::from_rgb(0xFF, 0xD1, 0x66));

        // Verify that stroke following #58C4DD33 is recognized as keyword
        let stroke_section = job.sections.iter().find(|s| {
            let start = s.byte_range.start;
            let end = s.byte_range.end;
            &job.text[start..end] == "stroke"
        });
        assert!(stroke_section.is_some(), "Expected 'stroke' keyword section");
        assert_eq!(stroke_section.unwrap().format.color, Color32::from_rgb(0xFF, 0x00, 0xA8));
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

