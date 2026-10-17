use eframe::egui::{self, Color32, Pos2, Rect, Stroke};
use mo_core::camera::Camera;
use mo_core::scene::{Player, Timeline};
use mo_render::Renderer;
use std::collections::HashMap;

pub struct ViewportPanel {
    pub texture_id: egui::TextureId,
    last_cursor_pos: Option<Pos2>,
    is_dragging_mob: bool,
    is_panning: bool,
    drag_mob_offset: (f32, f32),
}

impl ViewportPanel {
    pub fn new(
        render_state: &egui_wgpu::RenderState,
        renderer: &Renderer,
    ) -> Self {
        let texture_id = render_state.renderer.write().register_native_texture(
            &render_state.device,
            renderer.target_view(),
            wgpu::FilterMode::Linear,
        );

        Self {
            texture_id,
            last_cursor_pos: None,
            is_dragging_mob: false,
            is_panning: false,
            drag_mob_offset: (0.0, 0.0),
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        render_state: &egui_wgpu::RenderState,
        renderer: &mut Renderer,
        timeline: Option<&Timeline>,
        current_time: f32,
        camera: &mut Camera,
        selected_mobject: &mut Option<String>,
        overrides: &mut HashMap<String, (f32, f32)>,
        fps: f32,
    ) {
        let avail_size = ui.available_size();
        let target_w = (avail_size.x.round() as u32).max(64);
        let target_h = (avail_size.y.round() as u32).max(64);

        // Resize renderer if viewport panel size changed
        if renderer.width() != target_w || renderer.height() != target_h {
            renderer.resize(target_w, target_h);
            render_state.renderer.write().update_egui_texture_from_wgpu_texture(
                &render_state.device,
                renderer.target_view(),
                wgpu::FilterMode::Linear,
                self.texture_id,
            );
        }

        camera.width = target_w as f32;
        camera.height = target_h as f32;

        // Evaluate scene state at current time
        let mut state = if let Some(tl) = timeline {
            Player::state_at(tl, current_time)
        } else {
            HashMap::new()
        };

        // Apply shifts
        for (id, (dx, dy)) in overrides.iter() {
            if let Some(g) = state.get_mut(id) {
                g.shift(glam::Vec3::new(*dx, *dy, 0.0));
            }
        }

        // Render scene to offscreen target
        let mut encoder = renderer.device().create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("viewport scene encoder"),
        });
        renderer.draw_scene_pass(&state, camera, &mut encoder);
        renderer.queue().submit(Some(encoder.finish()));

