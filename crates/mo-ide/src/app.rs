use crate::assets::{AssetAction, AssetPanel};
use crate::editor::EditorPanel;
use crate::export::{export_moab, export_mp4};
use crate::inspector::{InspectorAction, InspectorPanel};
use crate::timeline::TimelinePanel;
use crate::viewport::ViewportPanel;
use crate::watcher::FileWatcher;
use crate::ext::eframe;
use crate::ext::egui::{self, Color32};
use crate::ext::egui_wgpu;
use mo_core::camera::Camera;
use mo_core::scene::{Player, Timeline};
use mo_render::Renderer;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
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
    /// Bumped on every timeline replacement (recompile, import, open file);
    /// invalidates the state cache and viewport frame signature.
    timeline_epoch: u64,
    /// LRU memo of sampled scene states keyed by (epoch, time bits) — one
    /// evaluation per unseen frame max, reused by inspector + viewport;
    /// scrub revisits become lookups. Entries share VGroups via Arc.
    frame_memo: mo_core::FrameMemo,
    /// Cached `TimelineIndex` keyed by timeline epoch.
    index_cache: Option<(u64, mo_core::TimelineIndex)>,
    camera: Camera,
    current_time: f32,
    selected_mobject: Option<String>,
    overrides: HashMap<String, (f32, f32)>,



    // Timing and FPS
    last_frame_instant: Instant,
    fps: f32,
    frame_count: u32,
    fps_timer: Instant,
    /// Pending auto-compile trigger (debounced — a full parse per keystroke
    /// is wasted work; 200ms of idle is imperceptible).
    compile_timer: Option<Instant>,

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

        let mut asset_panel = AssetPanel::new(crate::assets::src_home());
        asset_panel.restore_session();

        // Cyberpunk chrome: void purple panels, cyan selection/cursor.
        cc.egui_ctx.set_visuals({
            let mut v = cc.egui_ctx.style().visuals.clone();
            v.panel_fill = Color32::from_rgb(0x0A, 0x00, 0x2B);        // panels
            v.window_fill = Color32::from_rgb(0x0A, 0x00, 0x2B);
            v.extreme_bg_color = Color32::from_rgb(0x05, 0x00, 0x1A);  // text fields, deepest
            v.faint_bg_color = Color32::from_rgb(0x15, 0x00, 0x33);    // stripes / active-line tint
            v.selection.bg_fill = Color32::from_rgba_premultiplied(0x4A, 0x00, 0x88, 120); // deep royal violet
            v.selection.stroke = egui::Stroke::new(1.0_f32, Color32::from_rgb(0xFF, 0xE0, 0x66)); // warm gold
            let theme_widget = |w: &mut egui::style::WidgetVisuals| {
                w.bg_fill = Color32::from_rgb(0x15, 0x00, 0x33);
                w.weak_bg_fill = Color32::from_rgb(0x0F, 0x00, 0x28);
                w.fg_stroke.color = Color32::from_rgb(0xE8, 0xE0, 0xFF);
                w.bg_stroke.color = Color32::from_rgb(0x33, 0x00, 0x66); // borders
            };
            theme_widget(&mut v.widgets.noninteractive);
            theme_widget(&mut v.widgets.inactive);
            theme_widget(&mut v.widgets.hovered);
            theme_widget(&mut v.widgets.active);
            theme_widget(&mut v.widgets.open);
            v
        });

        // Most recently edited master across all video projects wins.
        let path = initial_file.or_else(|| {
            crate::assets::newest_master_project(&asset_panel.project_path)
                .map(|p| p.join("master.moa"))
                .filter(|p| p.exists())
        });
        let (code, actual_path, watcher) = if let Some(p) = path.filter(|p| p.exists()) {
            let content = std::fs::read_to_string(&p).unwrap_or_default();
            let w = FileWatcher::new(&p).ok();
            (content, Some(p), w)
        } else {
            let starter = default_starter_code();
            (starter, None, None)
        };

        let compile_base = actual_path
            .as_ref()
            .and_then(|p| p.parent())
            .unwrap_or(&asset_panel.project_path);
        let mut editor_panel = EditorPanel::new(code.clone());
        let (timeline, play_origins, err) = compile_code_in(&code, compile_base);
        editor_panel.set_error(err);

        let mut app = Self {
            render_state,
            renderer,
            viewport_panel,
            editor_panel,
            asset_panel,
            timeline_panel: TimelinePanel::default(),
            file_path: actual_path.clone(),
            watcher,
            timeline,
            play_origins,
            timeline_epoch: 0,
            frame_memo: mo_core::FrameMemo::new(16_384),
            index_cache: None,
            camera,
            current_time: 0.0,
            selected_mobject: None,
            overrides: HashMap::new(),
            last_frame_instant: Instant::now(),
            fps: 60.0,
            frame_count: 0,
            fps_timer: Instant::now(),
            compile_timer: None,
            status_message: Some(("MathOxide Studio ready — pair programming environment active".into(), Instant::now())),
        };
        if let Some(p) = actual_path {
            if let Some(proj) = crate::assets::project_of(&p, &app.asset_panel.project_path) {
                app.asset_panel.set_active_project(proj);
            }
        }
        Ok(app)
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), Instant::now()));
    }

    fn set_timeline(&mut self, tl: Option<Timeline>) {
        self.timeline_epoch += 1;
        self.timeline = tl;
    }

    /// Full DSL reparse (incl. imports from disk) + error surfacing.
    fn recompile(&mut self) {
        let (tl, origins, err) = compile_code_in(&self.editor_panel.code, &self.compile_base_dir());
        self.play_origins = origins;
        if let Some(t) = tl {
            self.set_timeline(Some(t));
        }
        self.editor_panel.set_error(err);
    }

    /// One scene evaluation per frame max — shared by inspector and viewport.
    /// Backed by the FrameMemo LRU: times seen before (scrub back, loop,
    /// replay) come straight from the cache. Takes fields separately so
    /// callers keep disjoint borrows of the rest.
    fn eval_state_cached<'a>(
        memo: &'a mut mo_core::FrameMemo,
        index_cache: &'a mut Option<(u64, mo_core::TimelineIndex)>,
        timeline: &'a Option<Timeline>,
        time: f32,
        epoch: u64,
    ) -> &'a HashMap<String, std::sync::Arc<mo_core::vgroup::VGroup>> {
        memo.get_or_insert_with((epoch, time.to_bits()), || match timeline {
            Some(tl) => {
                if !matches!(index_cache, Some((ep, _)) if *ep == epoch) {
                    *index_cache = Some((epoch, tl.index()));
                }
                let idx = &index_cache.as_ref().unwrap().1;
                Player::state_at_indexed(tl, idx, time)
            }
            None => HashMap::new(),
        })
    }

    pub fn open_file(&mut self, path: PathBuf) {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Some(proj) = crate::assets::project_of(&path, &self.asset_panel.project_path) {
                self.asset_panel.set_active_project(proj);
            }
            self.file_path = Some(path.clone());
            self.editor_panel.code = content.clone();
            self.editor_panel.is_dirty = false;
            let (tl, origins, err) = compile_code_in(&content, &path.parent().unwrap_or(std::path::Path::new(".")));
                self.play_origins = origins;
            self.set_timeline(tl);
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
            self.change_src_home(folder);
        }
    }

    /// Point the studio at a picked folder. If it is itself a project
    /// (has master.moa), it becomes the active project; otherwise it's the
    /// source root and we activate its newest master's project.
    pub fn change_src_home(&mut self, folder: PathBuf) {
        if folder.join("master.moa").exists() {
            let root = folder.parent().map(|p| p.to_path_buf()).unwrap_or_else(crate::assets::src_home);
            self.asset_panel.project_path = root;
            self.asset_panel.set_active_project(folder.clone());
            self.asset_panel.rescan();
            self.open_file(folder.join("master.moa"));
            return;
        }
        self.asset_panel.project_path = folder.clone();
        self.asset_panel.rescan();
        let newest = crate::assets::newest_master_project(&folder);
        match newest {
            Some(project) => {
                self.asset_panel.set_active_project(project.clone());
                let master = project.join("master.moa");
                if master.exists() {
                    self.open_file(master);
                }
            }
            None => {
                self.asset_panel.active_project = None;
                self.asset_panel.persist();
            }
        }
        self.set_status(format!("Source folder: {}", folder.display()));
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
        if let Some(path) = self.file_path.clone() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                self.editor_panel.code = content.clone();
                self.editor_panel.is_dirty = false;
                let (tl, origins, err) = compile_code_in(&content, &path.parent().unwrap_or(std::path::Path::new(".")));
                self.play_origins = origins;
                self.set_timeline(tl);
                self.editor_panel.set_error(err);
                self.set_status(format!("Reloaded {}", path.display()));
            }
        }
    }

    /// The active project: explicitly selected one wins; else the open file's.
    fn active_project(&self) -> Option<PathBuf> {
        self.asset_panel
            .active_project
            .clone()
            .or_else(|| {
                let path = self.file_path.as_ref()?;
                crate::assets::project_of(path, &self.asset_panel.project_path)
            })
    }

    /// Project's master.moa, if the project folder has one.
    fn project_master(&self, project: &Path) -> Option<PathBuf> {
        let m = project.join("master.moa");
        m.exists().then_some(m)
    }

    /// Name of the active file's video project folder.
    fn active_project_name(&self) -> Option<String> {
        self.active_project()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
    }

    /// Create a fresh video project: <name>/{master.moa, assets/}.
    pub fn new_video(&mut self, raw_name: &str) {
        let name: String = raw_name
            .trim()
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
            .collect();
        if name.is_empty() {
            self.set_status("Type a name for the new video first");
            return;
        }
        let dir = self.asset_panel.project_path.join(&name);
        if dir.exists() {
            self.set_status(format!("Project `{name}` already exists — opening it"));
            self.open_file(dir.join("master.moa"));
            return;
        }
        if let Err(e) = std::fs::create_dir_all(dir.join("assets")) {
            self.set_status(format!("Error creating project: {e}"));
            return;
        }
        match std::fs::write(dir.join("master.moa"), new_master_template()) {
            Ok(()) => {
                self.asset_panel.set_active_project(dir.clone());
                self.open_file(dir.join("master.moa"));
                self.asset_panel.rescan();
                self.asset_panel.new_video_name.clear();
                self.set_status(format!("Created video project `{name}`"));
            }
            Err(e) => self.set_status(format!("Error creating master: {e}")),
        }
    }

    /// Import base dir: the open file's folder (so `import "assets/x.moa"` works
    /// from any CWD), falling back to the active project, then the source root.
    fn compile_base_dir(&self) -> PathBuf {
        self.file_path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .or_else(|| self.asset_panel.active_project.clone())
            .unwrap_or_else(crate::assets::src_home)
    }

    pub fn new_asset(&mut self, raw_name: &str) {
        let name: String = raw_name
            .trim()
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
            .collect();
        let Some(project) = self.active_project() else {
            self.set_status("Select a project first — click one in the Videos panel, or create one");
            return;
        };
        if name.is_empty() {
            self.set_status("Type a name for the new asset first");
            return;
        }

        let assets_dir = project.join("assets");
        let _ = std::fs::create_dir_all(&assets_dir);
        let mut idx = 1;
        let mut target = assets_dir.join(format!("{name}.moa"));
        while target.exists() {
            idx += 1;
            target = assets_dir.join(format!("{name}-{idx}.moa"));
        }

        let template = new_asset_template();
        if let Err(e) = std::fs::write(&target, &template) {
            self.set_status(format!("Error creating asset: {e}"));
            return;
        }
        self.open_file(target.clone());
        self.asset_panel.rescan();

        // Wire the asset into the project's master automatically at the top of the file.
        let stem = target.file_stem().unwrap_or_default().to_string_lossy().replace('-', "_");
        let import_line = format!("import \"assets/{stem}.moa\" as {stem};");
        if let Some(master) = self.project_master(&project) {
            if let Ok(code) = std::fs::read_to_string(&master) {
                let updated = insert_import_at_top(&code, &import_line);
                let _ = std::fs::write(&master, &updated);
                self.asset_panel.new_asset_name.clear();
                self.asset_panel.rescan();
                self.set_status(format!("Created asset & added import to header in {}", master.display()));
                return;
            }
        }
        self.asset_panel.new_asset_name.clear();
        self.set_status(format!("Created asset {}", target.display()));
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

        // Poll file watcher for disk changes (only when editor is not dirty)
        if let Some(watcher) = &mut self.watcher {
            if watcher.poll_changed() && !self.editor_panel.is_dirty {
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

        // Global keyboard shortcuts. Playback / camera keys are gated on the
        // editor NOT holding focus, so typing never triggers them.
        let editor_focused = self.editor_panel.focus_held;
        ctx.input(|i| {
            if !editor_focused {
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
                if i.key_pressed(egui::Key::R) && !i.modifiers.any() {
                    self.camera.reset();
                    self.set_status("Camera reset to 2D default");
                }
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
                let name = self.asset_panel.new_asset_name.clone();
                self.new_asset(&name);
            }
        });

        // Top Menu Bar
        egui::TopBottomPanel::top("top_menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Asset (Ctrl+N)").clicked() {
                        let name = self.asset_panel.new_asset_name.clone();
                        self.new_asset(&name);
                        ui.close_menu();
                    }
                    ui.label(
                        egui::RichText::new("New video: use 🎬 New Video in the Videos panel")
                            .size(10.5)
                            .italics(),
                    );
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
                            let default_name = self.active_project_name().unwrap_or_else(|| "render".into());
                            if let Some(out_path) = rfd::FileDialog::new()
                                .add_filter("MP4 Video", &["mp4"])
                                .set_directory(crate::assets::out_home())
                                .set_file_name(&format!("{default_name}.mp4"))
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
                        let default_name = self.active_project_name().unwrap_or_else(|| "composition".into());
                        if let Some(out_path) = rfd::FileDialog::new()
                            .add_filter("MOAB Package", &["moab"])
                            .set_directory(crate::assets::out_home())
                            .set_file_name(&format!("{default_name}.moab"))
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
                    ui.separator();
                    if ui.button("Reset Camera (R)").clicked() {
                        self.camera.reset();
                        ui.close_menu();
                    }
                });

                ui.menu_button("Examples", |ui| {
                    let home = crate::assets::src_home();
                    for (label, project) in [
                        ("Master Video Composition", "master-showcase"),
                        ("3D Orbit Showcase", "demo_3d"),
                        ("LaTeX Showcase", "latex_showcase"),
                        ("Demo Scene", "demo"),
                    ] {
                        let p = home.join(project).join("master.moa");
                        if p.exists() && ui.button(format!("{label} ({})", home.join(project).display())).clicked() {
                            self.open_file(p);
                            ui.close_menu();
                        }
                    }
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some((msg, created)) = &self.status_message {
                        if now.duration_since(*created).as_secs_f32() < 4.5 {
                            ui.colored_label(Color32::from_rgb(0x00, 0xFF, 0x9D), msg);
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
                            .color(Color32::from_rgb(0xB8, 0xA8, 0xE8)),
                    );
                });
            });
        });

        // ===== Fixed layout: timeline bottom, explorer leftmost, editor, right column =====

        // Bottom: timeline (fixed height, not resizable)
        egui::TopBottomPanel::bottom("bottom_timeline_panel")
            .exact_height(150.0)
            .resizable(false)
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
                        match mo_dsl::parse_with_origins_in(&self.editor_panel.code, &self.compile_base_dir()) {
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
                                        let (tl, origins, err) = compile_code_in(&self.editor_panel.code, &self.compile_base_dir());
                self.play_origins = origins;
                                        if let Some(t) = tl {
                                            self.set_timeline(Some(t));
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

        // Leftmost: Explorer (always open, fixed width)
        egui::SidePanel::left("explorer_panel")
            .exact_width(270.0)
            .resizable(false)
            .show(ctx, |ui| {
                let action = self.asset_panel.show(ui, self.file_path.as_deref());
                match action {
                    AssetAction::Open(path) => {
                        eprintln!("[app] opening: {}", path.display());
                        self.open_file(path);
                    }
                    AssetAction::InsertImport { path, alias } => {
                        let stmt = format!("import \"{path}\" as {alias} at (0, 0);");
                        self.editor_panel.code = insert_import_at_top(&self.editor_panel.code, &stmt);
                        let (tl, origins, err) = compile_code_in(&self.editor_panel.code, &self.compile_base_dir());
                        self.play_origins = origins;
                        if let Some(t) = tl {
                            self.set_timeline(Some(t));
                        }
                        self.editor_panel.set_error(err);
                        self.editor_panel.is_dirty = true;
                        self.set_status(format!("Imported '{alias}' into scene header"));
                    }
                    AssetAction::NewAsset { name } => {
                        self.new_asset(&name);
                    }
                    AssetAction::NewVideo => {
                        let name = self.asset_panel.new_video_name.clone();
                        self.new_video(&name);
                    }
                    AssetAction::SelectProject(path) => {
                        self.asset_panel.set_active_project(path.clone());
                        self.asset_panel.rescan();
                        let master = path.join("master.moa");
                        if master.exists() {
                            self.open_file(master);
                        } else {
                            self.set_status(format!("Active project: {}", path.display()));
                        }
                    }
                    AssetAction::Deleted(path) => {
                        if self.file_path.as_deref() == Some(path.as_path()) {
                            self.file_path = None;
                            self.watcher = None;
                            self.set_timeline(None);
                            self.set_status("File deleted — editor cleared");
                        } else {
                            self.set_status("Deleted");
                        }
                    }
                    AssetAction::Renamed { old, new } => {
                        if self.file_path.as_deref() == Some(old.as_path()) {
                            self.file_path = Some(new.clone());
                            if let Some(w) = &mut self.watcher {
                                let _ = w.watch_file(&new);
                            }
                        }
                        self.asset_panel.rescan();
                        self.set_status(format!("Renamed to {}", new.display()));
                    }
                    AssetAction::ChangeSrcHome(folder) => {
                        self.change_src_home(folder);
                    }
                    AssetAction::None => {}
                }
            });

        // Next to explorer: Code Editor (fixed width, horizontal scroll inside)
        egui::SidePanel::left("editor_side_panel")
            .exact_width(430.0)
            .resizable(false)
            .show(ctx, |ui| {
                let (code_changed, save_req) = self.editor_panel.show(ui);
                if save_req {
                    self.save_current_file();
                    // Saving is explicit intent — flush any pending debounce now.
                    if self.compile_timer.take().is_some() && self.editor_panel.auto_compile {
                        self.recompile();
                    }
                }
                if code_changed && self.editor_panel.auto_compile {
                    self.compile_timer = Some(Instant::now());
                }
                if let Some(t0) = self.compile_timer {
                    if t0.elapsed() >= std::time::Duration::from_millis(200) {
                        self.compile_timer = None;
                        self.recompile();
                    }
                }
            });

        // Right column: viewport on top, inspector docked below (fixed)
        egui::SidePanel::right("right_column")
            .exact_width(560.0)
            .resizable(false)
            .show(ctx, |ui| {
                egui::TopBottomPanel::bottom("inspector_dock")
                    .exact_height(300.0)
                    .resizable(false)
                    .show_inside(ui, |ui| {
                        let current_state = Self::eval_state_cached(
                            &mut self.frame_memo,
                            &mut self.index_cache,
                            &self.timeline,
                            self.current_time,
                            self.timeline_epoch,
                        );
                        let inspector_action = InspectorPanel::show(
                            ui,
                            self.timeline.as_ref(),
                            current_state,
                            &mut self.selected_mobject,
                            &mut self.overrides,
                            &mut self.camera,
                            self.current_time,
                        );

                        if let InspectorAction::ApplyShift { id, dx, dy } = inspector_action {
                            if let Some(updated_code) = apply_shift_to_dsl(&self.editor_panel.code, &id, dx, dy) {
                                self.editor_panel.code = updated_code;
                                self.overrides.remove(&id);
                                let (tl, origins, err) = compile_code_in(&self.editor_panel.code, &self.compile_base_dir());
                                self.play_origins = origins;
                                if let Some(t) = tl {
                                    self.set_timeline(Some(t));
                                }
                                self.editor_panel.set_error(err);
                                self.editor_panel.is_dirty = true;
                                self.set_status(format!("Applied position ({:+.2}, {:+.2}) to code for '{}'", dx, dy, id));
                            } else {
                                // Check if `id` is a sub-object from an imported asset: `<alias>_<sub_id>`
                                let base_dir = self.compile_base_dir();
                                let mut handled = false;
                                for line in self.editor_panel.code.lines() {
                                    let trimmed = line.trim();
                                    if trimmed.starts_with("import") {
                                        let words: Vec<&str> = trimmed.split_whitespace().collect();
                                        if words.len() >= 4 && words[0] == "import" && words[2] == "as" {
                                            let rel_path = words[1].trim_matches('"');
                                            let alias = words[3].trim_end_matches(';');
                                            let prefix = format!("{alias}_");
                                            if id.starts_with(&prefix) {
                                                let sub_id = &id[prefix.len()..];
                                                let asset_file = base_dir.join(rel_path);
                                                if let Ok(asset_code) = std::fs::read_to_string(&asset_file) {
                                                    if let Some(new_asset_code) = apply_shift_to_dsl(&asset_code, sub_id, dx, dy) {
                                                        if std::fs::write(&asset_file, new_asset_code).is_ok() {
                                                            self.overrides.remove(&id);
                                                            let (tl, origins, err) = compile_code_in(&self.editor_panel.code, &base_dir);
                                                            self.play_origins = origins;
                                                            if let Some(t) = tl {
                                                                self.set_timeline(Some(t));
                                                            }
                                                            self.editor_panel.set_error(err);
                                                            self.set_status(format!(
                                                                "Applied position ({:+.2}, {:+.2}) to asset '{}' ({})",
                                                                dx, dy, sub_id, asset_file.display()
                                                            ));
                                                            handled = true;
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                if !handled {
                                    self.set_status(format!("Could not apply shift to code for '{}'", id));
                                }
                            }
                        }
                    });

                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(Color32::from_rgb(0x05, 0x00, 0x1A)))
                    .show_inside(ui, |ui| {
                        let base_state = Self::eval_state_cached(
                            &mut self.frame_memo,
                            &mut self.index_cache,
                            &self.timeline,
                            self.current_time,
                            self.timeline_epoch,
                        );
                        let epoch = self.timeline_epoch;
                        self.viewport_panel.show(
                            ui,
                            &self.render_state,
                            &mut self.renderer,
                            self.timeline.as_ref(),
                            self.current_time,
                            &mut self.camera,
                            &mut self.selected_mobject,
                            &mut self.overrides,
                            base_state,
                            epoch,
                            self.fps,
                        );
                    });
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
        let is_decl_match = words.len() >= 2 && words[1] == target_id;
        let is_import_match = words.len() >= 4
            && words[0] == "import"
            && words[2] == "as"
            && words[3].trim_end_matches(';') == target_id;

        if is_decl_match || is_import_match {
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
            } else if is_import_match {
                let mut new_line = line.clone();
                if let Some(semi_pos) = new_line.rfind(';') {
                    new_line.insert_str(semi_pos, &format!(" at ({:.2}, {:.2})", dx, dy));
                } else {
                    new_line.push_str(&format!(" at ({:.2}, {:.2});", dx, dy));
                }
                *line = new_line;
                modified = true;
                break;
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

/// Import base dir: the open file's folder (so `import "assets/x.moa"` works
/// from any CWD), falling back to the active project, then the source root.
fn compile_code_in(code: &str, base: &Path) -> (Option<Timeline>, Vec<mo_dsl::PlayOrigin>, Option<String>) {
    match mo_dsl::parse_with_origins_in(code, base) {
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
    r#"// MathOxide Component Asset — modular scene element
// Designed around local (0, 0) origin. Master composition imports and animates this asset.
circle c at (0, 0) r=1.5 fill=#58C4DD33, stroke=cyan, sw=3;
tex label at (0, -2) text="f(x) = \sin(x)" size=0.8 color=#58C4DD;
"#.into()
}

fn new_master_template() -> String {
    r#"// MathOxide Video — master composition
// Imports at the top:
// import "assets/intro.moa" as intro at (0, 0);

text title text="MathOxide Scene" at (0, 3) size=0.8 color=white;
play write(title) 1.0s smooth;
wait 1s;
"#.into()
}

/// Insert an import line cleanly at the top of a script:
/// - After existing imports if present, or
/// - Below the initial comment/header block.
pub fn insert_import_at_top(code: &str, import_stmt: &str) -> String {
    let import_trimmed = import_stmt.trim();
    if code.lines().any(|l| l.trim() == import_trimmed) {
        return code.to_string();
    }

    let mut lines: Vec<&str> = code.lines().collect();
    let last_import_idx = lines.iter().rposition(|l| l.trim_start().starts_with("import "));

    let insert_pos = if let Some(idx) = last_import_idx {
        idx + 1
    } else {
        // Place below initial comment or whitespace block
        let mut idx = 0;
        while idx < lines.len() && (lines[idx].trim_start().starts_with("//") || lines[idx].trim().is_empty()) {
            idx += 1;
        }
        idx
    };

    lines.insert(insert_pos, import_trimmed);
    let mut result = lines.join("\n");
    if !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_import_at_top_after_comments() {
        let code = "// Header\n// Subheader\ntext t at (0,0);\nplay write(t) 1s;\n";
        let updated = insert_import_at_top(code, "import \"assets/foo.moa\" as foo;");
        assert_eq!(
            updated,
            "// Header\n// Subheader\nimport \"assets/foo.moa\" as foo;\ntext t at (0,0);\nplay write(t) 1s;\n"
        );
    }

    #[test]
    fn test_insert_import_at_top_after_existing_imports() {
        let code = "import \"assets/a.moa\" as a;\ntext t at (0,0);\n";
        let updated = insert_import_at_top(code, "import \"assets/b.moa\" as b;");
        assert_eq!(
            updated,
            "import \"assets/a.moa\" as a;\nimport \"assets/b.moa\" as b;\ntext t at (0,0);\n"
        );
    }

    #[test]
    fn test_apply_shift_to_dsl_updates_import_coordinates() {
        let code = "import \"assets/intro.moa\" as intro at (0, 0);\nplay showCreation(intro) 1s;\n";
        let updated = apply_shift_to_dsl(code, "intro", 1.5, -2.0).expect("applies shift");
        assert!(updated.contains("import \"assets/intro.moa\" as intro at (1.50, -2.00);"));

        let code_without_at = "import \"assets/intro.moa\" as intro;\nplay showCreation(intro) 1s;\n";
        let updated2 = apply_shift_to_dsl(code_without_at, "intro", 0.5, 3.2).expect("applies shift");
        assert!(updated2.contains("import \"assets/intro.moa\" as intro at (0.50, 3.20);"));
    }
}
