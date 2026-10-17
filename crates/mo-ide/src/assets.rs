use eframe::egui::{self, Color32};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct AssetEntry {
    pub path: PathBuf,
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
    NewMaster,
    ChangeProjectDir(PathBuf),
}

pub struct AssetPanel {
    pub project_path: PathBuf,
    pub search_query: String,
    pub assets: Vec<AssetEntry>,
    last_scan: Instant,
}

impl AssetPanel {
    pub fn new(project_path: PathBuf) -> Self {
        let mut panel = Self {
            project_path,
            search_query: String::new(),
            assets: Vec::new(),
            last_scan: Instant::now(),
        };
        panel.rescan();
        panel
    }

    pub fn rescan(&mut self) {
        self.assets.clear();
        let dirs_to_scan = [
            self.project_path.join("assets"),
            self.project_path.join("examples"),
            self.project_path.clone(),
        ];

        let mut seen = std::collections::HashSet::new();

        for dir in &dirs_to_scan {
            if !dir.exists() {
                continue;
            }
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().map_or(false, |ext| ext == "moa") {
                        if seen.insert(path.clone()) {
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                                let rel_path = path
                                    .strip_prefix(&self.project_path)
                                    .unwrap_or(&path)
                                    .to_string_lossy()
                                    .to_string();

                                let is_master = content.contains("import ");
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

                                let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);

                                self.assets.push(AssetEntry {
                                    path,
                                    rel_path,
                                    filename,
                                    is_master,
                                    tags,
                                    size_bytes,
                                });
                            }
                        }
                    }
                }
            }
        }

        // Sort: masters first, then alphabetical by rel_path
        self.assets.sort_by(|a, b| {
            b.is_master.cmp(&a.is_master).then_with(|| a.rel_path.cmp(&b.rel_path))
        });
        self.last_scan = Instant::now();
    }

    pub fn show(&mut self, ui: &mut egui::Ui, current_file: Option<&Path>) -> AssetAction {
        let mut action = AssetAction::None;

        ui.horizontal(|ui| {
            ui.heading("Asset Browser");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🔄").on_hover_text("Rescan directory").clicked() {
                    self.rescan();
                }
                if ui.button("📁 Browse...").on_hover_text("Open folder on laptop").clicked() {
                    if let Some(folder) = rfd::FileDialog::new()
                        .set_directory(&self.project_path)
                        .pick_folder()
                    {
                        self.project_path = folder.clone();
                        self.rescan();
                        action = AssetAction::ChangeProjectDir(folder);
                    }
                }
            });
        });

        ui.add_space(2.0);

        // Project folder breadcrumb
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("📁 {}", self.project_path.display()))
                    .size(10.5)
                    .color(Color32::from_rgb(140, 150, 170)),
            );
        });

        ui.add_space(4.0);

        // Action buttons: New Asset and New Master Scene
        ui.horizontal(|ui| {
            if ui.button("➕ New Asset").on_hover_text("Create isolated component asset in assets/").clicked() {
                action = AssetAction::NewAsset;
            }
            if ui.button("🎬 New Master").on_hover_text("Create master composition file").clicked() {
                action = AssetAction::NewMaster;
            }
        });

        ui.add_space(6.0);

        // Filter / Search Bar
        ui.horizontal(|ui| {
            ui.label("🔍");
            ui.add(egui::TextEdit::singleline(&mut self.search_query).hint_text("Filter assets..."));
            if !self.search_query.is_empty() && ui.small_button("✕").clicked() {
                self.search_query.clear();
            }
        });

        ui.separator();

        // Separate assets into Master Compositions vs Isolated Assets
        let query = self.search_query.to_lowercase();
        let filtered: Vec<&AssetEntry> = self
            .assets
            .iter()
            .filter(|a| {
                if query.is_empty() {
                    true
                } else {
                    a.filename.to_lowercase().contains(&query)
                        || a.rel_path.to_lowercase().contains(&query)
                        || a.tags.iter().any(|t| t.to_lowercase().contains(&query))
                }
            })
            .collect();

        let masters: Vec<&&AssetEntry> = filtered.iter().filter(|a| a.is_master).collect();
        let indivs: Vec<&&AssetEntry> = filtered.iter().filter(|a| !a.is_master).collect();

        egui::ScrollArea::vertical().show(ui, |ui| {
            if !masters.is_empty() {
                ui.colored_label(Color32::from_rgb(255, 180, 80), "🎬 Master Compositions");
                for entry in masters {
                    let is_active = current_file.map_or(false, |p| p == entry.path);
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            let label_text = if is_active {
                                format!("▶ {}", entry.filename)
                            } else {
                                format!("  {}", entry.filename)
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
                    egui::RichText::new("No modular assets found in assets/ or examples/")
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
                            format!("▶ {}", entry.filename)
                        } else {
                            format!("  {}", entry.filename)
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

                            if ui.small_button("➕ Import").on_hover_text("Insert import statement into current script").clicked() {
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
