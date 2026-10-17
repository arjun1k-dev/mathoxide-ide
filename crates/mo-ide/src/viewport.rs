use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use mo_core::camera::Camera;
use mo_core::scene::{Player, Timeline};
use mo_render::Renderer;
use std::collections::HashMap;

pub struct ViewportPanel {
    pub texture_id: egui::TextureId,
    last_cursor_pos: Option<Pos2>,
    is_dragging_mob: bool,
    is_panning: bool,
    is_orbiting: bool,
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
            is_orbiting: false,
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
        let padding = 12.0;
        let avail_w = (avail_size.x - padding * 2.0).max(160.0);
        let avail_h = (avail_size.y - padding * 2.0).max(90.0);

        // Standard 16:9 widescreen composition aspect ratio
        let target_aspect = 16.0 / 9.0;
        let (canvas_w, canvas_h) = if avail_w / avail_h > target_aspect {
            let h = avail_h;
            let w = (h * target_aspect).round();
            (w, h)
        } else {
            let w = avail_w;
            let h = (w / target_aspect).round();
            (w, h)
        };

        let target_w = (canvas_w as u32).max(64);
        let target_h = (canvas_h as u32).max(36);

        // Keep camera dimensions in sync with viewport canvas
        camera.width = canvas_w;
        camera.height = canvas_h;

        // Resize renderer if viewport canvas size changed
        if renderer.width() != target_w || renderer.height() != target_h {
            renderer.resize(target_w, target_h);
            render_state.renderer.write().update_egui_texture_from_wgpu_texture(
                &render_state.device,
                renderer.target_view(),
                wgpu::FilterMode::Linear,
                self.texture_id,
            );
        }

        // Evaluate scene state at current time
        let mut state = if let Some(tl) = timeline {
            Player::state_at(tl, current_time)
        } else {
            HashMap::new()
        };

        // Animated camera (camera.orbit / camera.zoom / camera.move_to) overrides
        // the interactive base camera while the timeline defines camera motion.
        let has_camera_anims = timeline.map(|tl| {
            tl.stmts.iter().any(|s| {
                matches!(s, mo_core::scene::Stmt::Play(a) if matches!(
                    a.anim,
                    mo_core::anim::Animation::CameraOrbit { .. }
                        | mo_core::anim::Animation::CameraZoom { .. }
                        | mo_core::anim::Animation::CameraMoveTo { .. }
                ))
            })
        }).unwrap_or(false);
        let cam_owned = if has_camera_anims {
            Player::camera_at(timeline.unwrap(), current_time, camera)
        } else {
            camera.clone()
        };
        let cam: &Camera = &cam_owned;

        // Apply visual shift overrides
        for (id, (dx, dy)) in overrides.iter() {
            if let Some(g) = state.get_mut(id) {
                g.shift(glam::Vec3::new(*dx, *dy, 0.0));
            }
        }

