//! Interactive window: winit event loop + wgpu surface, pan/zoom/drag,
//! playback controls (space = play/pause, arrows = scrub/step).

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
}

enum DragState {
    Pan { last: (f32, f32) },
    /// Dragging a mobject: id + grab offset in scene coords.
    Mob { id: String, offset: (f32, f32) },
}

/// User overrides applied over the sampled timeline state (drag persistence).
#[derive(Default)]
struct Overrides {
    shifts: HashMap<String, (f32, f32)>,
}

impl App {
    fn new(timeline: Timeline) -> Self {
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
        }
    }

    fn current_state(&self) -> HashMap<String, mo_core::vgroup::VGroup> {
        let mut state = Player::state_at(&self.timeline, self.time);
        for (id, (dx, dy)) in &self.overrides.shifts {
            if let Some(g) = state.get_mut(id) {
                g.shift(glam::Vec3::new(*dx, *dy, 0.0));
            }
        }
        state
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

    fn draw(&self) {
        let (Some(window), Some(renderer), Some(surface), Some(config)) =
            (&self.window, &self.renderer, &self.surface, &self.config)
        else {
            return;
        };
        let size = window.inner_size();
        let cam = self.camera(size.width, size.height);
        let state = self.current_state();
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

    fn hit_test(&self, x: f64, y: f64) -> Option<(String, (f32, f32))> {
        let size = self.window.as_ref()?.inner_size();
        let cam = self.camera(size.width, size.height);
        let scene = cam.screen_to_scene(x as f32, y as f32);
        let state = Player::state_at(&self.timeline, self.time);
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
                return Some((id.clone(), (c.x - scene.x, c.y - scene.y)));
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
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: caps.formats[0],
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        let mut renderer = Renderer::new(device, queue, 1280, 720).expect("renderer");
        renderer.set_present_format(caps.formats[0]);
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
                if let (Some(config), Some(surface), Some(renderer)) =
                    (&mut self.config, &self.surface, &self.renderer)
                {
                    config.width = size.width.max(1);
                    config.height = size.height.max(1);
                    surface.configure(renderer.device(), config);
                }
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
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Space) => self.playing = !self.playing,
                    PhysicalKey::Code(KeyCode::ArrowRight) => {
                        self.playing = false;
                        self.time = (self.time + 1.0 / 30.0).min(self.timeline.duration());
                    }
                    PhysicalKey::Code(KeyCode::ArrowLeft) => {
                        self.playing = false;
                        self.time = (self.time - 1.0 / 30.0).max(0.0);
                    }
                    PhysicalKey::Code(KeyCode::Home) => {
                        self.playing = false;
                        self.time = 0.0;
                    }
                    _ => {}
                }
            }
            WindowEvent::MouseInput { state: st, button: MouseButton::Left, .. } => {
                let Some(pos) = self.cursor else { return };
                match st {
                    ElementState::Pressed => {
                        if let Some((id, off)) = self.hit_test(pos.0 as f64, pos.1 as f64) {
                            self.drag = Some(DragState::Mob { id, offset: off });
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
                        DragState::Mob { id, offset } => {
                            let s = cam.screen_to_scene(cur.0, cur.1);
                            let bx = base_center_x(&self.timeline, &id, self.time);
                            let by = base_center_y(&self.timeline, &id, self.time);
                            let e = self.overrides.shifts.entry(id.clone()).or_insert((0.0, 0.0));
                            e.0 = s.x + offset.0 - bx;
                            e.1 = s.y + offset.1 - by;
                        }
                    }
                }
                let _ = prev;
            }
            WindowEvent::MouseWheel { delta: MouseScrollDelta::LineDelta(_, y), .. } => {
                self.zoom = (self.zoom * (1.0 + y * 0.1)).clamp(0.05, 20.0);
            }
            WindowEvent::MouseWheel { delta: MouseScrollDelta::PixelDelta(p), .. } => {
                self.zoom = (self.zoom * (1.0 + p.y as f32 * 0.005)).clamp(0.05, 20.0);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _loop: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

fn base_center_x(tl: &Timeline, id: &str, t: f32) -> f32 {
    Player::state_at(tl, t)
        .get(id)
        .map(|g| g.center().x)
        .unwrap_or(0.0)
}

fn base_center_y(tl: &Timeline, id: &str, t: f32) -> f32 {
    Player::state_at(tl, t)
        .get(id)
        .map(|g| g.center().y)
        .unwrap_or(0.0)
}

/// Open an interactive window playing `timeline`. Blocks until closed.
pub fn preview(timeline: Timeline) -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    let mut app = App::new(timeline);
    event_loop.run_app(&mut app)?;
    Ok(())
}
