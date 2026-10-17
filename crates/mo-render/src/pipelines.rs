//! wgpu pipelines: fill (stencil winding + cover), stroke (ribbon), present.

use anyhow::Result;
use glam::Vec3;
use mo_core::vgroup::VGroup;
use mo_core::camera::FrameUniforms;
use wgpu::util::DeviceExt;

/// Fill vertex: fan/curve triangle corners, expanded on CPU.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct FillVertex {
    pos: [f32; 3],
    color: [f32; 4],
    uv: [f32; 2],
    fill_all: f32,
}

/// Stroke vertex: ribbon strip around a curve.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct StrokeVertex {
    pos: [f32; 3],
    color: [f32; 4],
}

const FILL_WGSL: &str = include_str!("fill_embedded.wgsl");
const STROKE_WGSL: &str = include_str!("stroke_embedded.wgsl");
const PRESENT_WGSL: &str = r#"
struct VsOut { @builtin(position) pos: vec4f, @location(0) uv: vec2f };
@vertex fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    var p = array<vec2f, 6>(vec2f(-1.0,-1.0), vec2f(1.0,-1.0), vec2f(-1.0,1.0),
                            vec2f(-1.0,1.0), vec2f(1.0,-1.0), vec2f(1.0,1.0));
    var o: VsOut;
    o.pos = vec4f(p[i], 0.0, 1.0);
    o.uv = vec2f((p[i].x + 1.0) * 0.5, 1.0 - (p[i].y + 1.0) * 0.5);
    return o;
}
@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@fragment fn fs_main(in: VsOut) -> @location(0) vec4f {
    return textureSample(tex, samp, in.uv);
}
"#;

pub struct Renderer {
    device: wgpu::Device,
    frame_bgl: wgpu::BindGroupLayout,
    queue: wgpu::Queue,
    width: u32,
    height: u32,
    frame_buf: wgpu::Buffer,
    target: wgpu::TextureView,
    target_tex: wgpu::Texture,
    sampler: wgpu::Sampler,
    fill_pipeline: wgpu::RenderPipeline,
    stroke_pipeline: wgpu::RenderPipeline,
    present_pipeline: wgpu::RenderPipeline,
}

