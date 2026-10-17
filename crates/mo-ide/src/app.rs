use crate::assets::{AssetAction, AssetPanel};
use crate::editor::EditorPanel;
use crate::export::{export_moab, export_mp4};
use crate::inspector::{InspectorAction, InspectorPanel};
use crate::scene_tree::SceneTreePanel;
use crate::timeline::TimelinePanel;
use crate::viewport::ViewportPanel;
use crate::watcher::FileWatcher;
use eframe::egui::{self, Color32};
use mo_core::camera::Camera;
use mo_core::scene::{Player, Timeline};
use mo_render::Renderer;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

pub struct MathOxideApp {
    render_state: egui_wgpu::RenderState,
    renderer: Renderer,
    viewport_panel: ViewportPanel,
    editor_panel: EditorPanel,
    asset_panel: AssetPanel,
    timeline_panel: TimelinePanel,

    file_path: Option<PathBuf>,
    watcher: Option<FileWatcher>,

    timeline: Option<Timeline>,
    play_origins: Vec<mo_dsl::PlayOrigin>,
    camera: Camera,
    current_time: f32,
    selected_mobject: Option<String>,
    overrides: HashMap<String, (f32, f32)>,

    // View toggles
    show_editor: bool,
    show_assets: bool,
    show_scene_tree: bool,
    show_inspector: bool,
    show_timeline: bool,

    // Left panel active tab (0: Editor, 1: Assets, 2: Scene Tree)
    left_tab: usize,

    // Timing and FPS
    last_frame_instant: Instant,
    fps: f32,
    frame_count: u32,
    fps_timer: Instant,

    // Status toaster / notification
    status_message: Option<(String, Instant)>,
}

impl MathOxideApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_file: Option<PathBuf>) -> anyhow::Result<Self> {
        let render_state = cc
            .wgpu_render_state
            .clone()
            .ok_or_else(|| anyhow::anyhow!("wgpu render state required"))?;

        let device = render_state.device.clone();
        let queue = render_state.queue.clone();

        let initial_w = 1280;
        let initial_h = 720;
        let renderer = Renderer::new(device, queue, initial_w, initial_h)?;
        let viewport_panel = ViewportPanel::new(&render_state, &renderer);

        let mut camera = Camera::default();
        camera.width = initial_w as f32;
        camera.height = initial_h as f32;
        camera.frame_height = 8.0;

        let project_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let asset_panel = AssetPanel::new(project_dir);

        let path = initial_file.unwrap_or_else(|| PathBuf::from("examples/master_video.moa"));
        let (code, actual_path, watcher) = if path.exists() {
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            let w = FileWatcher::new(&path).ok();
            (content, Some(path), w)
        } else if PathBuf::from("examples/demo.moa").exists() {
            let p = PathBuf::from("examples/demo.moa");
            let content = std::fs::read_to_string(&p).unwrap_or_default();
            let w = FileWatcher::new(&p).ok();
            (content, Some(p), w)
        } else {
            let starter = default_starter_code();
            (starter, None, None)
        };

        let mut editor_panel = EditorPanel::new(code.clone());
        let (timeline, play_origins, err) = compile_code(&code);
        editor_panel.set_error(err);

