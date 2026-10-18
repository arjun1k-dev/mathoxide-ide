use std::path::{Path, PathBuf};
use std::time::Instant;

/// Source home: all video projects live here, one folder per video.
pub fn src_home() -> PathBuf {
    let mut p = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    p.push("mathoxide");
    let _ = std::fs::create_dir_all(&p);
    p
}

/// Output home: rendered videos / bundles land here by default.
pub fn out_home() -> PathBuf {
    let mut p = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    p.push("Videos");
    p.push("mathoxide");
    let _ = std::fs::create_dir_all(&p);
    p
}

/// The project folder containing `path`, or None if outside any project.
pub fn project_of(path: &Path, src_root: &Path) -> Option<PathBuf> {
    let rel = path.strip_prefix(src_root).ok()?;
    rel.iter().next().map(|first| src_root.join(first))
}

fn config_path() -> PathBuf {
    let mut p = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    p.push(".config");
    p.push("mathoxide");
    let _ = std::fs::create_dir_all(&p);
    p.push("studio.toml");
    p
}

/// Persist the source root and active project across sessions.
pub fn save_config(src_root: &Path, active_project: Option<&Path>) {
    let mut text = format!("src_root = \"{}\"\n", src_root.display());
    if let Some(p) = active_project {
        text.push_str(&format!("active_project = \"{}\"\n", p.display()));
    }
    let _ = std::fs::write(config_path(), text);
}

#[derive(Clone, Debug)]
pub struct AssetEntry {
    pub path: PathBuf,
    /// Path relative to the owning project folder (for `import "assets/x.moa"`).
    pub rel_path: String,
    pub filename: String,
    pub is_master: bool,
}

/// One video project: `<src_root>/<name>/` with master.moa + assets/.
#[derive(Debug)]
pub struct ProjectEntry {
    pub name: String,
    pub path: PathBuf,
    pub master: Option<AssetEntry>,
    pub assets: Vec<AssetEntry>,
}

pub enum AssetAction {
    None,
    Open(PathBuf),
    InsertImport { path: String, alias: String },
    NewAsset { name: String },
    NewVideo,
    SelectProject(PathBuf),
    ChangeSrcHome(PathBuf),
    Deleted(PathBuf),
    Renamed { old: PathBuf, new: PathBuf },
}

fn scan_entry(root: &Path, path: &Path) -> Option<AssetEntry> {
    let content = std::fs::read_to_string(path).ok()?;
    let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let is_master = content.contains("import ") || filename == "master.moa";
    let rel_path = path.strip_prefix(root).unwrap_or(path).to_string_lossy().to_string();

    Some(AssetEntry {
        path: path.to_path_buf(),
        rel_path,
        filename,
        is_master,
    })
}

fn collect_moas(dir: &Path, project_root: &Path, out: &mut Vec<AssetEntry>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "moa") {
            if let Some(e) = scan_entry(project_root, &path) {
                out.push(e);
            }
        }
    }
}

pub struct AssetPanel {
    pub project_path: PathBuf,
    /// The video project new assets / timing edits belong to.
    pub active_project: Option<PathBuf>,
    pub projects: Vec<ProjectEntry>,
    /// Root-level .moa files not inside any project folder.
    pub loose: Vec<AssetEntry>,
    pub search_query: String,
    pub new_video_name: String,
    pub new_asset_name: String,
    /// Inline rename state: the file being renamed + its draft name.
    renaming: Option<PathBuf>,
    rename_text: String,
    /// File awaiting deletion confirmation.
    pending_delete: Option<PathBuf>,
    /// Inline "new asset" field under assets/ is visible.
    creating_asset: bool,
    last_scan: Instant,
}

impl AssetPanel {
    pub fn new(project_path: PathBuf) -> Self {
        let mut panel = Self {
            project_path,
            active_project: None,
            projects: Vec::new(),
            loose: Vec::new(),
            search_query: String::new(),
            new_video_name: String::new(),
            new_asset_name: String::new(),
            renaming: None,
            rename_text: String::new(),
            pending_delete: None,
            creating_asset: false,
            last_scan: Instant::now(),
        };
        panel.rescan();
        panel
    }

    /// Restore the last session's src root + active project from config,
    /// then rescan. Called by the app at startup, not by tests.
    pub fn restore_session(&mut self) {
        if let Ok(cfg) = std::fs::read_to_string(config_path()) {
            for line in cfg.lines() {
                if let Some(v) = line.strip_prefix("src_root = \"").and_then(|s| s.strip_suffix('"')) {
                    let p = PathBuf::from(v);
                    if p.is_dir() {
                        self.project_path = p;
                    }
                } else if let Some(v) = line.strip_prefix("active_project = \"").and_then(|s| s.strip_suffix('"')) {
                    let p = PathBuf::from(v);
                    if p.is_dir() {
                        self.active_project = Some(p);
                    }
                }
            }
        }
        self.rescan();
        if self.active_project.is_none() {
            self.active_project = newest_master_project(&self.project_path);
        }
    }

