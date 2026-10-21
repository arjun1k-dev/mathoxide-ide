use crate::ext::egui::{self, Color32, CursorIcon, Pos2, Rect, Stroke, Vec2};
use mo_core::anim::TimedAnim;
use mo_core::scene::{Stmt, Timeline};

/// Actions the timeline panel asks the app to perform.
pub enum TimelineAction {
    /// Rewrite `play ... Ns ...;` durations in the .moa code to match the
    /// (possibly drag-edited) run times. Carries run times in play order.
    ApplyTimings(Vec<f32>),
    /// The timeline was mutated by dragging; caller should mark the file dirty.
    Edited,
}

/// Min duration a play block can be trimmed to, in seconds.
const MIN_RUN: f32 = 0.1;
/// Half-width of the right-edge grab zone, in pixels.
const EDGE_GRAB: f32 = 6.0;

#[derive(Clone, Copy, PartialEq)]
enum DragMode {
    /// Drag the whole block: trades time with the preceding Wait (a zero-length
    /// one is inserted if needed) — everything after shifts along.
    Move,
    /// Drag the right edge: changes the animation's run time.
    TrimR,
}

struct TrackDrag {
    /// Index of the Play statement being dragged.
    play_idx: usize,
    /// Index of the Wait statement just before the play (Move mode only).
    wait_idx: usize,
    orig_run: f32,
    /// Pointer offset inside the block at press, in seconds.
    grab_offset: f32,
    mode: DragMode,
}

pub struct TimelinePanel {
    pub playing: bool,
    pub looping: bool,
    pub speed: f32,
    drag: Option<TrackDrag>,
}

impl Default for TimelinePanel {
    fn default() -> Self {
        Self {
            playing: false,
            looping: true,
            speed: 1.0,
            drag: None,
        }
    }
}

/// One drawn animation block: geometry plus the statement index that owns it.
struct Block {
    rect: Rect,
    start: f32,
    run: f32,
    play_idx: usize,
    label: String,
    fill: Color32,
    stroke: Color32,
}

