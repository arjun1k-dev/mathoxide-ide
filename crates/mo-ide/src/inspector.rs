use eframe::egui::{self, Color32};
use mo_core::camera::Camera;
use mo_core::scene::Timeline;
use mo_core::vgroup::VGroup;
use std::collections::HashMap;

pub enum InspectorAction {
    None,
    ApplyShift { id: String, dx: f32, dy: f32 },
}

/// Runtime inspector and live override manager.
pub struct InspectorPanel;

impl InspectorPanel {
    pub fn show(
        ui: &mut egui::Ui,
        timeline: Option<&Timeline>,
        current_state: &HashMap<String, VGroup>,
        selected_mobject: &mut Option<String>,
        overrides: &mut HashMap<String, (f32, f32)>,
        camera: &mut Camera,
        current_time: f32,
    ) -> InspectorAction {
        let mut action = InspectorAction::None;

        ui.heading("Inspector");
        ui.separator();

        if let Some(id) = selected_mobject.clone() {
            // Check if object exists in the scene
            let Some(group) = current_state.get(&id) else {
                ui.label(format!("Mobject '{}' is currently inactive or hidden.", id));
                if ui.button("Deselect").clicked() {
                    *selected_mobject = None;
                }
                return action;
            };

            ui.horizontal(|ui| {
                ui.colored_label(Color32::from_rgb(88, 196, 221), "Selected:");
                ui.strong(&id);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("✕").clicked() {
                        *selected_mobject = None;
                    }
                });
            });

            ui.separator();

            // Transform section
            ui.collapsing("Transform", |ui| {
                let center = group.center();
                let current_shift = overrides.get(&id).copied().unwrap_or((0.0, 0.0));

                ui.label("Position (Scene Coordinates):");
                let mut x = center.x;
                let mut y = center.y;

                let mut changed = false;
                ui.horizontal(|ui| {
                    ui.label("X:");
                    if ui.add(egui::DragValue::new(&mut x).speed(0.05)).changed() {
                        changed = true;
                    }
                    ui.label("Y:");
                    if ui.add(egui::DragValue::new(&mut y).speed(0.05)).changed() {
                        changed = true;
                    }
                });

                if changed {
                    let dx = x - (center.x - current_shift.0);
                    let dy = y - (center.y - current_shift.1);
                    overrides.insert(id.clone(), (dx, dy));
                }

                if overrides.contains_key(&id) {
                    ui.add_space(4.0);
                    ui.colored_label(
                        Color32::from_rgb(255, 158, 100),
                        format!("Shift Override: ({:+.2}, {:+.2})", current_shift.0, current_shift.1),
                    );
                    ui.horizontal(|ui| {
                        if ui.button("💾 Apply to Code").on_hover_text("Write updated position into editor code").clicked() {
                            action = InspectorAction::ApplyShift {
                                id: id.clone(),
                                dx: current_shift.0,
                                dy: current_shift.1,
                            };
                        }
                        if ui.button("Reset Shift").clicked() {
                            overrides.remove(&id);
                        }
                    });
                }
            });

            ui.add_space(6.0);