    pub fn persist(&self) {
        save_config(&self.project_path, self.active_project.as_deref());
    }

    pub fn set_active_project(&mut self, path: PathBuf) {
        self.active_project = Some(path);
        self.persist();
    }

    /// Scan the source folder in every layout:
    /// - project model: `<root>/<project>/master.moa` + `<root>/<project>/assets/*.moa`
    /// - loose .moa files at the root itself
    /// - legacy flat `assets/` / `examples/` folders at the root
    pub fn rescan(&mut self) {
        self.projects.clear();
        self.loose.clear();
        let root = self.project_path.clone();

        let push_project = |name: String, dir: PathBuf, out: &mut Vec<ProjectEntry>| {
            let mut assets = Vec::new();
            collect_moas(&dir.join("assets"), &dir, &mut assets);
            collect_moas(&dir, &dir, &mut assets);
            assets.sort_by(|a, b| b.is_master.cmp(&a.is_master).then_with(|| a.filename.cmp(&b.filename)));
            let master = assets
                .iter()
                .find(|a| a.filename == "master.moa" || a.is_master)
                .cloned();
            out.push(ProjectEntry { name, path: dir, master, assets });
        };

        // One level down: video project folders
        if let Ok(projects) = std::fs::read_dir(&root) {
            for proj in projects.flatten() {
                let path = proj.path();
                if path.is_dir() {
                    let name = proj.file_name().to_string_lossy().into_owned();
                    push_project(name, path, &mut self.projects);
                }
            }
        }

        // Legacy flat folders at the root, shown as projects too
        for legacy in ["assets", "examples"] {
            let dir = root.join(legacy);
            if dir.is_dir() {
                push_project(legacy.to_string(), dir, &mut self.projects);
            }
        }

        self.projects.sort_by(|a, b| a.name.cmp(&b.name));

        // Root-level loose .moa files
        collect_moas(&root, &root, &mut self.loose);
        self.loose.sort_by(|a, b| a.filename.cmp(&b.filename));

        self.last_scan = Instant::now();
    }

