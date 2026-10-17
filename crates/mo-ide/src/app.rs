use crate::editor::EditorPanel;
use crate::export::{export_moab, export_mp4};
use crate::inspector::InspectorPanel;
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
    timeline_panel: TimelinePanel,

    file_path: Option<PathBuf>,
    watcher: Option<FileWatcher>,

    timeline: Option<Timeline>,
    camera: Camera,
    current_time: f32,
    selected_mobject: Option<String>,
    overrides: HashMap<String, (f32, f32)>,

    // View toggles
    show_editor: bool,
    show_scene_tree: bool,
    show_inspector: bool,
    show_timeline: bool,

    // Left panel active tab (0: Editor, 1: Scene Tree)
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

        let path = initial_file.unwrap_or_else(|| PathBuf::from("examples/demo.moa"));
        let (code, watcher) = if path.exists() {
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            let w = FileWatcher::new(&path).ok();
            (content, w)
        } else {
            let starter = default_starter_code();
            (starter, None)
        };

        let mut editor_panel = EditorPanel::new(code.clone());
        let (timeline, err) = compile_code(&code);
        editor_panel.set_error(err);

        Ok(Self {
            render_state,
            renderer,
            viewport_panel,
            editor_panel,
            timeline_panel: TimelinePanel::default(),
            file_path: Some(path),
            watcher,
            timeline,
            camera,
            current_time: 0.0,
            selected_mobject: None,
            overrides: HashMap::new(),
            show_editor: true,
            show_scene_tree: true,
            show_inspector: true,
            show_timeline: true,
            left_tab: 0,
            last_frame_instant: Instant::now(),
            fps: 60.0,
            frame_count: 0,
            fps_timer: Instant::now(),
            status_message: Some(("MathOxide IDE ready".into(), Instant::now())),
        })
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), Instant::now()));
    }

    pub fn save_current_file(&mut self) {
        if let Some(path) = &self.file_path {
            if let Err(e) = std::fs::write(path, &self.editor_panel.code) {
                self.set_status(format!("Error saving: {}", e));
            } else {
                self.editor_panel.is_dirty = false;
                self.set_status(format!("Saved {}", path.display()));
            }
        }
    }

    pub fn reload_from_disk(&mut self) {
        if let Some(path) = &self.file_path {
            if let Ok(content) = std::fs::read_to_string(path) {
                self.editor_panel.code = content.clone();
                self.editor_panel.is_dirty = false;
                let (tl, err) = compile_code(&content);
                self.timeline = tl;
                self.editor_panel.set_error(err);
                self.set_status(format!("Reloaded {}", path.display()));
            }
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
            if i.modifiers.ctrl && i.key_pressed(egui::Key::S) {
                self.save_current_file();
            }
            if i.key_pressed(egui::Key::R) && !i.modifiers.any() {
                self.camera.center = glam::Vec3::ZERO;
                self.camera.height = 8.0;
                self.set_status("Camera reset");
            }
        });

        // Top Menu Bar
        egui::TopBottomPanel::top("top_menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Save (Ctrl+S)").clicked() {
                        self.save_current_file();
                        ui.close_menu();
                    }
                    if ui.button("Reload from Disk").clicked() {
                        self.reload_from_disk();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Export .moab Bundle...").clicked() {
                        let out_path = PathBuf::from("export.moab");
                        if let Err(e) = export_moab(&self.editor_panel.code, &out_path) {
                            self.set_status(format!("MOAB Export error: {}", e));
                        } else {
                            self.set_status(format!("Exported to {}", out_path.display()));
                        }
                        ui.close_menu();
                    }
                    if ui.button("Export MP4 Video...").clicked() {
                        if let Some(tl) = &self.timeline {
                            let out_path = PathBuf::from("export.mp4");
                            let tl_clone = tl.clone();
                            std::thread::spawn(move || {
                                let _ = export_mp4(&tl_clone, &out_path, 1920, 1080, 60, |_, _| {});
                            });
                            self.set_status("Started background MP4 export -> export.mp4");
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
                    ui.checkbox(&mut self.show_scene_tree, "Scene Tree");
                    ui.checkbox(&mut self.show_inspector, "Inspector");
                    ui.checkbox(&mut self.show_timeline, "Timeline");
                    ui.separator();
                    if ui.button("Reset Camera (R)").clicked() {
                        self.camera.center = glam::Vec3::ZERO;
                        self.camera.frame_height = 8.0;
                        ui.close_menu();
                    }
                });

                ui.menu_button("Presets", |ui| {
                    if ui.button("Demo Scene (examples/demo.moa)").clicked() {
                        let path = PathBuf::from("examples/demo.moa");
                        if let Ok(c) = std::fs::read_to_string(&path) {
                            self.file_path = Some(path.clone());
                            self.editor_panel.code = c.clone();
                            self.editor_panel.is_dirty = false;
                            let (tl, err) = compile_code(&c);
                            self.timeline = tl;
                            self.editor_panel.set_error(err);
                            self.overrides.clear();
                            if let Some(w) = &mut self.watcher {
                                let _ = w.watch_file(&path);
                            } else {
                                self.watcher = FileWatcher::new(&path).ok();
                            }
                            self.set_status("Loaded examples/demo.moa");
                        }
                        ui.close_menu();
                    }
                    if ui.button("Geometric Shapes Preset").clicked() {
                        let code = shapes_preset_code();
                        self.editor_panel.code = code.clone();
                        self.editor_panel.is_dirty = true;
                        let (tl, err) = compile_code(&code);
                        self.timeline = tl;
                        self.editor_panel.set_error(err);
                        self.overrides.clear();
                        self.set_status("Loaded Shapes Preset");
                        ui.close_menu();
                    }
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some((msg, created)) = &self.status_message {
                        if now.duration_since(*created).as_secs_f32() < 4.0 {
                            ui.colored_label(Color32::from_rgb(125, 207, 255), msg);
                        }
                    }
                    ui.separator();
                    ui.label(
                        egui::RichText::new("MathOxide v0.1")
                            .monospace()
                            .size(11.0)
                            .color(Color32::from_rgb(100, 110, 130)),
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
                    self.timeline_panel.show(
                        ui,
                        self.timeline.as_ref(),
                        &mut self.current_time,
                    );
                });
        }

        // Left Panel: Tabs for Editor and Scene Tree
        if self.show_editor || self.show_scene_tree {
            egui::SidePanel::left("left_side_panel")
                .resizable(true)
                .default_width(300.0)
                .min_width(200.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if self.show_editor && ui.selectable_label(self.left_tab == 0, "📝 Editor").clicked() {
                            self.left_tab = 0;
                        }
                        if self.show_scene_tree && ui.selectable_label(self.left_tab == 1, "🌳 Scene Tree").clicked() {
                            self.left_tab = 1;
                        }
                    });
                    ui.separator();

                    if self.left_tab == 0 && self.show_editor {
                        let (code_changed, save_req) = self.editor_panel.show(ui);
                        if save_req {
                            self.save_current_file();
                        }
                        if code_changed && self.editor_panel.auto_compile {
                            let (tl, err) = compile_code(&self.editor_panel.code);
                            if let Some(t) = tl {
                                self.timeline = Some(t);
                            }
                            self.editor_panel.set_error(err);
                        }
                    } else if self.show_scene_tree {
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
                });
        }

        // Right Panel: Inspector
        if self.show_inspector {
            egui::SidePanel::right("right_inspector_panel")
                .resizable(true)
                .default_width(240.0)
                .min_width(170.0)
                .show(ctx, |ui| {
                    let current_state = if let Some(tl) = &self.timeline {
                        Player::state_at(tl, self.current_time)
                    } else {
                        HashMap::new()
                    };
                    InspectorPanel::show(
                        ui,
                        self.timeline.as_ref(),
                        &current_state,
                        &mut self.selected_mobject,
                        &mut self.overrides,
                        &mut self.camera,
                        self.current_time,
                    );
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

fn compile_code(code: &str) -> (Option<Timeline>, Option<String>) {
    match mo_dsl::parse_or_report(code) {
        Ok(tl) => (Some(tl), None),
        Err(e) => (None, Some(e)),
    }
}

fn default_starter_code() -> String {
    r#"// MathOxide demo scene — circle drawn, square fades in, circle morphs into it.
circle c at (0,0) r=2 fill=#58C4DD33, stroke=blue, sw=6;
rect s at (3,0) w=3, h=3 fill=#FC625533, stroke=red, sw=4;
play showCreation(c) 2s smooth;
wait 0.5s;
play fadeIn(s) 1s smooth;
play transform(c, s) 1.5s smooth;
wait 0.5s;
"#.into()
}

fn shapes_preset_code() -> String {
    r#"// MathOxide shapes & transforms demo
circle c1 at (0,0) r=1.5 fill=#58C4DD55, stroke=cyan, sw=4;
rect r1 at (0,0) w=2.5, h=2.5 fill=#FC625555, stroke=red, sw=4;
play showCreation(c1) 1.5s smooth;
play transform(c1, r1) 1.2s smooth;
play rotate(c1, 3.14159) 1.5s smooth;
"#.into()
}