impl TimelinePanel {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        timeline: Option<&mut Timeline>,
        origins: &[mo_dsl::PlayOrigin],
        current_time: &mut f32,
    ) -> Option<TimelineAction> {
        let mut action = None;
        let tl = timeline?;

        let total_duration = tl.duration().max(0.001);
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

            // "Apply timings back to code" action
            if ui
                .small_button("💾 Apply Timings to Code")
                .on_hover_text("Rewrite the Ns durations of play statements in the .moa source")
                .clicked()
            {
                let run_times: Vec<f32> = tl
                    .stmts
                    .iter()
                    .filter_map(|s| match s {
                        Stmt::Play(a) => Some(a.run_time),
                        _ => None,
                    })
                    .collect();
                action = Some(TimelineAction::ApplyTimings(run_times));
            }
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

        // Visual animation tracks (sequencer view). Vertically scrollable so
        // every track is reachable; drag_to_scroll is off because the painter's
        // click_and_drag sense owns horizontal block drags (move/trim/seek).
        let track_height = 22.0;
        let track_infos = collect_tracks(tl, origins);
        if track_infos.is_empty() {
            return None;
        }
        let total_track_area_height =
            (track_infos.len() as f32) * (track_height + 4.0) + 10.0;

        egui::ScrollArea::vertical()
            .id_salt("timeline_tracks_scroll")
            .auto_shrink([false, false])
            .drag_to_scroll(false)
            .show(ui, |ui| {
        let (response, painter) = ui.allocate_painter(
            egui::vec2(ui.available_width(), total_track_area_height),
            egui::Sense::click_and_drag(),
        );

        let rect = response.rect;

        // Draw track area background
        painter.rect_filled(rect, 4.0, Color32::from_rgb(25, 28, 36));
        painter.rect_stroke(rect, 4.0, Stroke::new(1.0_f32, Color32::from_rgb(45, 50, 65)), egui::StrokeKind::Inside);

        // Lay out blocks in the (possibly drag-mutated) timeline.
        let blocks = layout_blocks(tl, origins, rect, track_height);
        let px_to_sec = |px: f32| ((px - rect.left()) / rect.width().max(1.0)) * total_duration;

        let pointer = response.interact_pointer_pos();

        // Hit-test the pointer against block edges/bodies.
        let mut hit: Option<(usize, DragMode)> = None;
        if let Some(p) = pointer {
            for (i, b) in blocks.iter().enumerate() {
                if b.rect.contains(p) {
                    let mode = if p.x >= b.rect.right() - EDGE_GRAB {
                        DragMode::TrimR
                    } else {
                        DragMode::Move
                    };
                    hit = Some((i, mode));
                    break;
                }
            }
        }

        // Cursor feedback: resize near right edges, grab over bodies.
        if self.drag.is_some() {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        } else {
            ui.ctx().set_cursor_icon(match hit {
                Some((_, DragMode::TrimR)) => CursorIcon::ResizeEast,
                Some(_) => CursorIcon::Grab,
                None => CursorIcon::Default,
            });
        }

        // Start a drag on press over a block.
        if self.drag.is_none() && response.is_pointer_button_down_on() {
            if let (Some((i, mode)), Some(p)) = (hit, pointer) {
                let b = &blocks[i];
                let grab_offset = (px_to_sec(p.x) - b.start).clamp(0.0, b.run);
                let mut play_idx = b.play_idx;
                let wait_idx = if mode == DragMode::Move {
                    // Guarantee a preceding Wait to trade time with.
                    if play_idx > 0 {
                        if let Stmt::Wait(_) = tl.stmts[play_idx - 1] {
                            play_idx - 1
                        } else {
                            tl.stmts.insert(play_idx, Stmt::Wait(0.0));
                            play_idx += 1; // the Play shifted right by one
                            play_idx - 1
                        }
                    } else {
                        tl.stmts.insert(0, Stmt::Wait(0.0));
                        play_idx += 1;
                        0
                    }
                } else {
                    0 // unused in TrimR mode
                };
                self.drag = Some(TrackDrag {
                    play_idx,
                    wait_idx,
                    orig_run: b.run,
                    grab_offset,
                    mode,
                });
                self.playing = false;
            }
        }

        // Apply the active drag each frame; keep it alive until release.
        let mut drag_alive = false;
        if let Some(d) = self.drag.take() {
            if response.dragged() {
                if let Some(p) = pointer {
                    let pointer_t = px_to_sec(p.x);
                    match d.mode {
                        DragMode::Move => {
                            // Everything before the Wait is a fixed prefix; the
                            // wait itself is the block's only positional freedom.
                            let cur_start = stmt_start(tl, d.play_idx);
                            let desired = pointer_t - d.grab_offset;
                            let prefix = cur_start - cur_wait(tl, d.wait_idx);
                            let new_wait = (desired - prefix).max(0.0);
                            if (new_wait - cur_wait(tl, d.wait_idx)).abs() > 1e-4 {
                                tl.stmts[d.wait_idx] = Stmt::Wait(new_wait);
                                *current_time = (*current_time).min(tl.duration().max(0.0));
                                action = Some(TimelineAction::Edited);
                            }
                        }
                        DragMode::TrimR => {
                            let start = stmt_start(tl, d.play_idx);
                            let new_run = (pointer_t - start).max(MIN_RUN);
                            if (new_run - d.orig_run).abs() > 1e-4 {
                                if let Some(Stmt::Play(a)) = tl.stmts.get_mut(d.play_idx) {
                                    a.run_time = new_run;
                                }
                                *current_time = (*current_time).min(tl.duration().max(0.0));
                                action = Some(TimelineAction::Edited);
                            }
                        }
                    }
                }
                drag_alive = true;
            }
            if drag_alive {
                self.drag = Some(d);
            }
        }

        // Seek by clicking/dragging empty track space (not while block-dragging).
        if self.drag.is_none() && hit.is_none() && (response.clicked() || response.dragged()) {
            if let Some(pos) = pointer {
                let frac = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                *current_time = frac * total_duration;
                self.playing = false;
            }
        }

        // Draw each animation block (recomputed after any drag mutation).
        let blocks = layout_blocks(tl, origins, rect, track_height);
        for b in blocks.iter() {
            let is_active = *current_time >= b.start && *current_time <= b.start + b.run;
            let final_bg = if is_active {
                Color32::from_rgb(
                    (b.fill.r() as f32 * 1.4).min(255.0) as u8,
                    (b.fill.g() as f32 * 1.4).min(255.0) as u8,
                    (b.fill.b() as f32 * 1.4).min(255.0) as u8,
                )
            } else {
                b.fill
            };

            painter.rect_filled(b.rect, 3.0, final_bg);
            painter.rect_stroke(
                b.rect,
                3.0,
                Stroke::new(if is_active { 1.5_f32 } else { 1.0_f32 }, b.stroke),
                egui::StrokeKind::Inside,
            );

            // Right-edge trim grip
            let grip = Rect::from_min_size(
                Pos2::new(b.rect.right() - 3.0, b.rect.top() + 4.0),
                Vec2::new(2.0, b.rect.height() - 8.0),
            );
            painter.rect_filled(grip, 1.0, b.stroke.gamma_multiply(0.6));

            if b.rect.width() > 30.0 {
                let text_pos = Pos2::new(b.rect.left() + 6.0, b.rect.top() + 4.0);
                painter.text(
                    text_pos,
                    egui::Align2::LEFT_TOP,
                    &b.label,
                    egui::FontId::proportional(11.0),
                    Color32::WHITE,
                );
            }
        }

        // Draw vertical playhead cursor. Clamp the dot's y into the visible
        // clip region so it stays visible when scrolled.
        let cursor_x = rect.left() + (*current_time / total_duration) * rect.width();
        painter.line_segment(
            [Pos2::new(cursor_x, rect.top()), Pos2::new(cursor_x, rect.bottom())],
            Stroke::new(2.0_f32, Color32::from_rgb(255, 255, 255)),
        );
        let clip = ui.clip_rect().intersect(rect);
        let dot_y = (rect.top() + 3.0).clamp(clip.top() + 3.0, clip.bottom().max(clip.top() + 3.0) - 3.0);
        painter.circle_filled(
            Pos2::new(cursor_x, dot_y),
            4.0,
            Color32::from_rgb(252, 98, 85),
        );
        });

        action
    }
}