    pub fn show(&mut self, ui: &mut eframe::egui::Ui, current_file: Option<&Path>) -> AssetAction {
        use eframe::egui::{self, Align, Layout, RichText, Color32};
        let mut action = AssetAction::None;

        // Auto-rescan: files created/deleted externally (MCP, CLI, file manager)
        // must show up without a manual refresh. Directory scan is cheap.
        if self.last_scan.elapsed().as_secs_f32() > 2.0 {
            self.rescan();
        }

        let file_label = |filename: &str, is_open: bool, indent: bool| {
            let prefix = if indent { "  📄 " } else { "📄 " };
            if is_open {
                RichText::new(format!("{prefix}{filename}"))
                    .color(Color32::from_rgb(0xFF, 0xE0, 0x66)) // Warm gold / high contrast
                    .strong()
            } else {
                RichText::new(format!("{prefix}{filename}"))
                    .color(Color32::from_rgb(0xDD, 0xD4, 0xF5)) // Soft lavender
            }
        };

        ui.horizontal(|ui| {
            ui.heading("Explorer");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("🔄").on_hover_text("Rescan").clicked() {
                    self.rescan();
                }
                if ui.button("📁").on_hover_text("Change source folder").clicked() {
                    if let Some(folder) = rfd::FileDialog::new()
                        .set_directory(&self.project_path)
                        .pick_folder()
                    {
                        action = AssetAction::ChangeSrcHome(folder);
                    }
                }
            });
        });

        // Project switcher
        ui.horizontal(|ui| {
            ui.label(RichText::new("▶").size(11.0).color(Color32::from_rgb(255, 180, 80)));
            let name_of = |p: &Path| {
                p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
            };
            let mut selected = self.active_project.clone();
            egui::ComboBox::from_id_salt("active_project_combo")
                .selected_text(selected.as_ref().map(|p| name_of(p.as_path())).unwrap_or_else(|| "select project...".into()))
                .width(180.0)
                .show_ui(ui, |ui| {
                    for p in &self.projects {
                        ui.selectable_value(&mut selected, Some(p.path.clone()), &p.name);
                    }
                });
            if selected != self.active_project {
                if let Some(p) = selected {
                    action = AssetAction::SelectProject(p);
                }
            }
        });

        ui.add_space(4.0);

        let query = self.search_query.to_lowercase();
        let matches = |text: &str| query.is_empty() || text.to_lowercase().contains(&query);

        egui::ScrollArea::vertical().show(ui, |ui| {
            let active = self.active_project.clone();
            if let Some(proj_path) = &active {
                let project = self.projects.iter().find(|p| &p.path == proj_path);

                // --- master.moa (clickable only) ---
                if let Some(master) = project.and_then(|p| p.master.clone()) {
                    let is_open = current_file == Some(master.path.as_path());
                    ui.horizontal(|ui| {
                        let btn = ui.selectable_label(is_open, file_label(&master.filename, is_open, false));
                        if btn.clicked() {
                            action = AssetAction::Open(master.path.clone());
                        }
                    });
                } else {
                    ui.label(
                        RichText::new("  (no master.moa in this project)")
                            .size(10.5)
                            .italics()
                            .color(Color32::from_rgb(140, 140, 160)),
                    );
                }

                // --- assets/ folder, always open ---
                let entries: Vec<AssetEntry> = project
                    .map(|p| p.assets.iter().filter(|a| !(a.filename == "master.moa")).cloned().collect())
                    .unwrap_or_default();
                let visible: Vec<&AssetEntry> = entries.iter().filter(|a| matches(&a.filename)).collect();

                ui.horizontal(|ui| {
                    let header = ui.add(
                        egui::Label::new(RichText::new("📂 assets").strong().size(12.0)).sense(egui::Sense::click()),
                    );
                    header.context_menu(|ui| {
                        if ui.button("➕ New Asset...").clicked() {
                            self.creating_asset = true;
                            ui.close_menu();
                        }
                    });
                    if ui
                        .add(
                            egui::Button::new("➕").small(),
                        )
                        .clicked()
                    {
                        self.creating_asset = true;
                    }
                });

                if self.creating_asset {
                    ui.indent("new_asset_row", |ui| {
                            ui.label("📝");
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut self.new_asset_name)
                                    .hint_text("asset-name")
                                    .desired_width(140.0),
                            );
                            resp.request_focus();
                            let ok = !self.new_asset_name.trim().is_empty();
                            let commit = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && ok;
                            if commit || ui.add_enabled(ok, egui::Button::new("Create")).clicked() {
                                action = AssetAction::NewAsset { name: self.new_asset_name.clone() };
                                self.creating_asset = false;
                                self.new_asset_name.clear();
                            }
                            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                self.creating_asset = false;
                            }
                        });
                }

                if visible.is_empty() && !self.creating_asset {
                    ui.indent("empty_assets_hint", |ui| {
                        ui.label(
                            RichText::new("(empty — ➕ to add)")
                                .size(10.0)
                                .italics()
                                .color(Color32::from_rgb(140, 140, 160)),
                        );
                    });
                }

                for entry in &visible {
                    let entry = (*entry).clone();
                    ui.indent(entry.path.display().to_string(), |ui| {
                        // Inline rename row
                        if self.renaming.as_deref() == Some(entry.path.as_path()) {
                            ui.horizontal(|ui| {
                                ui.label("✏️");
                                let resp = ui.add(
                                    egui::TextEdit::singleline(&mut self.rename_text)
                                        .desired_width(140.0),
                                );
                                resp.request_focus();
                                let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                let esc = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape));
                                if enter {
                                    let new_name: String = self.rename_text.trim().to_string();
                                    if !new_name.is_empty() && new_name != entry.filename {
                                        let target = entry.path.with_file_name(&new_name);
                                        if std::fs::rename(&entry.path, &target).is_ok() {
                                            action = AssetAction::Renamed { old: entry.path.clone(), new: target };
                                        }
                                    }
                                    self.renaming = None;
                                } else if esc {
                                    self.renaming = None;
                                }
                            });
                            return;
                        }

                        let is_open = current_file == Some(entry.path.as_path());
                        let row = ui.horizontal(|ui| {
                            let btn = ui.selectable_label(is_open, file_label(&entry.filename, is_open, true));
                            if btn.clicked() {
                                action = AssetAction::Open(entry.path.clone());
                            }
                        });
                        row.response.context_menu(|ui| {
                            if ui.button("Open").clicked() {
                                action = AssetAction::Open(entry.path.clone());
                                ui.close_menu();
                            }
                            if ui.button("➕ Insert import into script").clicked() {
                                let clean_alias = entry.filename.trim_end_matches(".moa").replace('-', "_");
                                action = AssetAction::InsertImport { path: entry.rel_path.clone(), alias: clean_alias };
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("✏️ Rename...").clicked() {
                                self.renaming = Some(entry.path.clone());
                                self.rename_text = entry.filename.clone();
                                ui.close_menu();
                            }
                            if ui.button("🗑 Delete...").clicked() {
                                self.pending_delete = Some(entry.path.clone());
                                ui.close_menu();
                            }
                        });
                    });
                }
            }

            // --- loose .moa files at the source root ---
            if !self.loose.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new("loose scenes").strong().size(11.0).color(Color32::from_rgb(120, 130, 150)));
                for entry in self.loose.clone() {
                    ui.indent(format!("loose-{}", entry.path.display()), |ui| {
                        let is_open = current_file == Some(entry.path.as_path());
                        let row = ui.horizontal(|ui| {
                            let btn = ui.selectable_label(is_open, file_label(&entry.filename, is_open, false));
                            if btn.clicked() {
                                action = AssetAction::Open(entry.path.clone());
                            }
                        });
                        row.response.context_menu(|ui| {
                            if ui.button("Open").clicked() {
                                action = AssetAction::Open(entry.path.clone());
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("🗑 Delete...").clicked() {
                                self.pending_delete = Some(entry.path.clone());
                                ui.close_menu();
                            }
                        });
                    });
                }
            }

            ui.add_space(8.0);
            ui.separator();

            // New video (compact, bottom)
            ui.horizontal(|ui| {
                ui.label("🎬");
                let name_resp = ui.add(
                    egui::TextEdit::singleline(&mut self.new_video_name)
                        .hint_text("new-video-name")
                        .desired_width(140.0),
                );
                let can_create = !self.new_video_name.trim().is_empty();
                let create_clicked = ui
                    .add_enabled(can_create, egui::Button::new("New Video"))
                    .on_disabled_hover_text("Type a name first")
                    .clicked();
                if create_clicked
                    || (name_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && can_create)
                {
                    action = AssetAction::NewVideo;
                }
            });
        });

        // Delete confirmation
        if let Some(target) = self.pending_delete.clone() {
            let mut confirmed = false;
            egui::Window::new("Delete file")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ui.ctx(), |ui| {
                    ui.label(format!("Delete {}?", target.display()));
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.button(egui::RichText::new("Delete").color(Color32::from_rgb(255, 100, 100))).clicked() {
                            confirmed = true;
                        }
                        if ui.button("Cancel").clicked() {
                            self.pending_delete = None;
                        }
                    });
                });
            if confirmed {
                if std::fs::remove_file(&target).is_ok() {
                    if self.renaming.as_deref() == Some(target.as_path()) {
                        self.renaming = None;
                    }
                    action = AssetAction::Deleted(target.clone());
                }
                self.pending_delete = None;
                self.rescan();
            }
        }

        action
    }

}

