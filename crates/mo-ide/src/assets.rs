use eframe::egui::{self, Color32};
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

#[derive(Clone, Debug)]
pub struct AssetEntry {
    pub path: PathBuf,
    /// Project folder name this file belongs to (e.g. `master-showcase`).
    pub project: String,
    pub rel_path: String,
    pub filename: String,
    pub is_master: bool,
    pub tags: Vec<String>,
    pub size_bytes: u64,
}

pub enum AssetAction {
    None,
    Open(PathBuf),
    InsertImport { path: String, alias: String },
    NewAsset,
    NewVideo,
    ChangeSrcHome(PathBuf),
}

pub struct AssetPanel {
    pub project_path: PathBuf,
    pub search_query: String,
    pub new_video_name: String,
    pub assets: Vec<AssetEntry>,
    last_scan: Instant,
}

impl AssetPanel {
    pub fn new(project_path: PathBuf) -> Self {
        let mut panel = Self {
            project_path,
            search_query: String::new(),
            new_video_name: String::new(),
            assets: Vec::new(),
            last_scan: Instant::now(),
        };
        panel.rescan();
        panel
    }

    /// Recursive scan of the source home: every `<project>/master.moa` is a
    /// master entry, every `<project>/assets/**.moa` a modular asset.
    pub fn rescan(&mut self) {
        self.assets.clear();
        let root = self.project_path.clone();
        let mut collect_dir = |dir: &Path, project: String, is_master: bool| {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().map_or(false, |ext| ext == "moa") {
                        self.push_entry(&root, &path, project.clone(), is_master);
                    }
                }
            }
        };

        if let Ok(projects) = std::fs::read_dir(&root) {
            for proj in projects.flatten() {
                if !proj.path().is_dir() {
                    continue;
                }
                let name = proj.file_name().to_string_lossy().into_owned();
                collect_dir(&proj.path(), name.clone(), true);
                collect_dir(&proj.path().join("assets"), name, false);
            }
        }

        // Sort: masters first, then project, then filename
        self.assets.sort_by(|a, b| {
            b.is_master
                .cmp(&a.is_master)
                .then_with(|| a.project.cmp(&b.project))
                .then_with(|| a.filename.cmp(&b.filename))
        });
        self.last_scan = Instant::now();
    }

    fn push_entry(&mut self, root: &Path, path: &Path, project: String, is_master: bool) {
        if let Ok(content) = std::fs::read_to_string(path) {
            let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let rel_path = path
                .strip_prefix(root.join(&project))
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            let mut tags = Vec::new();
            if content.contains("tex ") || content.contains("latex ") {
                tags.push("LaTeX".into());
            }
            if content.contains("axes3d") || content.contains("cube") || content.contains("surface3d") || content.contains("sphere") {
                tags.push("3D".into());
            }
            if content.contains("func ") || content.contains("plot ") || content.contains("axes ") {
                tags.push("Math Plot".into());
            }
            if content.contains("transform(") {
                tags.push("Transform".into());
            }

            let size_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            self.assets.push(AssetEntry {
                path: path.to_path_buf(),
                project,
                rel_path,
                filename,
                is_master,
                tags,
                size_bytes,
            });
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, current_file: Option<&Path>) -> AssetAction {
        let mut action = AssetAction::None;

        ui.horizontal(|ui| {
            ui.heading("Videos");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🔄").on_hover_text("Rescan projects").clicked() {
                    self.rescan();
                }
                if ui.button("📁 Browse...").on_hover_text("Use a different source folder").clicked() {
                    if let Some(folder) = rfd::FileDialog::new()
                        .set_directory(&self.project_path)
                        .pick_folder()
                    {
                        self.project_path = folder.clone();
                        self.rescan();
                        action = AssetAction::ChangeSrcHome(folder);
                    }
                }
            });
        });

        ui.add_space(2.0);

        // Source home breadcrumb
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("📁 {}", self.project_path.display()))
                    .size(10.5)
                    .color(Color32::from_rgb(140, 150, 170)),
            );
        });

        ui.add_space(4.0);

        // New video: inline name + button
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
                .on_hover_text("Creates a project folder with master.moa + assets/")
                .clicked();
            if create_clicked || (name_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && can_create) {
                action = AssetAction::NewVideo;
            }
            if ui.button("➕ New Asset").on_hover_text("Create an asset inside the active video's assets/ folder").clicked() {
                action = AssetAction::NewAsset;
            }
        });

        ui.add_space(6.0);

        // Filter / Search Bar
        ui.horizontal(|ui| {
            ui.label("🔍");
            ui.add(egui::TextEdit::singleline(&mut self.search_query).hint_text("Filter videos & assets..."));
            if !self.search_query.is_empty() && ui.small_button("✕").clicked() {
                self.search_query.clear();
            }
        });

        ui.separator();

        let query = self.search_query.to_lowercase();
        let filtered: Vec<&AssetEntry> = self
            .assets
            .iter()
            .filter(|a| {
                query.is_empty()
                    || a.filename.to_lowercase().contains(&query)
                    || a.project.to_lowercase().contains(&query)
                    || a.tags.iter().any(|t| t.to_lowercase().contains(&query))
            })
            .collect();

        let masters: Vec<&&AssetEntry> = filtered.iter().filter(|a| a.is_master).collect();
        let indivs: Vec<&&AssetEntry> = filtered.iter().filter(|a| !a.is_master).collect();

        egui::ScrollArea::vertical().show(ui, |ui| {
            if !masters.is_empty() {
                ui.colored_label(Color32::from_rgb(255, 180, 80), "🎬 Videos (master.moa)");
                for entry in masters {
                    let is_active = current_file.map_or(false, |p| p == entry.path);
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            let label_text = if is_active {
                                format!("▶ {} — {}", entry.project, entry.filename)
                            } else {
                                format!("  {} — {}", entry.project, entry.filename)
                            };

                            let btn = ui.selectable_label(is_active, label_text);
                            if btn.clicked() {
                                action = AssetAction::Open(entry.path.clone());
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(
                                    egui::RichText::new(format!("{}B", entry.size_bytes))
                                        .size(10.0)
                                        .color(Color32::from_rgb(120, 130, 150)),
                                );
                            });
                        });

                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(&entry.rel_path)
                                    .size(10.0)
                                    .color(Color32::from_rgb(110, 130, 160)),
                            );
                            for tag in &entry.tags {
                                ui.label(
                                    egui::RichText::new(format!("[{tag}]"))
                                        .size(9.5)
                                        .color(Color32::from_rgb(130, 200, 240)),
                                );
                            }
                        });
                    });
                    ui.add_space(2.0);
                }
                ui.add_space(6.0);
            }

            ui.colored_label(Color32::from_rgb(88, 196, 221), "📦 Modular Assets");
            if indivs.is_empty() {
                ui.label(
                    egui::RichText::new("No assets yet — create a video, then add assets to its assets/ folder")
                        .size(11.0)
                        .italics()
                        .color(Color32::from_rgb(140, 140, 160)),
                );
            }

            for entry in indivs {
                let is_active = current_file.map_or(false, |p| p == entry.path);
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        let label_text = if is_active {
                            format!("▶ [{}] {}", entry.project, entry.filename)
                        } else {
                            format!("  [{}] {}", entry.project, entry.filename)
                        };

                        let btn = ui.selectable_label(is_active, label_text);
                        if btn.clicked() {
                            action = AssetAction::Open(entry.path.clone());
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let clean_alias = entry
                                .filename
                                .trim_end_matches(".moa")
                                .replace('-', "_");

                            if ui.small_button("➕ Import").on_hover_text("Insert import statement into the current script").clicked() {
                                action = AssetAction::InsertImport {
                                    path: entry.rel_path.clone(),
                                    alias: clean_alias,
                                };
                            }
                        });
                    });

                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(&entry.rel_path)
                                .size(10.0)
                                .color(Color32::from_rgb(110, 130, 160)),
                        );
                        for tag in &entry.tags {
                            ui.label(
                                egui::RichText::new(format!("[{tag}]"))
                                    .size(9.5)
                                    .color(Color32::from_rgb(130, 200, 240)),
                            );
                        }
                    });
                });
                ui.add_space(2.0);
            }
        });

        action
    }
}
