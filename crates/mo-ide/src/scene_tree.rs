use eframe::egui::{self, Color32};
use mo_core::scene::Timeline;
use mo_core::vgroup::VGroup;
use std::collections::HashMap;

pub struct SceneTreePanel;

impl SceneTreePanel {
    pub fn show(
        ui: &mut egui::Ui,
        timeline: Option<&Timeline>,
        current_state: &HashMap<String, VGroup>,
        selected_mobject: &mut Option<String>,
    ) {
        ui.horizontal(|ui| {
            ui.heading("Scene Tree");
            if let Some(tl) = timeline {
                ui.label(format!("({} objects)", tl.initial_objects().len()));
            }
        });

        ui.separator();

        let Some(tl) = timeline else {
            ui.label("No active scene loaded.");
            return;
        };

        let initial_objs = tl.initial_objects();
        if initial_objs.is_empty() {
            ui.label("Scene has no mobjects defined.");
            return;
        }

        // Sort keys for deterministic display order
        let mut ids: Vec<_> = initial_objs.keys().collect();
        ids.sort();

        egui::ScrollArea::vertical().show(ui, |ui| {
            for id in ids {
                let is_visible = current_state.contains_key(id);
                let current_group = current_state.get(id).unwrap_or(&initial_objs[id]);
                let is_selected = selected_mobject.as_ref() == Some(id);

                let (bounds_min, bounds_max) = current_group.bounds();
                let width = (bounds_max[0] - bounds_min[0]).max(0.0);
                let height = (bounds_max[1] - bounds_min[1]).max(0.0);
                let center = current_group.center();

                let mut item_frame = egui::Frame::new()
                    .inner_margin(4.0)
                    .corner_radius(4.0);

                if is_selected {
                    item_frame = item_frame
                        .fill(Color32::from_rgb(45, 60, 80))
                        .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(88, 196, 221)));
                }

                item_frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Visibility icon
                        if is_visible {
                            ui.colored_label(Color32::from_rgb(88, 196, 221), "👁");
                        } else {
                            ui.colored_label(Color32::from_rgb(100, 100, 100), "🚫");
                        }

                        // Selectable label for the ID
                        let label_text = egui::RichText::new(id).strong();
                        let label = if is_selected {
                            label_text.color(Color32::from_rgb(88, 196, 221))
                        } else if is_visible {
                            label_text.color(Color32::from_rgb(220, 220, 240))
                        } else {
                            label_text.color(Color32::from_rgb(120, 120, 140))
                        };

                        if ui.selectable_label(is_selected, label).clicked() {
                            if is_selected {
                                *selected_mobject = None;
                            } else {
                                *selected_mobject = Some(id.clone());
                            }
                        }

                        // Metadata badges on the right
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let n_pts = current_group.points.len();
                            ui.label(
                                egui::RichText::new(format!("{} pts", n_pts))
                                    .size(11.0)
                                    .color(Color32::from_rgb(130, 130, 150)),
                            );
                        });
                    });

                    // Secondary details line
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "pos: ({:.2}, {:.2})  size: {:.2}×{:.2}",
                                center.x, center.y, width, height
                            ))
                            .size(11.0)
                            .color(Color32::from_rgb(100, 110, 130)),
                        );
                    });
                });

                ui.add_space(2.0);
            }
        });
    }
}