struct TrackInfo {
    start: f32,
    run: f32,
    play_idx: usize,
    label: String,
    fill: Color32,
    stroke: Color32,
}

/// Asset file stem for a play origin, e.g. `euler` for `assets/euler_formula.moa`.
fn origin_tag(o: &mo_dsl::PlayOrigin) -> Option<String> {
    o.file
        .as_ref()
        .and_then(|f| f.file_stem())
        .map(|s| s.to_string_lossy().into_owned())
}

/// Collect animation blocks with absolute start times and statement indices.
fn collect_tracks(tl: &Timeline, origins: &[mo_dsl::PlayOrigin]) -> Vec<TrackInfo> {
    let mut list = Vec::new();
    let mut t = 0.0;
    let mut play_n = 0usize;
    for (idx, s) in tl.stmts.iter().enumerate() {
        match s {
            Stmt::Play(a) => {
                let (mut label, fill, stroke) = anim_style(a);
                // Prefix imported plays with their asset name: `euler: write(...)`
                if let Some(tag) = origins.get(play_n).and_then(origin_tag) {
                    label = format!("{tag}: {label}");
                }
                play_n += 1;
                list.push(TrackInfo {
                    start: t,
                    run: a.run_time,
                    play_idx: idx,
                    label,
                    fill,
                    stroke,
                });
                t += a.run_time;
            }
            Stmt::Wait(d) => t += d,
            Stmt::Decl { .. } => {}
        }
    }
    list
}

/// Convert track infos into laid-out blocks within the painter rect.
fn layout_blocks(tl: &Timeline, origins: &[mo_dsl::PlayOrigin], rect: Rect, track_height: f32) -> Vec<Block> {
    let total = tl.duration().max(0.001);
    collect_tracks(tl, origins)
        .into_iter()
        .enumerate()
        .map(|(i, t)| {
            let y_top = rect.top() + 6.0 + (i as f32) * (track_height + 4.0);
            let start_x = rect.left() + (t.start / total) * rect.width();
            let end_x = rect.left() + ((t.start + t.run) / total) * rect.width();
            Block {
                rect: Rect::from_min_max(
                    Pos2::new(start_x, y_top),
                    Pos2::new(end_x.max(start_x + 4.0), y_top + track_height),
                ),
                start: t.start,
                run: t.run,
                play_idx: t.play_idx,
                label: t.label,
                fill: t.fill,
                stroke: t.stroke,
            }
        })
        .collect()
}

/// Absolute start time of the Play statement at `idx`.
fn stmt_start(tl: &Timeline, idx: usize) -> f32 {
    let mut t = 0.0;
    for (i, s) in tl.stmts.iter().enumerate() {
        if i == idx {
            break;
        }
        match s {
            Stmt::Play(a) => t += a.run_time,
            Stmt::Wait(d) => t += d,
            Stmt::Decl { .. } => {}
        }
    }
    t
}

fn cur_wait(tl: &Timeline, idx: usize) -> f32 {
    match tl.stmts.get(idx) {
        Some(Stmt::Wait(w)) => *w,
        _ => 0.0,
    }
}

