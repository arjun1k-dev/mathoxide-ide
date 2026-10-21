use crate::assets::{AssetAction, AssetPanel};
use crate::editor::EditorPanel;
use crate::export::{export_moab, export_mp4};
use crate::inspector::{InspectorAction, InspectorPanel};
use crate::timeline::{PlaybackScope, TimelinePanel};
use crate::viewport::{ViewportAction, ViewportPanel};
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

/// State of the inline text/expr editor opened by double-clicking an object
/// in the viewport. `owner == None` edits the open editor file; `Some(path)`
/// edits a decl inside an imported asset file on disk.
struct InlineEdit {
    id: String,
    owner: Option<PathBuf>,
    /// Which string parameter is being edited: "text" or "expr".
    param: &'static str,
    draft: String,
    /// Object center in scene coords at open time (overlay anchor).
    anchor: (f32, f32),
    just_opened: bool,
}

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
    /// Compiled master for the `Full master` playback scope, cached per
    /// (path, mtime): (master path, mtime, timeline, play origins). Rebuilt
    /// lazily by `sync_master_scope` when stale; the editor keeps showing the
    /// open asset while the composition timeline drives playback.
    master_scope: Option<(PathBuf, std::time::SystemTime, Timeline, Vec<mo_dsl::PlayOrigin>)>,
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
    /// Viewport multi-selection, in pick order (last = primary for the
    /// Inspector). Group drag/nudge moves every member.
    selection: Vec<String>,
    /// Grid snap quantum for drags/nudges (None = off; G toggles 0.25).
    snap: Option<f32>,
    /// Active inline text/expr edit (viewport double-click), if any.
    inline_edit: Option<InlineEdit>,
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
            master_scope: None,
            timeline_epoch: 0,
            frame_memo: mo_core::FrameMemo::new(16_384),
            index_cache: None,
            camera,
            current_time: 0.0,
            selection: Vec::new(),
            snap: None,
            inline_edit: None,
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

    /// Master path for the Full-master scope: the active project's master,
    /// when the open file isn't that master itself.
    fn scope_master_path(&self) -> Option<PathBuf> {
        let proj = self.asset_panel.active_project.clone()?;
        let master = proj.join("master.moa");
        if !master.exists() || self.file_path.as_deref() == Some(master.as_path()) {
            return None;
        }
        Some(master)
    }

    fn full_master_active(&self) -> bool {
        self.timeline_panel.scope == PlaybackScope::FullMaster && self.master_scope.is_some()
    }

    /// Duration of whichever timeline the transport is scoped to.
    fn active_duration(&self) -> Option<f32> {
        if self.full_master_active() {
            self.master_scope.as_ref().map(|(_, _, tl, _)| tl.duration())
        } else {
            self.timeline.as_ref().map(|tl| tl.duration())
        }
    }

    /// Nudge every selected object by (dx, dy) scene units as a pending
    /// override (same persistence path as viewport drags); snap-quantized.
    fn nudge_selection(&mut self, dx: f32, dy: f32) {
        let ids = self.selection.clone();
        for id in ids {
            let e = self.overrides.entry(id).or_insert((0.0, 0.0));
            e.0 += dx;
            e.1 += dy;
            if let Some(q) = self.snap.filter(|q| *q > 0.0) {
                e.0 = (e.0 / q).round() * q;
                e.1 = (e.1 / q).round() * q;
            }
        }
    }

    /// Open the inline editor for a double-clicked object: locate its decl
    /// line (open file first, then imported asset files) and the string
    /// param it carries.
    fn open_inline_edit(&mut self, id: String) {
        const PARAMS: &[&str] = &["text", "expr"];
        let mut located = find_string_decl(&self.editor_panel.code, &id, PARAMS)
            .map(|(_, p, v)| (None, p, v));
        if located.is_none() {
            let base_dir = self.compile_base_dir();
            for line in self.editor_panel.code.lines() {
                let words: Vec<&str> = line.trim().split_whitespace().collect();
                if words.len() >= 4 && words[0] == "import" && words[2] == "as" {
                    let alias = words[3].trim_end_matches(';');
                    let prefix = format!("{alias}_");
                    if let Some(sub_id) = id.strip_prefix(&prefix) {
                        let f = base_dir.join(words[1].trim_matches('"'));
                        if let Ok(code) = std::fs::read_to_string(&f) {
                            if let Some((_, p, v)) = find_string_decl(&code, sub_id, PARAMS) {
                                located = Some((Some(f), p, v));
                                break;
                            }
                        }
                    }
                }
            }
        }
        let Some((owner, param, value)) = located else {
            self.set_status(format!("'{id}' carries no inline text/expr to edit"));
            return;
        };
        let anchor = self.scene_anchor_for(&id);
        self.inline_edit = Some(InlineEdit {
            id,
            owner,
            param,
            draft: value,
            anchor,
            just_opened: true,
        });
    }

    /// Center (scene coords, override-compensated) of `id` at the current
    /// time — one-off eval, only paid when the inline editor opens.
    fn scene_anchor_for(&self, id: &str) -> (f32, f32) {
        if let Some(tl) = self.timeline.as_ref() {
            if let Some(g) = Player::state_at(tl, self.current_time).get(id) {
                let c = g.center();
                let (odx, ody) = self.overrides.get(id).copied().unwrap_or((0.0, 0.0));
                return (c.x + odx, c.y + ody);
            }
        }
        (0.0, 0.0)
    }

    /// Persist the inline edit: rewrite the `param="..."` value on the decl
    /// line in the owning file, then recompile (editor file → editor + dirty;
    /// asset file → disk write, imports re-read).
    fn apply_inline_edit(&mut self, edit: InlineEdit) {
        if edit.draft.contains('"') {
            self.set_status("Inline edit rejected: `\"` is not allowed in DSL string values");
            return;
        }
        let owner_file = edit.owner.clone();
        let code_src = match &owner_file {
            Some(f) => std::fs::read_to_string(f).unwrap_or_default(),
            None => self.editor_panel.code.clone(),
        };
        // The decl id inside the owning file (alias prefix stripped for assets).
        let local_id = match &owner_file {
            Some(f) => {
                let base_dir = self.compile_base_dir();
                let mut lid = edit.id.clone();
                for line in self.editor_panel.code.lines() {
                    let words: Vec<&str> = line.trim().split_whitespace().collect();
                    if words.len() >= 4 && words[0] == "import" && words[2] == "as" {
                        let prefix = format!("{}_", words[3].trim_end_matches(';'));
                        if let Some(sub) = edit.id.strip_prefix(&prefix) {
                            if base_dir.join(words[1].trim_matches('"')) == *f {
                                lid = sub.to_string();
                            }
                        }
                    }
                }
                lid
            }
            None => edit.id.clone(),
        };
        let Some((line_idx, _, _)) = find_string_decl(&code_src, &local_id, &[edit.param]) else {
            self.set_status(format!("Could not locate {}= on '{}' to update", edit.param, local_id));
            return;
        };
        let mut lines: Vec<String> = code_src.lines().map(|s| s.to_string()).collect();
        let line = &mut lines[line_idx];
        match string_param_span(line, edit.param) {
            Some((s, e)) => line.replace_range(s..e, &edit.draft),
            None => {
                self.set_status("Inline edit failed: parameter span vanished");
                return;
            }
        }
        let mut new_code = lines.join("\n");
        if code_src.ends_with('\n') {
            new_code.push('\n');
        }
        match owner_file {
            Some(f) => {
                if std::fs::write(&f, &new_code).is_ok() {
                    self.recompile();
                    self.set_status(format!("Updated {} of '{}' in {}", edit.param, local_id, f.display()));
                } else {
                    self.set_status(format!("Failed to write {}", f.display()));
                }
            }
            None => {
                self.editor_panel.code = new_code;
                self.editor_panel.is_dirty = true;
                self.recompile();
                self.set_status(format!("Updated {} of '{}'", edit.param, local_id));
            }
        }
    }

    /// Rebuild the master-scope cache when stale (missing, path changed, or
    /// master.moa mtime moved). Reverts the scope if the master won't compile.
    fn sync_master_scope(&mut self) {
        if self.timeline_panel.scope != PlaybackScope::FullMaster {
            return;
        }
        let Some(master) = self.scope_master_path() else {
            self.timeline_panel.scope = PlaybackScope::ThisFile;
            self.master_scope = None;
            return;
        };
        let mtime = std::fs::metadata(&master).and_then(|m| m.modified()).ok();
        let fresh = matches!(
            (&self.master_scope, &mtime),
            (Some((p, t, _, _)), Some(m)) if p == &master && t == m
        );
        if fresh {
            return;
        }
        let dir = master.parent().unwrap_or(std::path::Path::new(".")).to_path_buf();
        match std::fs::read_to_string(&master)
            .map_err(|e| e.to_string())
            .and_then(|code| {
                mo_dsl::parse_with_origins_in(&code, &dir)
                    .map_err(|d| d.iter().map(|d| d.message.clone()).collect::<Vec<_>>().join(", "))
            }) {
            Ok((tl, origins)) => {
                self.master_scope = Some((
                    master,
                    mtime.unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                    tl,
                    origins,
                ));
            }
            Err(e) => {
                self.timeline_panel.scope = PlaybackScope::ThisFile;
                self.master_scope = None;
                self.set_status(format!("Full master scope unavailable: {e}"));
            }
        }
    }

    /// Apply drag-edited run times to the master composition (cross-file,
    /// exactly like the normal path but against the scope's master, writing
    /// the master file + touched asset files to disk).
    fn apply_timings_master_scope(&mut self, run_times: Vec<f32>) {
        let Some((master, _, _, origins)) = &self.master_scope else {
            return;
        };
        if origins.len() != run_times.len() {
            self.set_status("Cannot apply: run times and plays out of sync — recompile first");
            return;
        }
        let master = master.clone();
        let dir = master.parent().unwrap_or(std::path::Path::new(".")).to_path_buf();
        let Ok(code) = std::fs::read_to_string(&master) else {
            self.set_status("Cannot apply: master.moa unreadable");
            return;
        };
        let Ok((_, origins)) = mo_dsl::parse_with_origins_in(&code, &dir) else {
            self.set_status("Cannot apply: master.moa failed to re-parse");
            return;
        };
        if origins.len() != run_times.len() {
            self.set_status("Cannot apply: run times and plays out of sync — recompile first");
            return;
        }
        match crate::timeline::apply_timings_multi(&code, &origins, &run_times) {
            Some((new_code, asset_writes)) => {
                let mut written = 0;
                for (path, content) in &asset_writes {
                    if std::fs::write(path, content).is_ok() {
                        written += 1;
                    }
                }
                if std::fs::write(&master, &new_code).is_ok() {
                    // Rebuild the scope cache from the written source (mtime
                    // granularity is too coarse to rely on here).
                    if let Ok((tl, origins)) = mo_dsl::parse_with_origins_in(&new_code, &dir) {
                        let mtime = std::fs::metadata(&master)
                            .and_then(|m| m.modified())
                            .ok()
                            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                        self.master_scope = Some((master, mtime, tl, origins));
                    }
                    if written > 0 {
                        self.set_status(format!(
                            "Timings applied to master composition: master + {written} asset file(s) updated"
                        ));
                    } else {
                        self.set_status("Timeline timings written to master composition");
                    }
                } else {
                    self.set_status("Apply failed: could not write master.moa");
                }
            }
            None => {
                self.set_status("Apply failed: could not read/rewrite an imported asset file");
            }
        }
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
        timeline: Option<&'a Timeline>,
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
            // Solo-preview hint for project assets that carry their own plays.
            let is_project_asset = self
                .asset_panel
                .active_project
                .as_ref()
                .map(|p| path.starts_with(p) && path != p.join("master.moa"))
                .unwrap_or(false);
            if is_project_asset && content.contains("play ") {
                self.set_status(format!(
                    "Opened {} — solo preview; timeline scope 'Full master' plays the composition",
                    path.display()
                ));
            } else {
                self.set_status(format!("Opened {}", path.display()));
            }
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

    /// Persist position overrides back into source, batched: master-owned ids
    /// in one rewrite pass, `alias_`-prefixed ids grouped per asset file
    /// (read once, rewrite once, write once), then a single recompile.
    fn apply_shifts_to_source(&mut self, shifts: Vec<(String, f32, f32)>) {
        let (updated, mut unapplied) = apply_shifts_to_dsl(&self.editor_panel.code, &shifts);
        if let Some(new_code) = updated {
            for (id, _, _) in shifts.iter().filter(|(id, _, _)| !unapplied.contains(id)) {
                self.overrides.remove(id);
            }
            self.editor_panel.code = new_code;
            let (tl, origins, err) = compile_code_in(&self.editor_panel.code, &self.compile_base_dir());
            self.play_origins = origins;
            if let Some(t) = tl {
                self.set_timeline(Some(t));
            }
            self.editor_panel.set_error(err);
            self.editor_panel.is_dirty = true;
        }

        // Cross-file fallback: group remaining ids by owning asset file via
        // the import header (alias → rel path); each file is read once,
        // rewritten once, written once.
        if !unapplied.is_empty() {
            let base_dir = self.compile_base_dir();
            // (asset file, aliased id, sub id, dx, dy)
            let mut per_file: HashMap<PathBuf, Vec<(String, String, f32, f32)>> = HashMap::new();
            for line in self.editor_panel.code.lines() {
                let trimmed = line.trim();
                if !trimmed.starts_with("import") {
                    continue;
                }
                let words: Vec<&str> = trimmed.split_whitespace().collect();
                if words.len() >= 4 && words[0] == "import" && words[2] == "as" {
                    let rel_path = words[1].trim_matches('"');
                    let alias = words[3].trim_end_matches(';');
                    let prefix = format!("{alias}_");
                    let asset_file = base_dir.join(rel_path);
                    for (id, dx, dy) in shifts.iter() {
                        if let Some(sub_id) = id.strip_prefix(&prefix) {
                            if unapplied.contains(id) {
                                per_file.entry(asset_file.clone()).or_default().push((
                                    id.clone(),
                                    sub_id.to_string(),
                                    *dx,
                                    *dy,
                                ));
                            }
                        }
                    }
                }
            }
            let mut written_files = 0;
            let mut applied_ids: Vec<String> = Vec::new();
            for (asset_file, entries) in per_file {
                let Ok(asset_code) = std::fs::read_to_string(&asset_file) else {
                    continue;
                };
                let sub_shifts: Vec<(String, f32, f32)> = entries
                    .iter()
                    .map(|(_, sub, dx, dy)| (sub.clone(), *dx, *dy))
                    .collect();
                if let Some(new_asset_code) = apply_shifts_to_dsl(&asset_code, &sub_shifts).0 {
                    if std::fs::write(&asset_file, new_asset_code).is_ok() {
                        written_files += 1;
                        for (aliased, _, _, _) in &entries {
                            applied_ids.push(aliased.clone());
                        }
                    }
                }
            }
            for id in &applied_ids {
                self.overrides.remove(id);
                unapplied.retain(|u| u != id);
            }
            if written_files > 0 {
                let (tl, origins, err) = compile_code_in(&self.editor_panel.code, &self.compile_base_dir());
                self.play_origins = origins;
                if let Some(t) = tl {
                    self.set_timeline(Some(t));
                }
                self.editor_panel.set_error(err);
            }
        }

        if unapplied.is_empty() {
            self.set_status("Applied position(s) to source");
        } else {
            self.set_status(format!(
                "Could not apply shift for: {}",
                unapplied.join(", ")
            ));
        }
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

        // Keep the master-scope cache fresh before anything samples it.
        self.sync_master_scope();

        // Advance timeline playback if playing
        if self.timeline_panel.playing {
            if let Some(dur) = self.active_duration() {
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
        // editor NOT holding focus and no inline edit being open, so typing
        // never triggers them. Escape is handled below, ungated.
        let editor_focused = self.editor_panel.focus_held;
        let inline_editing = self.inline_edit.is_some();
        ctx.input(|i| {
            if !editor_focused && !inline_editing {
                if i.key_pressed(egui::Key::Space) {
                    self.timeline_panel.playing = !self.timeline_panel.playing;
                }
                if i.key_pressed(egui::Key::Home) {
                    self.current_time = 0.0;
                }
                if i.key_pressed(egui::Key::End) {
                    self.timeline_panel.playing = false;
                    self.current_time = self.active_duration().unwrap_or(0.0);
                }
                // Arrows are context-dependent: with a selection they nudge
                // the selected objects (Shift = 1.0 world units); with none
                // they step the playhead (Shift = 10 frames).
                let arrow_step = if i.modifiers.shift { 1.0 } else { 0.1 };
                if i.key_pressed(egui::Key::ArrowRight) {
                    self.timeline_panel.playing = false;
                    if !self.selection.is_empty() {
                        self.nudge_selection(arrow_step, 0.0);
                    } else {
                        let frames = if i.modifiers.shift { 10.0 } else { 1.0 } / 60.0;
                        let dur = self.active_duration().unwrap_or(0.0);
                        self.current_time = (self.current_time + frames).min(dur);
                    }
                }
                if i.key_pressed(egui::Key::ArrowLeft) {
                    self.timeline_panel.playing = false;
                    if !self.selection.is_empty() {
                        self.nudge_selection(-arrow_step, 0.0);
                    } else {
                        let frames = if i.modifiers.shift { 10.0 } else { 1.0 } / 60.0;
                        self.current_time = (self.current_time - frames).max(0.0);
                    }
                }
                if i.key_pressed(egui::Key::ArrowUp) && !self.selection.is_empty() {
                    self.timeline_panel.playing = false;
                    self.nudge_selection(0.0, arrow_step);
                }
                if i.key_pressed(egui::Key::ArrowDown) && !self.selection.is_empty() {
                    self.timeline_panel.playing = false;
                    self.nudge_selection(0.0, -arrow_step);
                }
                if i.key_pressed(egui::Key::G) && !i.modifiers.any() {
                    self.snap = match self.snap {
                        Some(_) => {
                            self.set_status("Grid snap off");
                            None
                        }
                        None => {
                            self.set_status("Grid snap on (0.25)");
                            Some(0.25)
                        }
                    };
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
            // Escape is ungated: cancels the inline edit first, else deselects.
            if i.key_pressed(egui::Key::Escape) && !i.modifiers.any() {
                if self.inline_edit.take().is_some() {
                    self.set_status("Inline edit cancelled");
                } else if !self.selection.is_empty() && !editor_focused {
                    self.selection.clear();
                    self.set_status("Deselected");
                }
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
                        let export_tl = if self.full_master_active() {
                            self.master_scope.as_ref().map(|(_, _, tl, _)| tl.clone())
                        } else {
                            self.timeline.clone()
                        };
                        if let Some(tl_clone) = export_tl {
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
            .exact_height(180.0)
            .resizable(false)
            .show(ctx, |ui| {
                let full_master = self.full_master_active();
                let master_available = self.scope_master_path().is_some();
                let scope_before = self.timeline_panel.scope;
                // Scope-aware borrows: drag edits mutate the master-scope
                // timeline in memory; Apply Timings persists cross-file.
                let (tl_mut, origins_ref): (Option<&mut Timeline>, &[mo_dsl::PlayOrigin]) =
                    if full_master {
                        let (_, _, tl, org) = self.master_scope.as_mut().unwrap();
                        (Some(tl), org.as_slice())
                    } else {
                        (self.timeline.as_mut(), self.play_origins.as_slice())
                    };
                let mut panel = std::mem::take(&mut self.timeline_panel);
                let tl_action = panel.show(
                    ui,
                    tl_mut,
                    origins_ref,
                    &mut self.current_time,
                    master_available,
                );
                let scope_changed = panel.scope != scope_before;
                self.timeline_panel = panel;
                if scope_changed {
                    match self.timeline_panel.scope {
                        PlaybackScope::FullMaster => {
                            self.current_time = 0.0;
                            self.set_status("Scope: Full master — playback follows the composition");
                        }
                        PlaybackScope::ThisFile => {
                            self.current_time = 0.0;
                            self.set_status("Scope: this file's own timeline");
                        }
                    }
                }
                match tl_action {
                    Some(crate::timeline::TimelineAction::Edited) => {
                        if full_master {
                            self.set_status("Master timeline dragged — 💾 Apply Timings to persist");
                        } else {
                            self.editor_panel.is_dirty = true;
                        }
                    }
                    Some(crate::timeline::TimelineAction::ApplyTimings(run_times)) => {
                        if full_master {
                            self.apply_timings_master_scope(run_times);
                        } else {
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
                    AssetAction::PreviewFile(path) => {
                        self.open_file(path.clone());
                        if self.timeline.is_some() {
                            self.timeline_panel.playing = true;
                        }
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
                        let timeline_ref: Option<&Timeline> = if self.full_master_active() {
                            self.master_scope.as_ref().map(|(_, _, tl, _)| tl)
                        } else {
                            self.timeline.as_ref()
                        };
                        let current_state = Self::eval_state_cached(
                            &mut self.frame_memo,
                            &mut self.index_cache,
                            timeline_ref,
                            self.current_time,
                            self.timeline_epoch,
                        );
                        let inspector_action = InspectorPanel::show(
                            ui,
                            timeline_ref,
                            current_state,
                            &mut self.selection,
                            &mut self.overrides,
                            &mut self.camera,
                            self.current_time,
                        );

                        if let InspectorAction::ApplyShifts(shifts) = inspector_action {
                            self.apply_shifts_to_source(shifts);
                        }
                    });

                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(Color32::from_rgb(0x05, 0x00, 0x1A)))
                    .show_inside(ui, |ui| {
                        let timeline_ref: Option<&Timeline> = if self.full_master_active() {
                            self.master_scope.as_ref().map(|(_, _, tl, _)| tl)
                        } else {
                            self.timeline.as_ref()
                        };
                        let base_state = Self::eval_state_cached(
                            &mut self.frame_memo,
                            &mut self.index_cache,
                            timeline_ref,
                            self.current_time,
                            self.timeline_epoch,
                        );
                        let epoch = self.timeline_epoch;
                        let vp_action = self.viewport_panel.show(
                            ui,
                            &self.render_state,
                            &mut self.renderer,
                            timeline_ref,
                            self.current_time,
                            &mut self.camera,
                            &mut self.selection,
                            &mut self.overrides,
                            base_state,
                            epoch,
                            self.fps,
                            self.snap,
                        );
                        if let Some(ViewportAction::EditSource(id)) = vp_action {
                            self.open_inline_edit(id);
                        }
                    });
            });

        // Inline text/expr edit overlay, anchored above the edited object.
        if self.inline_edit.is_some() {
            let mut edit = self.inline_edit.take().unwrap();
            let mut apply_now = false;
            let canvas = self.viewport_panel.last_canvas_rect;
            let cam = &self.viewport_panel.last_cam;
            let p = cam.scene_to_screen(edit.anchor.0, edit.anchor.1);
            let box_w = 380.0;
            let mut bx = canvas.left() + p.x - box_w * 0.5;
            let mut by = canvas.top() + p.y - 70.0;
            if canvas.width() > box_w + 8.0 {
                bx = bx.clamp(canvas.left() + 4.0, canvas.right() - box_w - 4.0);
            }
            by = by.max(canvas.top() + 4.0).min(canvas.bottom() - 70.0);
            egui::Area::new(egui::Id::new("inline_edit_overlay"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(bx, by))
                .show(ctx, |ui| {
                    egui::Frame::window(ui.style()).show(ui, |ui| {
                        ui.set_width(box_w);
                        ui.horizontal(|ui| {
                            ui.colored_label(Color32::from_rgb(0xFF, 0xE0, 0x66), &edit.id);
                            ui.label(
                                egui::RichText::new(format!("{}=", edit.param))
                                    .monospace()
                                    .color(Color32::from_rgb(0x00, 0xD4, 0xFF)),
                            );
                        });
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut edit.draft)
                                .desired_width(box_w - 16.0)
                                .font(egui::TextStyle::Monospace),
                        );
                        if edit.just_opened {
                            resp.request_focus();
                            edit.just_opened = false;
                        }
                        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            apply_now = true;
                        }
                        ui.label(
                            egui::RichText::new("Enter = apply · Esc = cancel")
                                .size(10.0)
                                .color(Color32::from_rgb(0x8A, 0x2B, 0xE2)),
                        );
                    });
                });
            if apply_now {
                self.apply_inline_edit(edit);
            } else {
                self.inline_edit = Some(edit);
            }
        }
    }
}

/// Rewrite one matched declaration/import line's `at (x, y)` by (dx, dy),
/// inserting an `at` when the line carries none. Returns false if the line
/// has no editable position.
fn apply_shift_on_line(line: &mut String, target_id: &str, dx: f32, dy: f32, is_import: bool) -> bool {
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
                return true;
            }
        }
    }
    if is_import {
        let mut new_line = line.clone();
        if let Some(semi_pos) = new_line.rfind(';') {
            new_line.insert_str(semi_pos, &format!(" at ({:.2}, {:.2})", dx, dy));
        } else {
            new_line.push_str(&format!(" at ({:.2}, {:.2});", dx, dy));
        }
        *line = new_line;
        return true;
    }
    // Bare-id fallback: a decl match whose line has no parseable `at (`.
    if let Some(id_pos) = line.find(target_id) {
        let insert_pos = id_pos + target_id.len();
        let mut new_line = line[..insert_pos].to_string();
        new_line.push_str(&format!(" at ({:.2}, {:.2})", dx, dy));
        new_line.push_str(&line[insert_pos..]);
        *line = new_line;
        return true;
    }
    false
}

/// Apply several position shifts in ONE pass over the source. Returns the
/// rewritten code (None if nothing matched) plus the ids that found no
/// declaration line here (callers fall back to cross-file asset rewrites).
fn apply_shifts_to_dsl(code: &str, shifts: &[(String, f32, f32)]) -> (Option<String>, Vec<String>) {
    let mut lines: Vec<String> = code.lines().map(|s| s.to_string()).collect();
    let mut pending: HashMap<String, (f32, f32)> = shifts
        .iter()
        .map(|(id, dx, dy)| (id.clone(), (*dx, *dy)))
        .collect();
    let mut applied = 0usize;

    for line in &mut lines {
        if pending.is_empty() {
            break;
        }
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") {
            continue;
        }
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        let mut matched: Option<(String, bool)> = None;
        for (target, _) in pending.iter() {
            let is_decl = words.len() >= 2 && words[1] == target.as_str();
            let is_import = words.len() >= 4
                && words[0] == "import"
                && words[2] == "as"
                && words[3].trim_end_matches(';') == target.as_str();
            if is_decl || is_import {
                matched = Some((target.clone(), is_import));
                break;
            }
        }
        if let Some((target, is_import)) = matched {
            let (dx, dy) = pending[&target];
            if apply_shift_on_line(line, &target, dx, dy, is_import) {
                pending.remove(&target);
                applied += 1;
            }
        }
    }

    let out = if applied > 0 {
        let mut res = lines.join("\n");
        if code.ends_with('\n') {
            res.push('\n');
        }
        Some(res)
    } else {
        None
    };
    (out, pending.into_keys().collect())
}

fn apply_shift_to_dsl(code: &str, target_id: &str, dx: f32, dy: f32) -> Option<String> {
    apply_shifts_to_dsl(code, &[(target_id.to_string(), dx, dy)]).0
}

/// Byte span of a `param="..."` value's contents in a line. The DSL has no
/// `\"` escapes — quotes terminate the value (LaTeX content is backslash-heavy).
fn string_param_span(line: &str, param: &str) -> Option<(usize, usize)> {
    let needle = format!("{param}=\"");
    let start = line.find(&needle)? + needle.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some((start, start + end))
}

/// Find the declaration line for `id` and whichever of `params` it carries.
/// Returns (line index, param name, current value).
fn find_string_decl<'a>(code: &str, id: &str, params: &[&'a str]) -> Option<(usize, &'a str, String)> {
    for (i, line) in code.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        if words.len() >= 2 && words[1] == id {
            for p in params {
                if let Some((s, e)) = string_param_span(line, p) {
                    return Some((i, p, line[s..e].to_string()));
                }
            }
        }
    }
    None
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

    #[test]
    fn test_apply_shifts_to_dsl_batched_single_pass() {
        let code = "circle c at (0, 0) r=1;\nsquare s at (3, 0) side=2;\n";
        let shifts = vec![("c".to_string(), 1.0, 2.0), ("s".to_string(), -0.5, 0.25)];
        let (out, unapplied) = apply_shifts_to_dsl(code, &shifts);
        assert!(unapplied.is_empty());
        let out = out.expect("both applied");
        assert!(out.contains("circle c at (1.00, 2.00) r=1;"));
        assert!(out.contains("square s at (2.50, 0.25) side=2;"));
    }

    #[test]
    fn test_apply_shifts_to_dsl_reports_unmatched_ids() {
        let code = "circle c at (0, 0) r=1;\n";
        let shifts = vec![("c".to_string(), 1.0, 1.0), ("ghost".to_string(), 2.0, 2.0)];
        let (out, unapplied) = apply_shifts_to_dsl(code, &shifts);
        assert_eq!(unapplied, vec!["ghost".to_string()]);
        assert!(out.expect("c applied").contains("circle c at (1.00, 1.00) r=1;"));
    }

    #[test]
    fn test_apply_shifts_to_dsl_missing_target_is_none() {
        let (out, unapplied) = apply_shifts_to_dsl("circle c r=1;\n", &[("nope".to_string(), 1.0, 1.0)]);
        assert!(out.is_none());
        assert_eq!(unapplied, vec!["nope".to_string()]);
    }

    #[test]
    fn test_string_param_span_finds_value() {
        let line = r#"tex formula text="e^{i\pi} + 1 = 0" at (0, 0) size=1.5;"#;
        let (s, e) = string_param_span(line, "text").unwrap();
        assert_eq!(&line[s..e], r#"e^{i\pi} + 1 = 0"#);
        assert!(string_param_span(line, "expr").is_none());
    }

    #[test]
    fn test_find_string_decl_prefers_text_then_expr() {
        let code = "circle c r=1;\nfunc wave on=ax expr=\"sin(2*x)\" from=-5;\n";
        let (_, p, v) = find_string_decl(code, "wave", &["text", "expr"]).unwrap();
        assert_eq!(p, "expr");
        assert_eq!(v, "sin(2*x)");
        assert!(find_string_decl(code, "missing", &["text", "expr"]).is_none());
    }

    #[test]
    fn test_inline_edit_rewrite_round_trips_through_parser() {
        let code = "tex formula text=\"e^{i\\\\pi} + 1 = 0\" at (0, 0);\nplay write(formula) 1s;\n";
        let (line_idx, param, value) = find_string_decl(code, "formula", &["text", "expr"]).unwrap();
        assert_eq!(param, "text");
        let mut lines: Vec<String> = code.lines().map(|s| s.to_string()).collect();
        let (s, e) = string_param_span(&lines[line_idx], param).unwrap();
        lines[line_idx].replace_range(s..e, "e^{i\\\\pi}");
        let new_code = lines.join("\n") + "\n";
        assert!(new_code.contains("text=\"e^{i\\\\pi}\""));
        assert!(mo_dsl::parse_or_report(&new_code).is_ok());
        let _ = value;
    }
}