        Ok(Self {
            render_state,
            renderer,
            viewport_panel,
            editor_panel,
            asset_panel,
            timeline_panel: TimelinePanel::default(),
            file_path: actual_path,
            watcher,
            timeline,
            play_origins,
            camera,
            current_time: 0.0,
            selected_mobject: None,
            overrides: HashMap::new(),
            show_editor: true,
            show_assets: true,
            show_scene_tree: true,
            show_inspector: true,
            show_timeline: true,
            left_tab: 0,
            last_frame_instant: Instant::now(),
            fps: 60.0,
            frame_count: 0,
            fps_timer: Instant::now(),
            status_message: Some(("MathOxide Studio ready — pair programming environment active".into(), Instant::now())),
        })
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), Instant::now()));
    }

    pub fn open_file(&mut self, path: PathBuf) {
        if let Ok(content) = std::fs::read_to_string(&path) {
            self.file_path = Some(path.clone());
            self.editor_panel.code = content.clone();
            self.editor_panel.is_dirty = false;
            let (tl, origins, err) = compile_code(&content);
                self.play_origins = origins;
            self.timeline = tl;
            self.editor_panel.set_error(err);
            self.overrides.clear();
            self.current_time = 0.0;
            self.camera.reset();
            if let Some(w) = &mut self.watcher {
                let _ = w.watch_file(&path);
            } else {
                self.watcher = FileWatcher::new(&path).ok();
            }
            self.set_status(format!("Opened {}", path.display()));
        } else {
            self.set_status(format!("Failed to read {}", path.display()));
        }
    }

    pub fn open_file_dialog(&mut self) {
        let initial_dir = self
            .file_path
            .as_ref()
            .and_then(|p| p.parent())
            .unwrap_or(&self.asset_panel.project_path);

        if let Some(path) = rfd::FileDialog::new()
            .set_directory(initial_dir)
            .add_filter("MathOxide Scene", &["moa", "moab"])
            .pick_file()
        {
            self.open_file(path);
        }
    }

    pub fn open_folder_dialog(&mut self) {
        if let Some(folder) = rfd::FileDialog::new()
            .set_directory(&self.asset_panel.project_path)
            .pick_folder()
        {
            self.asset_panel.project_path = folder.clone();
            self.asset_panel.rescan();
            self.set_status(format!("Active project: {}", folder.display()));
        }
    }

    pub fn save_as(&mut self) {
        let initial_dir = self
            .file_path
            .as_ref()
            .and_then(|p| p.parent())
            .unwrap_or(&self.asset_panel.project_path);

        let default_name = self
            .file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|f| f.to_str())
            .unwrap_or("scene.moa");

        if let Some(path) = rfd::FileDialog::new()
            .set_directory(initial_dir)
            .set_file_name(default_name)
            .add_filter("MathOxide Animation", &["moa"])
            .save_file()
        {
            if let Err(e) = std::fs::write(&path, &self.editor_panel.code) {
                self.set_status(format!("Save error: {}", e));
            } else {
                self.file_path = Some(path.clone());
                self.editor_panel.is_dirty = false;
                if let Some(w) = &mut self.watcher {
                    let _ = w.watch_file(&path);
                } else {
                    self.watcher = FileWatcher::new(&path).ok();
                }
                self.asset_panel.rescan();
                self.set_status(format!("Saved to {}", path.display()));
            }
        }
    }

    pub fn save_current_file(&mut self) {
        if let Some(path) = self.file_path.clone() {
            if let Err(e) = std::fs::write(&path, &self.editor_panel.code) {
                self.set_status(format!("Error saving: {}", e));
            } else {
                self.editor_panel.is_dirty = false;
                self.set_status(format!("Saved {}", path.display()));
                self.asset_panel.rescan();
            }
        } else {
            self.save_as();
        }
    }

    pub fn reload_from_disk(&mut self) {
        if let Some(path) = &self.file_path {
            if let Ok(content) = std::fs::read_to_string(path) {
                self.editor_panel.code = content.clone();
                self.editor_panel.is_dirty = false;
                let (tl, origins, err) = compile_code(&content);
                self.play_origins = origins;
                self.timeline = tl;
                self.editor_panel.set_error(err);
                self.set_status(format!("Reloaded {}", path.display()));
            }
        }
    }

    pub fn new_asset(&mut self) {
        let assets_dir = self.asset_panel.project_path.join("assets");
        let _ = std::fs::create_dir_all(&assets_dir);
        let mut idx = 1;
        let mut target = assets_dir.join(format!("asset_{idx}.moa"));
        while target.exists() {
            idx += 1;
            target = assets_dir.join(format!("asset_{idx}.moa"));
        }

        let template = new_asset_template();
        if let Err(e) = std::fs::write(&target, &template) {
            self.set_status(format!("Error creating asset: {e}"));
        } else {
            self.open_file(target);
            self.asset_panel.rescan();
            self.left_tab = 0;
            self.set_status(format!("Created new asset asset_{idx}.moa"));
        }
    }

    pub fn new_master(&mut self) {
        let mut idx = 1;
        let mut target = self.asset_panel.project_path.join(format!("composition_{idx}.moa"));
        while target.exists() {
            idx += 1;
            target = self.asset_panel.project_path.join(format!("composition_{idx}.moa"));
        }

        let template = new_master_template();
        if let Err(e) = std::fs::write(&target, &template) {
            self.set_status(format!("Error creating composition: {e}"));
        } else {
            self.open_file(target);
            self.asset_panel.rescan();
            self.left_tab = 0;
            self.set_status(format!("Created new composition composition_{idx}.moa"));
        }
    }
}