fn anim_style(a: &TimedAnim) -> (String, Color32, Color32) {
    let label = match &a.anim {
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
        mo_core::anim::Animation::Uncreate { id } => format!("Uncreate({})", id),
        mo_core::anim::Animation::DrawBorderThenFill { id } => format!("DrawBorder({})", id),
        mo_core::anim::Animation::Circumscribe { id, .. } => format!("Circumscribe({})", id),
        mo_core::anim::Animation::Flash { id, .. } => format!("Flash({})", id),
        mo_core::anim::Animation::Scale { id, factor } => format!("Scale({}, {:.1}x)", id, factor),
        mo_core::anim::Animation::Shift { id, delta } => format!("Shift({}, {:.1}, {:.1})", id, delta.x, delta.y),
        mo_core::anim::Animation::CameraOrbit { phi, theta } => {
            format!("Orbit({:.0}\u{b0}, {:.0}\u{b0})", phi.to_degrees(), theta.to_degrees())
        }
        mo_core::anim::Animation::CameraZoom { factor } => format!("Zoom({:.2}x)", factor),
        mo_core::anim::Animation::CameraMoveTo { x, y } => format!("MoveTo({:.1}, {:.1})", x, y),
    };
    let (fill, stroke) = match &a.anim {
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
    (label, fill, stroke)
}

/// Rewrite the `Ns` durations of `play` statements in .moa source, in play
/// order. Used for the master's own plays and for each imported asset file.
fn rewrite_play_durations(code: &str, run_times: &[f32]) -> Option<String> {
    let mut next = 0usize;
    let mut modified = false;
    let mut lines: Vec<String> = code.lines().map(|s| s.to_string()).collect();

    for line in &mut lines {
        if next >= run_times.len() {
            break;
        }
        let trimmed = line.trim();
        if trimmed.starts_with("//") || !trimmed.starts_with("play ") {
            continue;
        }
        // Find the first `<digits>[.<digits>]s` after "play ".
        let bytes = line.as_bytes();
        let mut i = 4; // past "play"
        let mut span: Option<(usize, usize)> = None;
        while i < bytes.len() {
            let c = bytes[i] as char;
            if c.is_ascii_digit() || c == '.' {
                let start = i;
                while i < bytes.len() {
                    let ch = bytes[i] as char;
                    if ch.is_ascii_digit() || ch == '.' {
                        i += 1;
                    } else {
                        break;
                    }
                }
                if i < bytes.len() && bytes[i] == b's' {
                    span = Some((start, i));
                }
                break;
            }
            i += 1;
        }
        if let Some((s, e)) = span {
            let new_dur = format!("{:.2}", run_times[next]);
            let new_dur = new_dur.trim_end_matches('0').trim_end_matches('.');
            let mut new_line = String::from(&line[..s]);
            new_line.push_str(new_dur);
            new_line.push('s');
            new_line.push_str(&line[e + 1..]);
            *line = new_line;
            modified = true;
        }
        next += 1;
    }

    if modified {
        let mut res = lines.join("\n");
        if code.ends_with('\n') {
            res.push('\n');
        }
        Some(res)
    } else {
        None
    }
}

/// Rewrite run times across the master source AND imported asset files, using
/// the per-play provenance recorded by the importer. Master plays are rewritten
/// positionally in the master's own `play` lines; each imported asset's plays
/// are rewritten positionally inside that asset file. Returns the new master
/// code plus (path, new content) pairs to write back to disk.
pub fn apply_timings_multi(
    master_code: &str,
    origins: &[mo_dsl::PlayOrigin],
    run_times: &[f32],
) -> Option<(String, Vec<(std::path::PathBuf, String)>)> {
    if origins.len() != run_times.len() {
        return None; // stale origins — caller recompiled out of sync
    }

    // Master's own plays, in flat (= document) order.
    let master_runs: Vec<f32> = origins
        .iter()
        .zip(run_times)
        .filter_map(|(o, r)| o.file.is_none().then_some(*r))
        .collect();

    // Group imported plays by file, ordered by their local play index.
    let mut per_file: std::collections::BTreeMap<std::path::PathBuf, Vec<(usize, f32)>> =
        Default::default();
    for (o, r) in origins.iter().zip(run_times) {
        if let Some(f) = &o.file {
            per_file.entry(f.clone()).or_default().push((o.local_play_idx, *r));
        }
    }

    let new_master = rewrite_play_durations(master_code, &master_runs);

    let mut asset_writes = Vec::new();
    for (path, mut runs) in per_file {
        runs.sort_by_key(|(idx, _)| *idx);
        let durations: Vec<f32> = runs.into_iter().map(|(_, r)| r).collect();
        let content = std::fs::read_to_string(&path).ok()?;
        if let Some(new_content) = rewrite_play_durations(&content, &durations) {
            asset_writes.push((path, new_content));
        }
    }

    Some((
        new_master.unwrap_or_else(|| master_code.to_string()),
        asset_writes,
    ))
}

#[cfg(test)]
mod tests {
    use super::{apply_timings_multi, layout_blocks};
    use crate::ext::egui;

    #[test]
    fn layout_blocks_exposes_every_track_without_cap() {
        // 10 plays used to be capped at 8 visible tracks.
        let plays: Vec<mo_core::scene::Stmt> = (0..10)
            .map(|i| {
                mo_core::scene::Stmt::Play(mo_core::anim::TimedAnim::new(
                    mo_core::anim::Animation::ShowCreation { id: format!("c{i}") },
                    1.0,
                    "smooth",
                ))
            })
            .collect();
        let mut tl = mo_core::scene::Timeline::default();
        tl.stmts = plays;
        let origins = vec![mo_dsl::PlayOrigin { file: None, local_play_idx: 0 }; 10];
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 10_000.0));
        let blocks = layout_blocks(&tl, &origins, rect, 22.0);
        assert_eq!(blocks.len(), 10, "all tracks must be laid out (no 8-cap)");
        for pair in blocks.windows(2) {
            assert!(pair[1].rect.top() > pair[0].rect.bottom());
        }
    }

    #[test]
    fn rewrites_play_durations_in_order() {
        let code = "circle c at (0, 0) r=2;\nplay showCreation(c) 2s smooth;\nwait 0.5s;\nplay fadeIn(c) 1s;\n";
        let origins = vec![
            mo_dsl::PlayOrigin { file: None, local_play_idx: 0 },
            mo_dsl::PlayOrigin { file: None, local_play_idx: 0 },
        ];
        let (out, assets) = apply_timings_multi(code, &origins, &[3.5, 0.75]).unwrap();
        assert!(assets.is_empty());
        assert!(out.contains("play showCreation(c) 3.5s smooth;"));
        assert!(out.contains("play fadeIn(c) 0.75s;"));
        assert!(out.contains("wait 0.5s;")); // waits untouched
    }

    #[test]
    fn applies_across_imported_asset_files() {
        let asset_file = std::env::temp_dir().join("test_apply_asset.moa");
        std::fs::write(&asset_file, "circle s r=1;\nplay showCreation(s) 1s;\nplay fadeIn(s) 1s;\n")
            .expect("write temp asset");

        let master = format!(
            "import \"{}\" as a;\ncircle m r=1;\nplay showCreation(m) 2s;\n",
            asset_file.to_str().unwrap()
        );
        let (tl, origins) = mo_dsl::parse_with_origins(&master).expect("parses master");
        // flat plays: asset's 2 plays, then master's 1
        let run_times: Vec<f32> = tl
            .stmts
            .iter()
            .filter_map(|s| match s {
                mo_core::scene::Stmt::Play(a) => Some(a.run_time),
                _ => None,
            })
            .collect();
        assert_eq!(run_times.len(), 3);

        let trimmed: Vec<f32> = vec![run_times[0], 0.4, run_times[2]];
        let (new_master, assets) = apply_timings_multi(&master, &origins, &trimmed).unwrap();

        // asset file's second play rewritten to 0.4s; master untouched
        assert!(assets.len() == 1 && assets[0].0 == asset_file);
        let asset_out = &assets[0].1;
        assert!(asset_out.contains("play fadeIn(s) 0.4s;"));
        assert!(asset_out.contains("play showCreation(s) 1s;"));
        assert!(new_master.contains("play showCreation(m) 2s;"));

        let _ = std::fs::remove_file(asset_file);
    }

    #[test]
    fn round_trips_through_parser() {
        let code = "circle c at (0,0) r=2;\nplay showCreation(c) 2s smooth;\n";
        let origins = vec![mo_dsl::PlayOrigin { file: None, local_play_idx: 0 }];
        let (out, _) = apply_timings_multi(code, &origins, &[1.25]).unwrap();
        let tl = mo_dsl::parse_or_report(&out).expect("recompiled");
        assert!((tl.duration() - 1.25).abs() < 1e-3);
    }
}