        // Display texture in egui
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(target_w as f32, target_h as f32),
            egui::Sense::click_and_drag(),
        );

        let painter = ui.painter_at(rect);

        // Draw rendered image
        painter.image(
            self.texture_id,
            rect,
            Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );

        // Handle user input
        let cursor_in_screen = response.hover_pos();
        let mut cursor_scene_pos = None;

        if let Some(pos) = cursor_in_screen {
            let local_x = pos.x - rect.left();
            let local_y = pos.y - rect.top();
            let sc = camera.screen_to_scene(local_x, local_y);
            cursor_scene_pos = Some(sc);

            // Zoom via mouse wheel
            let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
            if scroll_delta != 0.0 && response.hovered() {
                camera.height = (camera.height * (1.0 - scroll_delta * 0.002)).clamp(0.1, 100.0);
            }

            // Click handling
            if response.clicked() {
                // Hit test mobjects
                let mut clicked_id = None;
                for (id, group) in state.iter() {
                    let (min, max) = group.bounds();
                    if sc.x >= min[0] && sc.x <= max[0] && sc.y >= min[1] && sc.y <= max[1] {
                        clicked_id = Some(id.clone());
                        break;
                    }
                }
                *selected_mobject = clicked_id;
            }

            // Drag handling
            let is_alt_down = ui.input(|i| i.modifiers.alt);
            let is_middle_down = ui.input(|i| i.pointer.middle_down());
            let is_right_down = ui.input(|i| i.pointer.secondary_down());
            let _is_primary_drag = response.dragged_by(egui::PointerButton::Primary);

            if response.drag_started() {
                if is_middle_down || is_right_down || is_alt_down || selected_mobject.is_none() {
                    self.is_panning = true;
                    self.is_dragging_mob = false;
                } else if let Some(id) = selected_mobject {
                    if let Some(group) = state.get(id) {
                        let (min, max) = group.bounds();
                        if sc.x >= min[0] && sc.x <= max[0] && sc.y >= min[1] && sc.y <= max[1] {
                            self.is_dragging_mob = true;
                            self.is_panning = false;
                            let c = group.center();
                            self.drag_mob_offset = (c.x - sc.x, c.y - sc.y);
                        } else {
                            self.is_panning = true;
                            self.is_dragging_mob = false;
                        }
                    }
                }
            }

            if response.drag_stopped() {
                self.is_panning = false;
                self.is_dragging_mob = false;
            }

            if let Some(prev) = self.last_cursor_pos {
                if self.is_panning {
                    let p1 = camera.screen_to_scene(prev.x - rect.left(), prev.y - rect.top());
                    let p2 = camera.screen_to_scene(local_x, local_y);
                    camera.center.x -= p2.x - p1.x;
                    camera.center.y -= p2.y - p1.y;
                } else if self.is_dragging_mob {
                    if let Some(id) = selected_mobject {
                        if let Some(tl) = timeline {
                            // Find base position from timeline without overrides
                            let base = Player::state_at(tl, current_time);
                            let base_center = base.get(id).map(|g| g.center()).unwrap_or(glam::Vec3::ZERO);
                            let target_x = sc.x + self.drag_mob_offset.0;
                            let target_y = sc.y + self.drag_mob_offset.1;
                            let dx = target_x - base_center.x;
                            let dy = target_y - base_center.y;
                            overrides.insert(id.clone(), (dx, dy));
                        }
                    }
                }
            }

            self.last_cursor_pos = Some(pos);
        } else {
            self.last_cursor_pos = None;
        }

        // Draw Selection Bounding Box on top of Viewport
        if let Some(id) = selected_mobject {
            if let Some(group) = state.get(id) {
                let (min, max) = group.bounds();
                // Map scene coordinates to viewport screen coords
                let p_min = camera.scene_to_screen(min[0], min[1]);
                let p_max = camera.scene_to_screen(max[0], max[1]);

                let screen_min = Pos2::new(rect.left() + p_min.x, rect.top() + p_min.y);
                let screen_max = Pos2::new(rect.left() + p_max.x, rect.top() + p_max.y);

                let box_rect = Rect::from_two_pos(screen_min, screen_max);

                painter.rect_stroke(
                    box_rect.expand(4.0),
                    3.0,
                    Stroke::new(1.5_f32, Color32::from_rgb(88, 196, 221)),
                    egui::StrokeKind::Inside,
                );

                // Small corner handle markers
                let corners = [
                    box_rect.left_top(),
                    box_rect.right_top(),
                    box_rect.left_bottom(),
                    box_rect.right_bottom(),
                ];
                for c in corners {
                    painter.circle_filled(c, 3.5, Color32::from_rgb(88, 196, 221));
                }

                // ID Tag
                painter.text(
                    Pos2::new(box_rect.left() + 2.0, box_rect.top() - 14.0),
                    egui::Align2::LEFT_BOTTOM,
                    id,
                    egui::FontId::monospace(12.0),
                    Color32::from_rgb(88, 196, 221),
                );
            }
        }

        // HUD Overlays
        // Top-left: Resolution & Scene
        painter.text(
            Pos2::new(rect.left() + 10.0, rect.top() + 10.0),
            egui::Align2::LEFT_TOP,
            format!(
                "MathOxide | {}×{}",
                target_w, target_h
            ),
            egui::FontId::monospace(12.0),
            Color32::from_rgba_unmultiplied(220, 220, 240, 180),
        );

        // Top-right: FPS & Camera Zoom
        painter.text(
            Pos2::new(rect.right() - 10.0, rect.top() + 10.0),
            egui::Align2::RIGHT_TOP,
            format!("{:.0} FPS | Zoom: {:.2}x", fps, 8.0 / camera.height),
            egui::FontId::monospace(12.0),
            Color32::from_rgba_unmultiplied(220, 220, 240, 180),
        );

        // Bottom-left: Scene coordinates under cursor
        if let Some(sc) = cursor_scene_pos {
            painter.text(
                Pos2::new(rect.left() + 10.0, rect.bottom() - 10.0),
                egui::Align2::LEFT_BOTTOM,
                format!("Cursor: ({:+.2}, {:+.2})", sc.x, sc.y),
                egui::FontId::monospace(12.0),
                Color32::from_rgba_unmultiplied(120, 200, 255, 180),
            );
        }
    }
}