impl Renderer {
    /// Headless device (used by CLI rendering and tests).
    pub fn offscreen(width: u32, height: u32) -> Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .ok_or_else(|| anyhow::anyhow!("no GPU adapter"))?;
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("mo-render"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))?;
        Self::new(device, queue, width, height)
    }

    pub fn new(device: wgpu::Device, queue: wgpu::Queue, width: u32, height: u32) -> Result<Self> {
        let target_tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("scene target"),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
        let target = target_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
        let frame_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame uniforms"),
            size: std::mem::size_of::<FrameUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let frame_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let fill_pipeline = Self::fill_pipeline(&device, &frame_bgl, wgpu::TextureFormat::Rgba8Unorm);
        let stroke_pipeline = Self::stroke_pipeline(&device, &frame_bgl, wgpu::TextureFormat::Rgba8Unorm);
        let present_pipeline = Self::present_pipeline(&device, wgpu::TextureFormat::Rgba8Unorm);

        Ok(Self {
            device, queue, width, height, frame_buf, target, target_tex, sampler, frame_bgl,
            fill_pipeline, stroke_pipeline, present_pipeline,
        })
    }

    fn fill_pipeline(device: &wgpu::Device, bgl: &wgpu::BindGroupLayout, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fill"),
            source: wgpu::ShaderSource::Wgsl(FILL_WGSL.into()),
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fill"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[bgl],
                push_constant_ranges: &[],
            })),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<FillVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 },
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 12, shader_location: 1 },
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 28, shader_location: 2 },
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32, offset: 36, shader_location: 3 },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::all(),
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            // Winding-count stencil pass, then cover pass shares this pipeline
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Stencil8,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                bias: Default::default(),
                stencil: wgpu::StencilState {
                    front: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Always,
                        fail_op: wgpu::StencilOperation::IncrementWrap,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::IncrementWrap,
                    },
                    back: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Always,
                        fail_op: wgpu::StencilOperation::DecrementWrap,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::DecrementWrap,
                    },
                    read_mask: 0xff,
                    write_mask: 0xff,
                },
                
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    fn stroke_pipeline(device: &wgpu::Device, bgl: &wgpu::BindGroupLayout, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("stroke"),
            source: wgpu::ShaderSource::Wgsl(STROKE_WGSL.into()),
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("stroke"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[bgl],
                push_constant_ranges: &[],
            })),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<StrokeVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 },
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 12, shader_location: 1 },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::all(),
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Stencil8,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                bias: Default::default(),
                stencil: wgpu::StencilState {
                    front: wgpu::StencilFaceState::IGNORE,
                    back: wgpu::StencilFaceState::IGNORE,
                    read_mask: 0,
                    write_mask: 0,
                },
                
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    fn present_pipeline(device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("present"),
            source: wgpu::ShaderSource::Wgsl(PRESENT_WGSL.into()),
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("present"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("present"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                })],
                push_constant_ranges: &[],
            })),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                targets: &[Some(format.into())],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    /// Render a HashMap state to the offscreen target, return RGBA pixels.
    pub fn render(&self, state: &std::collections::HashMap<String, VGroup>) -> image::RgbaImage {
        let mut cam = mo_core::camera::Camera::default();
        cam.width = self.width as f32;
        cam.height = self.height as f32;
        let frame: FrameUniforms = cam.uniforms();
        self.queue.write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&frame));
        let frame_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.frame_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: self.frame_buf.as_entire_binding() }],
        });
        let frame_bg_s = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.frame_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: self.frame_buf.as_entire_binding() }],
        });

        let stencil_view = self
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("stencil"),
                size: wgpu::Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Stencil8,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &stencil_view,
                    depth_ops: None,
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Store,
                    }),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            for group in state.values() {
                self.draw_fill(&mut pass, group, &frame_bg);
            }
            for group in state.values() {
                self.draw_stroke(&mut pass, group, &frame_bg_s);
            }
        }
        self.queue.submit(Some(encoder.finish()));

        let bytes = self.read_target();
        image::RgbaImage::from_raw(self.width, self.height, bytes).expect("size mismatch")
    }

    fn draw_fill<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        group: &'a VGroup,
        frame_bg: &'a wgpu::BindGroup,
    ) {
        let verts = tessellate_fill(group);
        if verts.is_empty() {
            return;
        }
        let buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        // Pass 1: winding into stencil (color masked off)
        pass.set_pipeline(&self.fill_pipeline);
        pass.set_bind_group(0, frame_bg, &[]);
        pass.set_stencil_reference(0);
        pass.set_vertex_buffer(0, buf.slice(..));
        // winding write: stencil op, but color writes only in cover pass.
        // We do both here using fill_all: fragments with fill_all==1 (interior fan)
        // get colored in the same pass, guarded by stencil != 0 test — simplest
        // correct-enough v0.1: two draws with different stencil compare.
        // Draw A: write winding only (no color)
        pass.set_vertex_buffer(0, buf.slice(..));
        pass.draw(0..verts.len() as u32, 0..1);
        // Note: full two-pass winding+cover requires per-draw stencil state changes;
        // v0.1 uses single-pass nonzero fill via the fan + curve discard. See
        // fill_embedded.wgsl for the uv-space curve cut.
    }

    fn draw_stroke<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        group: &'a VGroup,
        frame_bg: &'a wgpu::BindGroup,
    ) {
        let verts = tessellate_stroke(group);
        if verts.is_empty() {
            return;
        }
        let buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        pass.set_pipeline(&self.stroke_pipeline);
        pass.set_bind_group(0, frame_bg, &[]);
        pass.set_vertex_buffer(0, buf.slice(..));
        pass.draw(0..verts.len() as u32, 0..1);
    }

    pub fn read_target(&self) -> Vec<u8> {
        let bytes_per_row = self.width * 4;
        let padded = (bytes_per_row + 255) / 256 * 256;
        let out = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded * self.height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            self.target_tex.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &out,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
        );
        self.queue.submit(Some(encoder.finish()));
        let slice = out.slice(..);
        let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = sender.send(r);
        });
        self.device.poll(wgpu::Maintain::Wait);
        pollster::block_on(async { receiver.receive().await.expect("map channel") }).expect("map failed");
        let data = slice.get_mapped_range();
        let mut result = Vec::with_capacity((bytes_per_row * self.height) as usize);
        for row in 0..self.height {
            let start = (row * padded) as usize;
            result.extend_from_slice(&data[start..start + bytes_per_row as usize]);
        }
        drop(data);
        out.unmap();
        result
    }

    /// Present the offscreen target onto an external surface view (IDE window).
    pub fn present_to(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.present_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.target) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("present"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.present_pipeline);
        pass.set_bind_group(0, &bg, &[]);
        pass.draw(0..6, 0..1);
    }

    pub fn texture_view(&self) -> &wgpu::TextureView {
        &self.target
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }
}