impl eframe::App for MathOxideApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_instant).as_secs_f32();
        self.last_frame_instant = now;

        // Calculate smoothed FPS
        self.frame_count += 1;
        if now.duration_since(self.fps_timer).as_secs_f32() >= 0.5 {
            self.fps = (self.frame_count as f32) / now.duration_since(self.fps_timer).as_secs_f32();
            self.frame_count = 0;
            self.fps_timer = now;
        }

        // Poll file watcher for disk changes
        if let Some(watcher) = &mut self.watcher {
            if watcher.poll_changed() {
                self.reload_from_disk();
            }
        }

        // Advance timeline playback if playing
        if self.timeline_panel.playing {
            if let Some(tl) = &self.timeline {
                let dur = tl.duration();
                self.current_time += dt * self.timeline_panel.speed;
                if self.current_time > dur {
                    if self.timeline_panel.looping {
                        self.current_time = 0.0;
                    } else {
                        self.current_time = dur;
                        self.timeline_panel.playing = false;
                    }
                }
            }
            ctx.request_repaint();
        }

        // Global Keyboard shortcuts
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Space) {
                self.timeline_panel.playing = !self.timeline_panel.playing;
            }
            if i.key_pressed(egui::Key::Home) {
                self.current_time = 0.0;
            }
            if i.key_pressed(egui::Key::ArrowRight) {
                self.timeline_panel.playing = false;
                let dur = self.timeline.as_ref().map(|t| t.duration()).unwrap_or(0.0);
                self.current_time = (self.current_time + 1.0 / 60.0).min(dur);
            }
            if i.key_pressed(egui::Key::ArrowLeft) {
                self.timeline_panel.playing = false;
                self.current_time = (self.current_time - 1.0 / 60.0).max(0.0);
            }
            if i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(egui::Key::S) {
                self.save_as();
            } else if i.modifiers.ctrl && i.key_pressed(egui::Key::S) {
                self.save_current_file();
            }
            if i.modifiers.ctrl && i.key_pressed(egui::Key::O) {
                self.open_file_dialog();
            }
            if i.modifiers.ctrl && i.key_pressed(egui::Key::N) {
                self.new_asset();
            }
            if i.key_pressed(egui::Key::R) && !i.modifiers.any() {
                self.camera.reset();
                self.set_status("Camera reset to 2D default");
            }
        });

        // Top Menu Bar
        egui::TopBottomPanel::top("top_menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Asset (Ctrl+N)").clicked() {
                        self.new_asset();
                        ui.close_menu();
                    }
                    if ui.button("New Master Composition...").clicked() {
                        self.new_master();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Open File... (Ctrl+O)").clicked() {
                        self.open_file_dialog();
                        ui.close_menu();
                    }
                    if ui.button("Open Project Folder...").clicked() {
                        self.open_folder_dialog();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Save (Ctrl+S)").clicked() {
                        self.save_current_file();
                        ui.close_menu();
                    }
                    if ui.button("Save As... (Ctrl+Shift+S)").clicked() {
                        self.save_as();
                        ui.close_menu();
                    }
                    if ui.button("Reload from Disk").clicked() {
                        self.reload_from_disk();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Export MP4 Video...").clicked() {
                        if let Some(tl) = &self.timeline {
                            let tl_clone = tl.clone();
                            if let Some(out_path) = rfd::FileDialog::new()
                                .add_filter("MP4 Video", &["mp4"])
                                .set_file_name("render.mp4")
                                .save_file()
                            {
                                let dest_str = out_path.display().to_string();
                                std::thread::spawn(move || {
                                    let _ = export_mp4(&tl_clone, &out_path, 1920, 1080, 60, |_, _| {});
                                });
                                self.set_status(format!("Started background MP4 export -> {}", dest_str));
                            }
                        } else {
                            self.set_status("Cannot export: scene is empty or has compilation errors");
                        }
                        ui.close_menu();
                    }
                    if ui.button("Export .moab Bundle...").clicked() {
                        if let Some(out_path) = rfd::FileDialog::new()
                            .add_filter("MOAB Package", &["moab"])
                            .set_file_name("composition.moab")
                            .save_file()
                        {
                            if let Err(e) = export_moab(&self.editor_panel.code, &out_path) {
                                self.set_status(format!("MOAB Export error: {}", e));
                            } else {
                                self.set_status(format!("Exported to {}", out_path.display()));
                            }
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.show_editor, "Code Editor");
                    ui.checkbox(&mut self.show_assets, "Asset Browser");
                    ui.checkbox(&mut self.show_scene_tree, "Scene Tree");
                    ui.checkbox(&mut self.show_inspector, "Inspector");
                    ui.checkbox(&mut self.show_timeline, "Timeline");
                    ui.separator();
                    if ui.button("Reset Camera (R)").clicked() {
                        self.camera.reset();
                        ui.close_menu();
                    }
                });

                ui.menu_button("Presets", |ui| {
                    if ui.button("Master Video Composition (examples/master_video.moa)").clicked() {
                        self.open_file(PathBuf::from("examples/master_video.moa"));
                        ui.close_menu();
                    }
                    if ui.button("3D Orbit Showcase (examples/demo_3d.moa)").clicked() {
                        self.open_file(PathBuf::from("examples/demo_3d.moa"));
                        ui.close_menu();
                    }
                    if ui.button("LaTeX Showcase (examples/demo_latex.moa)").clicked() {
                        self.open_file(PathBuf::from("examples/demo_latex.moa"));
                        ui.close_menu();
                    }
                    if ui.button("Demo Scene (examples/demo.moa)").clicked() {
                        self.open_file(PathBuf::from("examples/demo.moa"));
                        ui.close_menu();
                    }
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some((msg, created)) = &self.status_message {
                        if now.duration_since(*created).as_secs_f32() < 4.5 {
                            ui.colored_label(Color32::from_rgb(125, 207, 255), msg);
                        }
                    }
                    ui.separator();
                    let file_label = self
                        .file_path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .and_then(|f| f.to_str())
                        .unwrap_or("Untitled");

                    ui.label(
                        egui::RichText::new(format!("[{file_label}]"))
                            .monospace()
                            .size(11.0)
                            .color(Color32::from_rgb(140, 160, 190)),
                    );
                });
            });
        });

        // Bottom Timeline Panel
        if self.show_timeline {
            egui::TopBottomPanel::bottom("bottom_timeline_panel")
                .resizable(true)
                .default_height(110.0)
                .min_height(75.0)
                .show(ctx, |ui| {
                    let mut panel = std::mem::take(&mut self.timeline_panel);
                    let tl_action = panel.show(
                        ui,
                        self.timeline.as_mut(),
                        &self.play_origins,
                        &mut self.current_time,
                    );
                    self.timeline_panel = panel;
                    match tl_action {
                        Some(crate::timeline::TimelineAction::Edited) => {
                            self.editor_panel.is_dirty = true;
                        }
                        Some(crate::timeline::TimelineAction::ApplyTimings(run_times)) => {
                            match mo_dsl::parse_with_origins(&self.editor_panel.code) {
                                Ok((_, origins))
                                    if origins.len() == run_times.len() =>
                                {
                                    match crate::timeline::apply_timings_multi(
                                        &self.editor_panel.code,
                                        &origins,
                                        &run_times,
                                    ) {
                                        Some((new_code, asset_writes)) => {
                                            let mut written = 0;
                                            for (path, content) in &asset_writes {
                                                if std::fs::write(path, content).is_ok() {
                                                    written += 1;
                                                }
                                            }
                                            self.editor_panel.code = new_code;
                                            let (tl, origins, err) = compile_code(&self.editor_panel.code);
                self.play_origins = origins;
                                            if let Some(t) = tl {
                                                self.timeline = Some(t);
                                            }
                                            self.editor_panel.set_error(err);
                                            self.editor_panel.is_dirty = true;
                                            if written > 0 {
                                                self.set_status(format!(
                                                    "Timings applied: master + {written} asset file(s) updated"
                                                ));
                                            } else {
                                                self.set_status("Timeline timings written to code");
                                            }
                                        }
                                        None => {
                                            self.set_status("Apply failed: could not read/rewrite an imported asset file");
                                        }
                                    }
                                }
                                _ => {
                                    self.set_status("Cannot apply: run times and plays out of sync — recompile first");
                                }
                            }
                        }
                        None => {}
                    }
                });
        }

        // Left Panel: Tabs for Editor, Assets, and Scene Tree
        if self.show_editor || self.show_assets || self.show_scene_tree {
            egui::SidePanel::left("left_side_panel")
                .resizable(true)
                .default_width(320.0)
                .min_width(220.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if self.show_editor && ui.selectable_label(self.left_tab == 0, "📝 Editor").clicked() {
                            self.left_tab = 0;
                        }
                        if self.show_assets && ui.selectable_label(self.left_tab == 1, "📁 Assets").clicked() {
                            self.left_tab = 1;
                        }
                        if self.show_scene_tree && ui.selectable_label(self.left_tab == 2, "🌳 Scene Tree").clicked() {
                            self.left_tab = 2;
                        }
                    });
                    ui.separator();

                    match self.left_tab {
                        0 if self.show_editor => {
                            let (code_changed, save_req) = self.editor_panel.show(ui);
                            if save_req {
                                self.save_current_file();
                            }
                            if code_changed && self.editor_panel.auto_compile {
                                let (tl, origins, err) = compile_code(&self.editor_panel.code);
                self.play_origins = origins;
                                if let Some(t) = tl {
                                    self.timeline = Some(t);
                                }
                                self.editor_panel.set_error(err);
                            }
                        }
                        1 if self.show_assets => {
                            let action = self.asset_panel.show(ui, self.file_path.as_deref());
                            match action {
                                AssetAction::Open(path) => {
                                    self.open_file(path);
                                }
                                AssetAction::InsertImport { path, alias } => {
                                    let stmt = format!("import \"{path}\" as {alias} at (0, 0);\n");
                                    self.editor_panel.code.push_str(&stmt);
                                    let (tl, origins, err) = compile_code(&self.editor_panel.code);
                self.play_origins = origins;
                                    if let Some(t) = tl {
                                        self.timeline = Some(t);
                                    }
                                    self.editor_panel.set_error(err);
                                    self.editor_panel.is_dirty = true;
                                    self.set_status(format!("Imported '{alias}' into scene"));
                                }
                                AssetAction::NewAsset => {
                                    self.new_asset();
                                }
                                AssetAction::NewMaster => {
                                    self.new_master();
                                }
                                AssetAction::ChangeProjectDir(folder) => {
                                    self.set_status(format!("Project directory: {}", folder.display()));
                                }
                                AssetAction::None => {}
                            }
                        }
                        2 if self.show_scene_tree => {
                            let current_state = if let Some(tl) = &self.timeline {
                                Player::state_at(tl, self.current_time)
                            } else {
                                HashMap::new()
                            };
                            SceneTreePanel::show(
                                ui,
                                self.timeline.as_ref(),
                                &current_state,
                                &mut self.selected_mobject,
                            );
                        }
                        _ => {}
                    }
                });
        }

        // Right Panel: Inspector
        if self.show_inspector {
            egui::SidePanel::right("right_inspector_panel")
                .resizable(true)
                .default_width(260.0)
                .min_width(180.0)
                .show(ctx, |ui| {
                    let current_state = if let Some(tl) = &self.timeline {
                        Player::state_at(tl, self.current_time)
                    } else {
                        HashMap::new()
                    };
                    let inspector_action = InspectorPanel::show(
                        ui,
                        self.timeline.as_ref(),
                        &current_state,
                        &mut self.selected_mobject,
                        &mut self.overrides,
                        &mut self.camera,
                        self.current_time,
                    );

                    if let InspectorAction::ApplyShift { id, dx, dy } = inspector_action {
                        if let Some(updated_code) = apply_shift_to_dsl(&self.editor_panel.code, &id, dx, dy) {
                            self.editor_panel.code = updated_code;
                            self.overrides.remove(&id);
                            let (tl, origins, err) = compile_code(&self.editor_panel.code);
                self.play_origins = origins;
                            if let Some(t) = tl {
                                self.timeline = Some(t);
                            }
                            self.editor_panel.set_error(err);
                            self.editor_panel.is_dirty = true;
                            self.set_status(format!("Applied position ({:+.2}, {:+.2}) to code for '{}'", dx, dy, id));
                        }
                    }
                });
        }

        // Central Panel: GPU Viewport
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::from_rgb(15, 17, 23)))
            .show(ctx, |ui| {
                self.viewport_panel.show(
                    ui,
                    &self.render_state,
                    &mut self.renderer,
                    self.timeline.as_ref(),
                    self.current_time,
                    &mut self.camera,
                    &mut self.selected_mobject,
                    &mut self.overrides,
                    self.fps,
                );
            });
    }
}

