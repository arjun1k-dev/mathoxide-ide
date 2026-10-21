//! Interactive window: winit event loop + wgpu surface, pan/zoom/drag,
//! playback controls (space = play/pause, arrows = scrub/step).
//!
//! Façade note: this file is the crate's single owner of winit — per the
//! dependency-façade rule (HANDOFF.md), don't add `use winit::` elsewhere.

use mo_core::camera::Camera;
use mo_core::scene::{Player, Timeline};
use mo_render::Renderer;
use std::collections::HashMap;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

struct App {
    timeline: Timeline,
    window: Option<std::sync::Arc<Window>>,
    renderer: Option<Renderer>,
    surface: Option<wgpu::Surface<'static>>,
    config: Option<wgpu::SurfaceConfiguration>,
    /// Camera overrides (pan/zoom); scene time drives content.
    cam_offset: (f32, f32),
    zoom: f32,
    /// Playhead in seconds; None = paused at this time.
    time: f32,
    playing: bool,
    last_instant: std::time::Instant,
    drag: Option<DragState>,
    cursor: Option<(f32, f32)>,
    overrides: Overrides,
    /// LRU memo of sampled states keyed by (epoch=0, time bits) — the
    /// timeline is fixed here, so scrubbing back and forth becomes lookups.
    memo: mo_core::FrameMemo,
    /// Timeline index — the timeline never changes after startup.
    index: mo_core::TimelineIndex,
    /// Whether the timeline carries camera anims — t-invariant, computed once.
    has_camera_anims: bool,
    /// Hash of the last presented frame's inputs; match = skip eval + GPU pass.
    last_draw_sig: Option<u64>,
}

enum DragState {
    Pan { last: (f32, f32) },
    /// Dragging a mobject: id + grab offset + base center captured at press.
    Mob { id: String, offset: (f32, f32), base_center: (f32, f32) },
}

/// User overrides applied over the sampled timeline state (drag persistence).
#[derive(Default)]
struct Overrides {
    shifts: HashMap<String, (f32, f32)>,
}

impl App {
    fn new(timeline: Timeline) -> Self {
        let index = timeline.index();
        let has_camera_anims = timeline.stmts.iter().any(|s| {
            matches!(
                s,
                mo_core::scene::Stmt::Play(a)
                if matches!(
                    a.anim,
                    mo_core::anim::Animation::CameraOrbit { .. }
                        | mo_core::anim::Animation::CameraZoom { .. }
                        | mo_core::anim::Animation::CameraMoveTo { .. }
                )
            )
        });
        Self {
            timeline,
            window: None,
            renderer: None,
            surface: None,
            config: None,
            cam_offset: (0.0, 0.0),
            zoom: 1.0,
            time: 0.0,
            playing: true,
            last_instant: std::time::Instant::now(),
            drag: None,
            cursor: None,
            overrides: Overrides::default(),
            memo: mo_core::FrameMemo::new(16_384),
            index,
            has_camera_anims,
            last_draw_sig: None,
        }
    }

    /// Base (override-free) scene state, evaluated at most once per time
    /// value; times seen before come from the memo.
    fn base_state(&mut self) -> &HashMap<String, std::sync::Arc<mo_core::vgroup::VGroup>> {
        let (timeline, index, time) = (&self.timeline, &self.index, self.time);
        self.memo
            .get_or_insert_with((0, time.to_bits()), || {
                Player::state_at_indexed(timeline, index, time)
            })
    }

    fn current_state(&mut self) -> HashMap<String, std::sync::Arc<mo_core::vgroup::VGroup>> {
        // Arc makes the base clone a refcount bump; only overridden ids copy.
        let mut state = self.base_state().clone();
        for (id, (dx, dy)) in &self.overrides.shifts {
            if let Some(g) = state.get(id) {
                let mut ng = g.as_ref().clone();
                ng.shift(glam::Vec3::new(*dx, *dy, 0.0));
                state.insert(id.clone(), std::sync::Arc::new(ng));
            }
        }
        state
    }