        // Render scene to offscreen target
        let mut encoder = renderer.device().create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("viewport scene encoder"),
        });
        renderer.draw_scene_pass(&state, cam, &mut encoder);
        renderer.queue().submit(Some(encoder.finish()));

        // Center canvas inside available panel area
        let offset_x = ((avail_size.x - canvas_w) * 0.5).max(padding);
        let offset_y = ((avail_size.y - canvas_h) * 0.5).max(padding);

        let screen_rect = ui.available_rect_before_wrap();
        let canvas_rect = Rect::from_min_size(
            Pos2::new(screen_rect.left() + offset_x, screen_rect.top() + offset_y),
            Vec2::new(canvas_w, canvas_h),
        );

        // Register interaction response on canvas
        let response = ui.interact(canvas_rect, ui.id().with("viewport_canvas"), egui::Sense::click_and_drag());
        let painter = ui.painter_at(screen_rect);

        // Draw letterbox background shadow / border around canvas
        painter.rect_filled(
            canvas_rect.expand(2.0),
            4.0,
            Color32::from_rgb(32, 36, 48),
        );

        // Draw rendered image
        painter.image(
            self.texture_id,
            canvas_rect,
            Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );

        // Draw subtle border around video canvas
        painter.rect_stroke(
            canvas_rect,
            0.0,
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(100, 120, 160, 60)),
            egui::StrokeKind::Outside,
        );

        // Handle user input
        let cursor_in_screen = response.hover_pos();
        let mut cursor_scene_pos = None;

        if let Some(pos) = cursor_in_screen {
            let local_x = pos.x - canvas_rect.left();
            let local_y = pos.y - canvas_rect.top();
            let sc = cam.screen_to_scene(local_x, local_y);
            cursor_scene_pos = Some(sc);

            // Zoom via mouse wheel
            let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
            if scroll_delta != 0.0 && response.hovered() {
                camera.frame_height = (camera.frame_height * (1.0 - scroll_delta * 0.002)).clamp(0.1, 100.0);
            }

            // Click handling
            if response.clicked() {
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

            // Drag handling modifiers
            let is_alt_down = ui.input(|i| i.modifiers.alt);
            let is_shift_down = ui.input(|i| i.modifiers.shift);
            let is_middle_down = ui.input(|i| i.pointer.middle_down());
            let is_right_down = ui.input(|i| i.pointer.secondary_down());

            if response.drag_started() {
                if is_right_down || is_alt_down {
                    // Right-click or Alt-drag = 3D camera orbit
                    self.is_orbiting = true;
                    self.is_panning = false;
                    self.is_dragging_mob = false;
                } else if is_middle_down || is_shift_down || selected_mobject.is_none() {
                    // Middle-click, Shift-drag, or dragging empty space = Camera pan
                    self.is_panning = true;
                    self.is_orbiting = false;
                    self.is_dragging_mob = false;
                } else if let Some(id) = selected_mobject {
                    if let Some(group) = state.get(id) {
                        let (min, max) = group.bounds();
                        if sc.x >= min[0] && sc.x <= max[0] && sc.y >= min[1] && sc.y <= max[1] {
                            self.is_dragging_mob = true;
                            self.is_panning = false;
                            self.is_orbiting = false;
                            let c = group.center();
                            self.drag_mob_offset = (c.x - sc.x, c.y - sc.y);
                        } else {
                            self.is_panning = true;
                            self.is_orbiting = false;
                            self.is_dragging_mob = false;
                        }
                    }
                }
            }

            if response.drag_stopped() {
                self.is_panning = false;
                self.is_orbiting = false;
                self.is_dragging_mob = false;
            }

            if let Some(prev) = self.last_cursor_pos {
                let dx = pos.x - prev.x;
                let dy = pos.y - prev.y;

                if self.is_orbiting {
                    // Orbit 3D camera
                    camera.orbit(-dy * 0.008, -dx * 0.008);
                } else if self.is_panning {
                    let p1 = camera.screen_to_scene(prev.x - canvas_rect.left(), prev.y - canvas_rect.top());
                    let p2 = camera.screen_to_scene(local_x, local_y);
                    camera.center.x -= p2.x - p1.x;
                    camera.center.y -= p2.y - p1.y;
                } else if self.is_dragging_mob {
                    if let Some(id) = selected_mobject {
                        if let Some(tl) = timeline {
                            let base = Player::state_at(tl, current_time);
                            let base_center = base.get(id).map(|g| g.center()).unwrap_or(glam::Vec3::ZERO);
                            let target_x = sc.x + self.drag_mob_offset.0;
                            let target_y = sc.y + self.drag_mob_offset.1;
                            let shift_x = target_x - base_center.x;
                            let shift_y = target_y - base_center.y;
                            overrides.insert(id.clone(), (shift_x, shift_y));
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
                let p_min = cam.scene_to_screen(min[0], min[1]);
                let p_max = cam.scene_to_screen(max[0], max[1]);

                let screen_min = Pos2::new(canvas_rect.left() + p_min.x, canvas_rect.top() + p_min.y);
                let screen_max = Pos2::new(canvas_rect.left() + p_max.x, canvas_rect.top() + p_max.y);

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
        // Top-left: Resolution & Aspect ratio
        painter.text(
            Pos2::new(canvas_rect.left() + 10.0, canvas_rect.top() + 10.0),
            egui::Align2::LEFT_TOP,
            format!("16:9 Widescreen | {}×{}", target_w, target_h),
            egui::FontId::monospace(11.0),
            Color32::from_rgba_unmultiplied(220, 225, 240, 190),
        );

        // Top-right: FPS & 3D Orbit Status
        let is_3d = cam.phi.abs() > 0.01 || cam.theta.abs() > 0.01;
        let camera_status = if is_3d {
            format!(
                "🎥 3D Orbit: φ {}° θ {}° | {:.0} FPS",
                cam.phi.to_degrees().round() as i32,
                cam.theta.to_degrees().round() as i32,
                fps
            )
        } else {
            format!("🎥 2D View | Zoom {:.2}x | {:.0} FPS", 8.0 / cam.frame_height, fps)
        };

        painter.text(
            Pos2::new(canvas_rect.right() - 10.0, canvas_rect.top() + 10.0),
            egui::Align2::RIGHT_TOP,
            camera_status,
            egui::FontId::monospace(11.0),
            if is_3d {
                Color32::from_rgb(255, 180, 80)
            } else {
                Color32::from_rgba_unmultiplied(220, 225, 240, 190)
            },
        );

        // Bottom-left: Scene coordinates under cursor
        if let Some(sc) = cursor_scene_pos {
            painter.text(
                Pos2::new(canvas_rect.left() + 10.0, canvas_rect.bottom() - 10.0),
                egui::Align2::LEFT_BOTTOM,
                format!("Cursor: ({:+.2}, {:+.2})", sc.x, sc.y),
                egui::FontId::monospace(11.0),
                Color32::from_rgba_unmultiplied(120, 200, 255, 190),
            );
        }

        // Bottom-right: Shortcut hints
        painter.text(
            Pos2::new(canvas_rect.right() - 10.0, canvas_rect.bottom() - 10.0),
            egui::Align2::RIGHT_BOTTOM,
            "Pan: Shift+Drag | 3D Orbit: Right-Drag / Alt+Drag | Reset: R",
            egui::FontId::monospace(10.0),
            Color32::from_rgba_unmultiplied(150, 160, 180, 140),
        );
    }
}