// -- CPU tessellation --------------------------------------------------------

const SIMPLE_QUAD: [[f32; 2]; 3] = [[0.0, 0.0], [0.5, 0.0], [1.0, 1.0]];

/// Per bezier: fan triangle (base_point, p0, p2) + curve triangle (p0, p1, p2),
/// the latter cut to the curve in uv space by the fragment shader.
fn tessellate_fill(g: &VGroup) -> Vec<FillVertex> {
    let mut out = Vec::new();
    let curves: Vec<_> = g.curves().collect();
    for (i, [p0, p1, p2]) in curves.iter().enumerate() {
        // skip degenerate (no-curve) entries
        if *p0 == *p1 {
            continue;
        }
        let anchor_record = i * 2;
        let base = subpath_base(g, anchor_record);
        let color = pick_fill(g, anchor_record);
        // fan triangle
        for p in [base, *p0, *p2] {
            out.push(FillVertex { pos: p.to_array(), color, uv: [0.0, 0.0], fill_all: 1.0 });
        }
        // curve triangle with y = x^2 uv cut
        for (c, uv) in [*p0, *p1, *p2].iter().zip(SIMPLE_QUAD) {
            out.push(FillVertex { pos: c.to_array(), color, uv, fill_all: 0.0 });
        }
    }
    out
}

fn pick_fill(g: &VGroup, record: usize) -> [f32; 4] {
    let idx = record.min(g.points.len().saturating_sub(1));
    g.points[idx].fill_rgba
}

fn subpath_base(g: &VGroup, anchor_record: usize) -> Vec3 {
    for sp in &g.subpaths {
        if anchor_record >= sp.start && anchor_record < sp.end {
            return Vec3::from(g.points[sp.start].point);
        }
    }
    Vec3::ZERO
}

/// Ribbon strip: for each curve, sample points, offset perpendicular by
/// stroke_width/2 in screen-ish units (approximation: scene units).
fn tessellate_stroke(g: &VGroup) -> Vec<StrokeVertex> {
    let mut out = Vec::new();
    for sp in &g.subpaths {
        if sp.end < 3 {
            continue;
        }
        let stroke_rgba = g.points[sp.start].stroke_rgba;
        let width = g.points[sp.start].stroke_width / 800.0 * 8.0; // scene units
        let half = width / 2.0;
        let pts = flatten_subpath(g, sp, 24);
        if pts.len() < 2 {
            continue;
        }
        for w in pts.windows(2) {
            let dir = (w[1] - w[0]).normalize_or_zero();
            let n = Vec3::new(-dir.y, dir.x, 0.0) * half;
            let (a, b) = (w[0], w[1]);
            let quad = [a - n, a + n, b - n, a + n, b + n, b - n];
            for p in quad {
                out.push(StrokeVertex { pos: p.to_array(), color: stroke_rgba });
            }
        }
    }
    out
}

fn flatten_subpath(g: &VGroup, sp: &mo_core::vgroup::SubPath, per_curve: usize) -> Vec<Vec3> {
    let mut out = Vec::new();
    for r in (sp.start..sp.end.saturating_sub(2)).step_by(2) {
        let p0 = Vec3::from(g.points[r].point);
        let p1 = Vec3::from(g.points[r + 1].point);
        let p2 = Vec3::from(g.points[r + 2].point);
        if p0 == p1 {
            continue;
        }
        for s in 0..per_curve {
            let t = s as f32 / per_curve as f32;
            let mt = 1.0 - t;
            out.push(mt * mt * p0 + 2.0 * mt * t * p1 + t * t * p2);
        }
    }
    let last = g.points[sp.end - 1].point;
    out.push(Vec3::from(last));
    out
}
