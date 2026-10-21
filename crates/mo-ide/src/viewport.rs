use crate::ext::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use crate::ext::egui_wgpu;
use crate::ext::wgpu;
use mo_core::camera::Camera;
use mo_core::scene::{Player, Timeline};
use mo_render::Renderer;
use std::collections::HashMap;

/// Actions the viewport asks the app to perform.
pub enum ViewportAction {
    /// Double-clicked an object: open the inline text/expr editor for it.
    EditSource(String),
}

pub struct ViewportPanel {
    pub texture_id: egui::TextureId,
    /// Last frame's canvas rect + effective camera — lets the app anchor
    /// overlays (inline edit box) to scene positions.
    pub last_canvas_rect: Rect,
    pub last_cam: Camera,
    last_cursor_pos: Option<Pos2>,
    is_dragging_mob: bool,
    is_panning: bool,
    is_orbiting: bool,
    /// Group-drag grab points: (id, pointer-relative offset) captured at drag
    /// start for every selected object, so the whole selection moves as one.
    group_grab: Vec<(String, (f32, f32))>,
    /// Hash of the last rendered frame's inputs; equal hash = skip eval + GPU pass.
    last_draw_sig: Option<u64>,
    /// Cached `has_camera_anims` scan, invalidated by timeline epoch.
    cam_anims: (u64, bool),
    /// Shifted copies of overridden objects, keyed by (base Arc ptr, offset
    /// bits). The entry keeps its base Arc alive so pointer identity stays
    /// sound; cleared on timeline change and capped against playback-drag
    /// growth.
    shift_cache:
        HashMap<(usize, u32, u32), (std::sync::Arc<mo_core::vgroup::VGroup>, std::sync::Arc<mo_core::vgroup::VGroup>)>,
    shift_cache_epoch: u64,
}