/// Project folder of the most recently modified master.moa under `root`.
pub fn newest_master_project(root: &Path) -> Option<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.file_name().map_or(false, |n| n == "master.moa") {
                    if let Some(t) = e.metadata().and_then(|m| m.modified()).ok() {
                        if best.as_ref().map_or(true, |(bt, _)| t > *bt) {
                            best = Some((t, p.parent()?.to_path_buf()));
                        }
                    }
                }
            }
        }
    }
    best.map(|(_, p)| p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rescan_groups_projects_and_loose_files() {
        let root = std::env::temp_dir().join("moa_panel_test2");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("my-video/assets")).unwrap();
        std::fs::write(root.join("my-video/master.moa"), "circle c r=1;").unwrap();
        std::fs::write(root.join("my-video/assets/part.moa"), "circle p r=1;").unwrap();
        std::fs::write(root.join("loose.moa"), "circle l r=1;").unwrap();
        std::fs::create_dir_all(root.join("assets")).unwrap();
        std::fs::write(root.join("assets/legacy.moa"), "circle g r=1;").unwrap();

        let mut panel = AssetPanel::new(root.clone());
        panel.rescan();

        let names: Vec<&str> = panel.projects.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"my-video"), "{names:?}");
        assert!(names.contains(&"assets"), "{names:?}");

        let video = panel.projects.iter().find(|p| p.name == "my-video").unwrap();
        assert!(video.master.as_ref().map_or(false, |m| m.filename == "master.moa"), "{video:?}");
        assert!(video.assets.iter().any(|a| a.filename == "part.moa"), "{video:?}");

        let legacy = panel.projects.iter().find(|p| p.name == "assets").unwrap();
        assert!(legacy.assets.iter().any(|a| a.filename == "legacy.moa"), "{legacy:?}");

        assert!(panel.loose.iter().any(|a| a.filename == "loose.moa"), "{:?}", panel.loose);

        // rel_path of an asset is project-relative, usable for imports
        let part = video.assets.iter().find(|a| a.filename == "part.moa").unwrap();
        assert_eq!(part.rel_path, "assets/part.moa");

        let _ = std::fs::remove_dir_all(&root);
    }
}