fn apply_shift_to_dsl(code: &str, target_id: &str, dx: f32, dy: f32) -> Option<String> {
    let mut lines: Vec<String> = code.lines().map(|s| s.to_string()).collect();
    let mut modified = false;

    for line in &mut lines {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") {
            continue;
        }

        let words: Vec<&str> = trimmed.split_whitespace().collect();
        if words.len() >= 2 && words[1] == target_id {
            if let Some(at_idx) = line.find("at (") {
                if let Some(close_idx) = line[at_idx..].find(')') {
                    let full_close = at_idx + close_idx;
                    let inner = &line[at_idx + 4..full_close];
                    let parts: Vec<&str> = inner.split(',').collect();
                    if parts.len() == 2 {
                        let orig_x: f32 = parts[0].trim().parse().unwrap_or(0.0);
                        let orig_y: f32 = parts[1].trim().parse().unwrap_or(0.0);
                        let new_at = format!("at ({:.2}, {:.2})", orig_x + dx, orig_y + dy);
                        let mut new_line = line[..at_idx].to_string();
                        new_line.push_str(&new_at);
                        new_line.push_str(&line[full_close + 1..]);
                        *line = new_line;
                        modified = true;
                        break;
                    }
                }
            } else if let Some(id_pos) = line.find(target_id) {
                let insert_pos = id_pos + target_id.len();
                let new_at = format!(" at ({:.2}, {:.2})", dx, dy);
                let mut new_line = line[..insert_pos].to_string();
                new_line.push_str(&new_at);
                new_line.push_str(&line[insert_pos..]);
                *line = new_line;
                modified = true;
                break;
            }
        }
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

fn compile_code(code: &str) -> (Option<Timeline>, Vec<mo_dsl::PlayOrigin>, Option<String>) {
    match mo_dsl::parse_with_origins(code) {
        Ok((tl, origins)) => (Some(tl), origins, None),
        Err(e) => (None, Vec::new(), Some(e.iter().map(|d| format!("{}:{}: {}", d.line, d.col, d.message)).collect::<Vec<_>>().join("\n"))),
    }
}

fn default_starter_code() -> String {
    r#"// MathOxide demo scene — circle drawn, square fades in, circle morphs into it.
circle c at (0, 0) r=2 fill=#58C4DD33, stroke=blue, sw=6;
rect s at (3, 0) w=3, h=3 fill=#FC625533, stroke=red, sw=4;
play showCreation(c) 2s smooth;
wait 0.5s;
play fadeIn(s) 1s smooth;
play transform(c, s) 1.5s smooth;
wait 0.5s;
"#.into()
}

fn new_asset_template() -> String {
    r#"// MathOxide Component Asset
circle c at (0, 0) r=1.5 fill=#58C4DD33, stroke=cyan, sw=3;
tex label at (0, -2) text="f(x) = \sin(x)" size=0.8;
play showCreation(c) 1.5s smooth;
play fadeIn(label) 1.0s smooth;
wait 0.5s;
"#.into()
}

fn new_master_template() -> String {
    r#"// MathOxide Master Composition
// Import and sequence modular assets across the timeline
import "assets/euler_formula.moa" as euler at (0, 1.5);
import "assets/sine_wave.moa" as sine at (0, -1.5);

wait 1s;
"#.into()
}