    fn request_frame(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn camera(&self, width: u32, height: u32) -> Camera {
        let mut cam = Camera::default();
        cam.width = width as f32;
        cam.height = height as f32;
        cam.frame_height /= self.zoom;
        cam.center.x += self.cam_offset.0;
        cam.center.y += self.cam_offset.1;
        cam
    }

    fn draw(&mut self) {
        let Some(window) = self.window.clone() else { return };
        let size = window.inner_size();
        let w = size.width.max(1);
        let h = size.height.max(1);
        let cam = if self.has_camera_anims {
            // Keyframed camera anims override the interactive base camera.
            Player::camera_at(&self.timeline, self.time, &self.camera(w, h))
        } else {
            self.camera(w, h)
        };

        // Skip eval + GPU pass when nothing visual changed (paused, idle).
        let mut hsh = std::collections::hash_map::DefaultHasher::new();
        use std::hash::{Hash, Hasher};
        self.time.to_bits().hash(&mut hsh);
        w.hash(&mut hsh);
        h.hash(&mut hsh);
        cam.center.x.to_bits().hash(&mut hsh);
        cam.center.y.to_bits().hash(&mut hsh);
        cam.frame_height.to_bits().hash(&mut hsh);
        // Keyframed orbit angles change the frame without touching center/zoom.
        cam.phi.to_bits().hash(&mut hsh);
        cam.theta.to_bits().hash(&mut hsh);
        cam.gamma.to_bits().hash(&mut hsh);
        let mut ov = 0u64;
        for (id, (dx, dy)) in &self.overrides.shifts {
            let mut eh = std::collections::hash_map::DefaultHasher::new();
            id.hash(&mut eh);
            dx.to_bits().hash(&mut eh);
            dy.to_bits().hash(&mut eh);
            ov ^= eh.finish();
        }
        ov.hash(&mut hsh);
        let sig = hsh.finish();
        if self.last_draw_sig == Some(sig) {
            return;
        }
        self.last_draw_sig = Some(sig);

        let state = self.current_state();

        let (Some(renderer), Some(surface), Some(config)) =
            (&mut self.renderer, &self.surface, &self.config)
        else {
            return;
        };
        if renderer.width() != w || renderer.height() != h {
            renderer.resize(w, h);
        }
        let mut encoder = renderer.device().create_command_encoder(&Default::default());
        renderer.draw_scene_pass(&state, &cam, &mut encoder);
        // Occluded/stale surfaces can fail to acquire — skip that frame instead of dying.
        let frame = match surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => return,
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        renderer.present_to(&mut encoder, &view);
        renderer.queue().submit(Some(encoder.finish()));
        window.pre_present_notify();
        frame.present();
        let _ = config;
    }

    fn hit_test(&mut self, x: f64, y: f64) -> Option<(String, (f32, f32), (f32, f32))> {
        let size = self.window.as_ref()?.inner_size();
        let cam = self.camera(size.width, size.height);
        let scene = cam.screen_to_scene(x as f32, y as f32);
        let state = self.base_state();
        // topmost = last drawn (ids collected; HashMap has no rev)
        let ids: Vec<&String> = state.keys().collect();
        for id in ids.into_iter().rev() {
            let g = &state[id];
            let (mn, mx) = g.bounds();
            let inside = scene.x >= mn[0]
                && scene.x <= mx[0]
                && scene.y >= mn[1]
                && scene.y <= mx[1];
            if inside {
                let c = g.center();
                return Some((id.clone(), (c.x - scene.x, c.y - scene.y), (c.x, c.y)));
            }
        }
        None
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, loop_: &ActiveEventLoop) {
        let attrs = Window::default_attributes()
            .with_title("MathOxide — interactive preview")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));
        let window = std::sync::Arc::new(loop_.create_window(attrs).expect("create window"));
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance
            .create_surface(window.clone())
            .expect("create surface");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("no adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor::default(),
            None,
        ))
        .expect("device");
        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let surface_format = caps.formats.iter().copied().find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        let mut renderer = Renderer::new(device, queue, size.width.max(1), size.height.max(1)).expect("renderer");
        renderer.set_present_format(surface_format);
        self.window = Some(window.clone());
        self.surface = Some(surface);
        self.config = Some(config);
        self.renderer = Some(renderer);
        self.last_instant = std::time::Instant::now();
        window.request_redraw();
    }

    fn window_event(&mut self, loop_: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => loop_.exit(),
            WindowEvent::Resized(size) => {
                let w = size.width.max(1);
                let h = size.height.max(1);
                if let (Some(config), Some(surface), Some(renderer)) =
                    (&mut self.config, &self.surface, &mut self.renderer)
                {
                    config.width = w;
                    config.height = h;
                    surface.configure(renderer.device(), config);
                    renderer.resize(w, h);
                }
                self.last_draw_sig = None; // new surface: force a present
                self.request_frame();
            }
            WindowEvent::RedrawRequested => {
                let now = std::time::Instant::now();
                let dt = now.duration_since(self.last_instant).as_secs_f32();
                self.last_instant = now;
                if self.playing {
                    self.time += dt;
                    let dur = self.timeline.duration();
                    if self.time > dur {
                        self.time = 0.0; // loop
                    }
                }
                self.draw();
                // Playing keeps the loop alive; paused waits for the next input
                // event (each visual-changing handler calls request_frame).
                if self.playing {
                    self.request_frame();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Space) => {
                        self.playing = !self.playing;
                        self.last_instant = std::time::Instant::now();
                        self.request_frame();
                    }
                    PhysicalKey::Code(KeyCode::ArrowRight) => {
                        self.playing = false;
                        self.time = (self.time + 1.0 / 30.0).min(self.timeline.duration());
                        self.request_frame();
                    }
                    PhysicalKey::Code(KeyCode::ArrowLeft) => {
                        self.playing = false;
                        self.time = (self.time - 1.0 / 30.0).max(0.0);
                        self.request_frame();
                    }
                    PhysicalKey::Code(KeyCode::Home) => {
                        self.playing = false;
                        self.time = 0.0;
                        self.request_frame();
                    }
                    _ => {}
                }
            }
            WindowEvent::MouseInput { state: st, button: MouseButton::Left, .. } => {
                let Some(pos) = self.cursor else { return };
                match st {
                    ElementState::Pressed => {
                        if let Some((id, off, base_center)) = self.hit_test(pos.0 as f64, pos.1 as f64) {
                            self.drag = Some(DragState::Mob { id, offset: off, base_center });
                        } else {
                            self.drag = Some(DragState::Pan { last: pos });
                        }
                    }
                    ElementState::Released => self.drag = None,
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let prev = self.cursor.replace((position.x as f32, position.y as f32));
                if let (Some(drag), Some(cur)) = (&self.drag, self.cursor) {
                    let size = self
                        .window
                        .as_ref()
                        .map(|w| w.inner_size())
                        .unwrap_or(winit::dpi::PhysicalSize::new(1280, 720));
                    let cam = self.camera(size.width, size.height);
                    match drag {
                        DragState::Pan { last } => {
                            let a = cam.screen_to_scene(last.0, last.1);
                            let b = cam.screen_to_scene(cur.0, cur.1);
                            self.cam_offset.0 += b.x - a.x;
                            self.cam_offset.1 += b.y - a.y;
                        }
                        // Base center is captured once at press — no per-move
                        // scene re-evaluation.
                        DragState::Mob { id, offset, base_center } => {
                            let s = cam.screen_to_scene(cur.0, cur.1);
                            let e = self.overrides.shifts.entry(id.clone()).or_insert((0.0, 0.0));
                            e.0 = s.x + offset.0 - base_center.0;
                            e.1 = s.y + offset.1 - base_center.1;
                        }
                    }
                    self.request_frame();
                }
                let _ = prev;
            }
            WindowEvent::MouseWheel { delta: MouseScrollDelta::LineDelta(_, y), .. } => {
                self.zoom = (self.zoom * (1.0 + y * 0.1)).clamp(0.05, 20.0);
                self.request_frame();
            }
            WindowEvent::MouseWheel { delta: MouseScrollDelta::PixelDelta(p), .. } => {
                self.zoom = (self.zoom * (1.0 + p.y as f32 * 0.005)).clamp(0.05, 20.0);
                self.request_frame();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _loop: &ActiveEventLoop) {
        if self.playing {
            self.request_frame();
        }
    }
}

/// Open an interactive window playing `timeline`. Blocks until closed.
pub fn preview(timeline: Timeline) -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    let mut app = App::new(timeline);
    event_loop.run_app(&mut app)?;
    Ok(())
}