/// Frame signature: everything that changes what the offscreen target shows.
/// Changing any component produces a different hash; a match with the previous
/// frame means the target already holds the right pixels.
fn frame_sig(
    current_time: f32,
    timeline_epoch: u64,
    cam: &Camera,
    target_w: u32,
    target_h: u32,
    overrides: &HashMap<String, (f32, f32)>,
) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    current_time.to_bits().hash(&mut h);
    timeline_epoch.hash(&mut h);
    target_w.hash(&mut h);
    target_h.hash(&mut h);
    for f in [
        cam.center.x,
        cam.center.y,
        cam.center.z,
        cam.frame_height,
        cam.fov,
        cam.phi,
        cam.theta,
        cam.gamma,
    ] {
        f.to_bits().hash(&mut h);
    }
    // Order-independent override hash (HashMap iteration order is unstable).
    let mut ov = 0u64;
    for (id, (dx, dy)) in overrides {
        let mut eh = std::collections::hash_map::DefaultHasher::new();
        id.hash(&mut eh);
        dx.to_bits().hash(&mut eh);
        dy.to_bits().hash(&mut eh);
        ov ^= eh.finish();
    }
    ov.hash(&mut h);
    h.finish()
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
            last_canvas_rect: Rect::NOTHING,
            last_cam: Camera::default(),
            last_cursor_pos: None,
            is_dragging_mob: false,
            is_panning: false,
            is_orbiting: false,
            group_grab: Vec::new(),
            last_draw_sig: None,
            cam_anims: (0, false),
            shift_cache: HashMap::new(),
            shift_cache_epoch: 0,
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
        selection: &mut Vec<String>,
        overrides: &mut HashMap<String, (f32, f32)>,
        base_state: &HashMap<String, std::sync::Arc<mo_core::vgroup::VGroup>>,
        timeline_epoch: u64,
        fps: f32,
        snap: Option<f32>,
    ) -> Option<ViewportAction> {
        let mut action: Option<ViewportAction> = None;
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

        // Animated camera (camera.orbit / camera.zoom / camera.move_to) overrides
        // the interactive base camera while the timeline defines camera motion.
        // The stmts scan is t-invariant — cache it per timeline epoch.
        let has_camera_anims = if self.cam_anims.0 == timeline_epoch {
            self.cam_anims.1
        } else {
            let v = timeline.map(|tl| {
                tl.stmts.iter().any(|s| {
                    matches!(s, mo_core::scene::Stmt::Play(a) if matches!(
                        a.anim,
                        mo_core::anim::Animation::CameraOrbit { .. }
                            | mo_core::anim::Animation::CameraZoom { .. }
                            | mo_core::anim::Animation::CameraMoveTo { .. }
                    ))
                })
            }).unwrap_or(false);
            self.cam_anims = (timeline_epoch, v);
            v
        };
        let cam_owned = if has_camera_anims {
            Player::camera_at(timeline.unwrap(), current_time, camera)
        } else {
            camera.clone()
        };
        let cam: &Camera = &cam_owned;

        // Skip eval + GPU pass entirely when nothing that affects the target changed
        // (paused scrub, idle, static camera). The texture from the last pass persists.
        let sig = frame_sig(current_time, timeline_epoch, cam, renderer.width(), renderer.height(), overrides);
        let skip_render = self.last_draw_sig == Some(sig);
        self.last_draw_sig = Some(sig);

        // Scene state: reuse the app's pre-evaluated base; only materialize a
        // shifted copy when we are actually going to draw. Arc makes the map
        // clone a refcount bump; overridden ids reuse cached shifted copies.
        let mut shifted_state;
        let state: &HashMap<String, std::sync::Arc<mo_core::vgroup::VGroup>> = if skip_render {
            base_state
        } else {
            if self.shift_cache_epoch != timeline_epoch {
                self.shift_cache.clear();
                self.shift_cache_epoch = timeline_epoch;
            }
            shifted_state = base_state.clone();
            for (id, (dx, dy)) in overrides.iter() {
                if let Some(g) = shifted_state.get(id) {
                    let key = (
                        std::sync::Arc::as_ptr(g) as usize,
                        dx.to_bits(),
                        dy.to_bits(),
                    );
                    let shifted = match self.shift_cache.get(&key) {
                        Some((_, s)) => s.clone(),
                        None => {
                            let mut ng = g.as_ref().clone();
                            ng.shift(glam::Vec3::new(*dx, *dy, 0.0));
                            let s = std::sync::Arc::new(ng);
                            if self.shift_cache.len() > 128 {
                                self.shift_cache.clear();
                            }
                            self.shift_cache.insert(key, (g.clone(), s.clone()));
                            s
                        }
                    };
                    shifted_state.insert(id.clone(), shifted);
                }
            }
            &shifted_state
        };

        if !skip_render {
            // Render scene to offscreen target
            let mut encoder = renderer.device().create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("viewport scene encoder"),
            });
            renderer.draw_scene_pass(state, cam, &mut encoder);
            renderer.queue().submit(Some(encoder.finish()));
        }

        // Center canvas inside available panel area
        let offset_x = ((avail_size.x - canvas_w) * 0.5).max(padding);
        let offset_y = ((avail_size.y - canvas_h) * 0.5).max(padding);

        let screen_rect = ui.available_rect_before_wrap();
        let canvas_rect = Rect::from_min_size(
            Pos2::new(screen_rect.left() + offset_x, screen_rect.top() + offset_y),
            Vec2::new(canvas_w, canvas_h),
        );

        // Overlay anchoring data for the app (inline edit box placement).
        self.last_canvas_rect = canvas_rect;
        self.last_cam = cam_owned.clone();

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

        // AABB hit-test against what's on screen: `state` is the unshifted
        // base on skip frames, so compensate for drag overrides.
        let hit_test = |px: f32, py: f32| -> Option<String> {
            for (id, group) in state.iter() {
                let (min, max) = group.bounds();
                let (odx, ody) = overrides.get(id).copied().unwrap_or((0.0, 0.0));
                if px >= min[0] + odx && px <= max[0] + odx && py >= min[1] + ody && py <= max[1] + ody {
                    return Some(id.clone());
                }
            }
            None
        };

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

            // Click handling: Shift+click toggles membership; plain click
            // selects (or clears on empty space). Double-click asks the app
            // to open the inline text/expr editor for the hit object.
            if response.clicked() {
                if let Some(hit) = hit_test(sc.x, sc.y) {
                    if ui.input(|i| i.modifiers.shift) {
                        if let Some(i) = selection.iter().position(|s| s == &hit) {
                            selection.remove(i);
                        } else {
                            selection.push(hit);
                        }
                    } else {
                        *selection = vec![hit];
                    }
                } else if !ui.input(|i| i.modifiers.shift) {
                    selection.clear();
                }
            }
            if response.double_clicked() {
                if let Some(hit) = hit_test(sc.x, sc.y) {
                    action = Some(ViewportAction::EditSource(hit));
                }
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
                } else if is_middle_down || is_shift_down {
                    // Middle-click or Shift-drag = Camera pan
                    self.is_panning = true;
                    self.is_orbiting = false;
                    self.is_dragging_mob = false;
                } else {
                    // Dragging an object moves the whole selection; dragging
                    // an unselected object selects it solo first; empty space pans.
                    let hit = hit_test(sc.x, sc.y);
                    let over_selected = hit.as_ref().map(|h| selection.contains(h)).unwrap_or(false);
                    if let Some(h) = hit {
                        if !over_selected {
                            *selection = vec![h];
                        }
                        self.group_grab.clear();
                        for id in selection.iter() {
                            if let Some(group) = state.get(id) {
                                let mut c = group.center();
                                let (odx, ody) = overrides.get(id).copied().unwrap_or((0.0, 0.0));
                                c.x += odx;
                                c.y += ody;
                                self.group_grab.push((id.clone(), (c.x - sc.x, c.y - sc.y)));
                            }
                        }
                        if !self.group_grab.is_empty() {
                            self.is_dragging_mob = true;
                            self.is_panning = false;
                            self.is_orbiting = false;
                        } else {
                            self.is_panning = true;
                            self.is_orbiting = false;
                            self.is_dragging_mob = false;
                        }
                    } else {
                        self.is_panning = true;
                        self.is_orbiting = false;
                        self.is_dragging_mob = false;
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
                    // base_state is exactly `Player::state_at(tl, current_time)`
                    // pre-evaluated by the app — no second evaluation here.
                    for (id, rel) in &self.group_grab {
                        let base_center = base_state
                            .get(id)
                            .map(|g| g.center())
                            .unwrap_or(glam::Vec3::ZERO);
                        let mut shift_x = sc.x + rel.0 - base_center.x;
                        let mut shift_y = sc.y + rel.1 - base_center.y;
                        if let Some(q) = snap.filter(|q| *q > 0.0) {
                            shift_x = (shift_x / q).round() * q;
                            shift_y = (shift_y / q).round() * q;
                        }
                        overrides.insert(id.clone(), (shift_x, shift_y));
                    }
                }
            }

            self.last_cursor_pos = Some(pos);
        } else {
            self.last_cursor_pos = None;
        }

        // Draw Selection Bounding Boxes on top of the viewport. The primary
        // (last-selected) object gets the cyan box + handles + id tag; the
        // rest of the selection gets a dim violet stroke.
        let primary = selection.last().cloned();
        for id in selection.iter() {
            let Some(group) = state.get(id) else { continue };
            let (mut min, mut max) = group.bounds();
            let (odx, ody) = overrides.get(id).copied().unwrap_or((0.0, 0.0));
            min[0] += odx;
            min[1] += ody;
            max[0] += odx;
            max[1] += ody;
            let p_min = cam.scene_to_screen(min[0], min[1]);
            let p_max = cam.scene_to_screen(max[0], max[1]);

            let screen_min = Pos2::new(canvas_rect.left() + p_min.x, canvas_rect.top() + p_min.y);
            let screen_max = Pos2::new(canvas_rect.left() + p_max.x, canvas_rect.top() + p_max.y);

            let box_rect = Rect::from_two_pos(screen_min, screen_max);
            let is_primary = primary.as_deref() == Some(id.as_str());
            let accent = if is_primary {
                Color32::from_rgb(88, 196, 221)
            } else {
                Color32::from_rgb(199, 36, 255)
            };

            painter.rect_stroke(
                box_rect.expand(4.0),
                3.0,
                Stroke::new(if is_primary { 1.5_f32 } else { 1.0_f32 }, accent),
                egui::StrokeKind::Inside,
            );

            if is_primary {
                // Small corner handle markers
                let corners = [
                    box_rect.left_top(),
                    box_rect.right_top(),
                    box_rect.left_bottom(),
                    box_rect.right_bottom(),
                ];
                for c in corners {
                    painter.circle_filled(c, 3.5, accent);
                }

                // ID Tag
                painter.text(
                    Pos2::new(box_rect.left() + 2.0, box_rect.top() - 14.0),
                    egui::Align2::LEFT_BOTTOM,
                    id,
                    egui::FontId::monospace(12.0),
                    accent,
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

        // Top-right: FPS & 3D Orbit Status (+ snap badge when active)
        let is_3d = cam.phi.abs() > 0.01 || cam.theta.abs() > 0.01;
        let snap_badge = snap
            .filter(|q| *q > 0.0)
            .map(|q| format!(" | SNAP {:.2}", q))
            .unwrap_or_default();
        let camera_status = if is_3d {
            format!(
                "🎥 3D Orbit: φ {}° θ {}° | {:.0} FPS{}",
                cam.phi.to_degrees().round() as i32,
                cam.theta.to_degrees().round() as i32,
                fps,
                snap_badge
            )
        } else {
            format!("🎥 2D View | Zoom {:.2}x | {:.0} FPS{}", 8.0 / cam.frame_height, fps, snap_badge)
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
            "Pan: Shift+Drag | 3D Orbit: Right-Drag / Alt+Drag | Reset: R | Snap: G | Esc: Deselect",
            egui::FontId::monospace(10.0),
            Color32::from_rgba_unmultiplied(150, 160, 180, 140),
        );

        action
    }
}
