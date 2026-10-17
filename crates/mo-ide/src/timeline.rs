use eframe::egui::{self, Color32, Pos2, Rect, Stroke};
use mo_core::scene::Timeline;

pub struct TimelinePanel {
    pub playing: bool,
    pub looping: bool,
    pub speed: f32,
}

impl Default for TimelinePanel {
    fn default() -> Self {
        Self {
            playing: false,
            looping: true,
            speed: 1.0,
        }
    }
}

impl TimelinePanel {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        timeline: Option<&Timeline>,
        current_time: &mut f32,
    ) {
        let total_duration = timeline.map(|t| t.duration()).unwrap_or(0.0).max(0.001);
        let current_frame = (*current_time * 60.0).round() as u32;
        let total_frames = (total_duration * 60.0).round() as u32;

        ui.horizontal(|ui| {
            // Transport controls
            let play_label = if self.playing { "⏸ Pause" } else { "▶ Play" };
            if ui.button(play_label).clicked() {
                self.playing = !self.playing;
            }

            if ui.button("⏮").on_hover_text("Jump to Start (Home)").clicked() {
                *current_time = 0.0;
            }

            if ui.button("◀").on_hover_text("Step Back 1 Frame (Left Arrow)").clicked() {
                self.playing = false;
                *current_time = (*current_time - 1.0 / 60.0).max(0.0);
            }

            if ui.button("▶").on_hover_text("Step Forward 1 Frame (Right Arrow)").clicked() {
                self.playing = false;
                *current_time = (*current_time + 1.0 / 60.0).min(total_duration);
            }

            ui.checkbox(&mut self.looping, "Loop");

            ui.separator();

            // Playback speed
            ui.label("Speed:");
            for &s in &[0.25, 0.5, 1.0, 2.0] {
                if ui.selectable_label(self.speed == s, format!("{}x", s)).clicked() {
                    self.speed = s;
                }
            }

            ui.separator();

            // Time & frame counter
            ui.label(
                egui::RichText::new(format!(
                    "{:05.2}s / {:05.2}s",
                    *current_time, total_duration
                ))
                .monospace()
                .strong()
                .color(Color32::from_rgb(88, 196, 221)),
            );

            ui.label(
                egui::RichText::new(format!(
                    "Frame: {} / {} (60 FPS)",
                    current_frame, total_frames
                ))
                .monospace()
                .color(Color32::from_rgb(150, 160, 180)),
            );
        });

        ui.add_space(4.0);

        // Scrubber slider
        ui.horizontal(|ui| {
            let slider_response = ui.add(
                egui::Slider::new(current_time, 0.0..=total_duration)
                    .show_value(false)
                    .trailing_fill(true),
            );
            if slider_response.dragged() {
                self.playing = false;
            }
        });

        ui.add_space(4.0);

        // Visual animation tracks (sequencer view)
        let track_height = 22.0;
        let Some(tl) = timeline else { return };
        let anim_list = tl.timed_anims();
        if anim_list.is_empty() { return };

        let num_tracks = anim_list.len().min(8);
        let total_track_area_height = (num_tracks as f32) * (track_height + 4.0) + 10.0;

        let (response, painter) = ui.allocate_painter(
            egui::vec2(ui.available_width(), total_track_area_height),
            egui::Sense::click_and_drag(),
        );

        let rect = response.rect;

        // Draw track area background
        painter.rect_filled(rect, 4.0, Color32::from_rgb(25, 28, 36));
        painter.rect_stroke(rect, 4.0, Stroke::new(1.0_f32, Color32::from_rgb(45, 50, 65)), egui::StrokeKind::Inside);

        // Handle clicking / dragging on track area to seek
        if response.clicked() || response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                let frac = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                *current_time = frac * total_duration;
                self.playing = false;
            }
        }

        // Draw each animation block
        for (idx, &(start_time, timed_anim)) in anim_list.iter().enumerate().take(num_tracks) {
            let y_top = rect.top() + 6.0 + (idx as f32) * (track_height + 4.0);
            let y_bot = y_top + track_height;

            let start_x = rect.left() + (start_time / total_duration) * rect.width();
            let end_x = rect.left() + ((start_time + timed_anim.run_time) / total_duration) * rect.width();

            let block_rect = Rect::from_min_max(Pos2::new(start_x, y_top), Pos2::new(end_x.max(start_x + 4.0), y_bot));

            let is_active = *current_time >= start_time
                && *current_time <= start_time + timed_anim.run_time;

            let (block_color, stroke_color) = match &timed_anim.anim {
                mo_core::anim::Animation::ShowCreation { .. } => (
                    Color32::from_rgb(40, 100, 120),
                    Color32::from_rgb(88, 196, 221),
                ),
                mo_core::anim::Animation::Transform { .. } => (
                    Color32::from_rgb(120, 60, 40),
                    Color32::from_rgb(252, 98, 85),
                ),
                mo_core::anim::Animation::Rotate { .. } => (
                    Color32::from_rgb(100, 90, 30),
                    Color32::from_rgb(240, 200, 60),
                ),
                mo_core::anim::Animation::FadeIn { .. } | mo_core::anim::Animation::FadeOut { .. } => (
                    Color32::from_rgb(60, 50, 90),
                    Color32::from_rgb(187, 154, 247),
                ),
                _ => (
                    Color32::from_rgb(45, 65, 85),
                    Color32::from_rgb(120, 160, 200),
                ),
            };

            let final_bg = if is_active {
                Color32::from_rgb(
                    (block_color.r() as f32 * 1.4).min(255.0) as u8,
                    (block_color.g() as f32 * 1.4).min(255.0) as u8,
                    (block_color.b() as f32 * 1.4).min(255.0) as u8,
                )
            } else {
                block_color
            };

            painter.rect_filled(block_rect, 3.0, final_bg);
            painter.rect_stroke(block_rect, 3.0, Stroke::new(if is_active { 1.5_f32 } else { 1.0_f32 }, stroke_color), egui::StrokeKind::Inside);

            // Animation label text
            let anim_name = match &timed_anim.anim {
                mo_core::anim::Animation::ShowCreation { id } => format!("ShowCreation({})", id),
                mo_core::anim::Animation::Transform { id, .. } => format!("Transform({})", id),
                mo_core::anim::Animation::Rotate { id, angle } => format!("Rotate({}, {:.1})", id, angle),
                mo_core::anim::Animation::FadeIn { id, .. } => format!("FadeIn({})", id),
                mo_core::anim::Animation::FadeOut { id, .. } => format!("FadeOut({})", id),
                mo_core::anim::Animation::Grow { id, .. } => format!("Grow({})", id),
                mo_core::anim::Animation::SpinInFromNothing { id } => format!("SpinIn({})", id),
                mo_core::anim::Animation::ShrinkToCenter { id } => format!("Shrink({})", id),
                mo_core::anim::Animation::Indicate { id, .. } => format!("Indicate({})", id),
                mo_core::anim::Animation::Wiggle { id, .. } => format!("Wiggle({})", id),
            };

            if block_rect.width() > 30.0 {
                let text_pos = Pos2::new(block_rect.left() + 6.0, block_rect.top() + 4.0);
                painter.text(
                    text_pos,
                    egui::Align2::LEFT_TOP,
                    anim_name,
                    egui::FontId::proportional(11.0),
                    Color32::WHITE,
                );
            }
        }

        // Draw vertical playhead cursor
        let cursor_x = rect.left() + (*current_time / total_duration) * rect.width();
        painter.line_segment(
            [Pos2::new(cursor_x, rect.top()), Pos2::new(cursor_x, rect.bottom())],
            Stroke::new(2.0_f32, Color32::from_rgb(255, 255, 255)),
        );
        painter.circle_filled(
            Pos2::new(cursor_x, rect.top() + 3.0),
            4.0,
            Color32::from_rgb(252, 98, 85),
        );
    }
}