            // Styling / Appearance section
            ui.collapsing("Appearance", |ui| {
                // Find primary fill and stroke colors
                let fill = group
                    .points
                    .iter()
                    .find(|p| p.fill_rgba[3] > 0.0)
                    .map(|p| p.fill_rgba)
                    .unwrap_or([0.0, 0.0, 0.0, 0.0]);

                let stroke = group
                    .points
                    .first()
                    .map(|p| (p.stroke_rgba, p.stroke_width))
                    .unwrap_or(([1.0, 1.0, 1.0, 1.0], 2.0));

                // Fill color display
                let fill_c32 = Color32::from_rgba_unmultiplied(
                    (fill[0] * 255.0) as u8,
                    (fill[1] * 255.0) as u8,
                    (fill[2] * 255.0) as u8,
                    (fill[3] * 255.0) as u8,
                );
                ui.horizontal(|ui| {
                    ui.label("Fill Color:");
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 3.0, fill_c32);
                    ui.label(format!(
                        "#{:02X}{:02X}{:02X} (alpha: {:.2})",
                        fill_c32.r(), fill_c32.g(), fill_c32.b(), fill[3]
                    ));
                });

                // Stroke color display
                let stroke_c32 = Color32::from_rgba_unmultiplied(
                    (stroke.0[0] * 255.0) as u8,
                    (stroke.0[1] * 255.0) as u8,
                    (stroke.0[2] * 255.0) as u8,
                    (stroke.0[3] * 255.0) as u8,
                );
                ui.horizontal(|ui| {
                    ui.label("Stroke Color:");
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 3.0, stroke_c32);
                    ui.label(format!(
                        "#{:02X}{:02X}{:02X} (width: {:.1})",
                        stroke_c32.r(), stroke_c32.g(), stroke_c32.b(), stroke.1
                    ));
                });
            });

            ui.add_space(6.0);

            // Geometry Details
            ui.collapsing("Geometry", |ui| {
                let (min, max) = group.bounds();
                ui.label(format!("Points: {}", group.points.len()));
                ui.label(format!("Subpaths: {}", group.subpaths.len().max(1)));
                ui.label(format!("Width:  {:.2}", (max[0] - min[0]).max(0.0)));
                ui.label(format!("Height: {:.2}", (max[1] - min[1]).max(0.0)));
                ui.label(format!("Min Bounds: ({:.2}, {:.2})", min[0], min[1]));
                ui.label(format!("Max Bounds: ({:.2}, {:.2})", max[0], max[1]));
            });

            ui.add_space(8.0);

            if let Some(shift) = overrides.get(&id) {
                if ui.button("📋 Copy Shift to Clipboard").clicked() {
                    let dsl_snippet = format!("at: ({:+.2}, {:+.2})", shift.0, shift.1);
                    ui.ctx().copy_text(dsl_snippet);
                }
            }
        } else {
            // General Scene Info when nothing is selected
            ui.label("Click on an object in the Viewport or Scene Tree to inspect and edit its properties.");
            ui.separator();

            if let Some(tl) = timeline {
                ui.collapsing("Scene Info", |ui| {
                    ui.label(format!("Duration: {:.2}s", tl.duration()));
                    ui.label(format!("Mobjects: {}", tl.initial_objects().len()));
                    ui.label(format!("Animations: {}", tl.timed_anims().len()));
                    ui.label(format!("Statements: {}", tl.stmts.len()));
                    ui.label(format!("Current Time: {:.2}s", current_time));
                    ui.label(format!("Current Frame: {}", (current_time * 60.0).round() as u32));
                });

                ui.add_space(6.0);

                ui.collapsing("Camera (2D & 3D)", |ui| {
                    ui.label(format!("Center: ({:.2}, {:.2})", camera.center.x, camera.center.y));
                    ui.label(format!("Frame Height: {:.2}", camera.frame_height));
                    ui.label(format!("Aspect Ratio: {:.2}", camera.aspect_ratio()));

                    ui.add_space(4.0);
                    ui.label("3D Orbit Orientation:");
                    let mut phi_deg = camera.phi.to_degrees();
                    if ui.add(egui::Slider::new(&mut phi_deg, -89.0..=89.0).text("Tilt φ (°)")).changed() {
                        camera.phi = phi_deg.to_radians();
                    }
                    let mut theta_deg = camera.theta.to_degrees();
                    if ui.add(egui::Slider::new(&mut theta_deg, -180.0..=180.0).text("Azimuth θ (°)")).changed() {
                        camera.theta = theta_deg.to_radians();
                    }
                    let mut gamma_deg = camera.gamma.to_degrees();
                    if ui.add(egui::Slider::new(&mut gamma_deg, -180.0..=180.0).text("Roll γ (°)")).changed() {
                        camera.gamma = gamma_deg.to_radians();
                    }

                    ui.add_space(4.0);
                    if ui.button("Reset Camera to 2D (R)").clicked() {
                        camera.reset();
                    }
                });

                ui.add_space(6.0);

                if !overrides.is_empty() {
                    ui.collapsing(format!("Active Overrides ({})", overrides.len()), |ui| {
                        for (ov_id, shift) in overrides.iter() {
                            ui.label(format!("{}: ({:+.2}, {:+.2})", ov_id, shift.0, shift.1));
                        }
                        if ui.button("Clear All Overrides").clicked() {
                            overrides.clear();
                        }
                    });
                }
            } else {
                ui.label("No active scene loaded.");
            }
        }

        action
    }
}
