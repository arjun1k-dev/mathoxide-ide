//! wgpu pipelines: ManimGL-fidelity rendering using WGSL shaders.
//! - Fill: two-pass winding-number stencil counting (WINDING_COUNT) + border AA (FILL_BORDER) + cover (WINDING_COVER).
//! - Stroke: procedural GPU strokes with rounded joins, miter shift, and smoothstep analytic antialiasing.
//! - Present: full-screen triangle with native resolution scaling and sRGB gamma handling.

use anyhow::Result;
use mo_core::camera::FrameUniforms;
use mo_core::vgroup::VGroup;

/// GPU-boundary aliases — the only wgpu types that cross this module's public
/// API. On a wgpu version bump, adjust these (and the internals below) here;
/// callers use the aliases, never raw `wgpu::` paths.
pub type GpuDevice = wgpu::Device;
pub type GpuQueue = wgpu::Queue;
pub type GpuTextureFormat = wgpu::TextureFormat;
pub type GpuCommandEncoder = wgpu::CommandEncoder;
pub type GpuTextureView = wgpu::TextureView;

const FILL_WGSL: &str = include_str!(concat!(env!("OUT_DIR"), "/fill.wgsl"));
const STROKE_WGSL: &str = include_str!(concat!(env!("OUT_DIR"), "/stroke.wgsl"));
const BORDER_WGSL: &str = include_str!(concat!(env!("OUT_DIR"), "/border.wgsl"));
const PRESENT_WGSL: &str = include_str!(concat!(env!("OUT_DIR"), "/present.wgsl"));
const PRESENT_SRGB_WGSL: &str = include_str!(concat!(env!("OUT_DIR"), "/present_srgb.wgsl"));

/// std140 layout matching `shaders/inserts/mobject_uniforms.wgsl`. Size = 192 bytes.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MobjectUniforms {
    pub is_fixed_in_frame: f32,
    pub _pad0: [f32; 3],
    pub shading: [f32; 3],
    pub _pad1: f32,
    pub clip_plane0: [f32; 4],
    pub clip_plane1: [f32; 4],
    pub clip_plane2: [f32; 4],
    pub clip_plane3: [f32; 4],
    pub anti_alias_width: f32,
    pub joint_roundness: f32,
    pub flat_stroke: f32,
    pub stroke_width_in_scene_units: f32,
    pub unit_normal: [f32; 3],
    pub _pad2: f32,
    pub fill_rgba: [f32; 4],
    pub fill_rgba_end: [f32; 4],
    pub gradient_start: [f32; 3],
    pub _pad3: f32,
    pub gradient_end: [f32; 3],
    pub fill_border_width: f32,
}

impl Default for MobjectUniforms {
    fn default() -> Self {
        Self {
            is_fixed_in_frame: 0.0,
            _pad0: [0.0; 3],
            shading: [0.0; 3],
            _pad1: 0.0,
            clip_plane0: [0.0; 4],
            clip_plane1: [0.0; 4],
            clip_plane2: [0.0; 4],
            clip_plane3: [0.0; 4],
            anti_alias_width: 1.5,
            joint_roundness: 0.0,
            flat_stroke: 0.0,
            stroke_width_in_scene_units: 0.0,
            unit_normal: [0.0, 0.0, 1.0],
            _pad2: 0.0,
            fill_rgba: [0.0; 4],
            fill_rgba_end: [0.0; 4],
            gradient_start: [0.0; 3],
            _pad3: 0.0,
            gradient_end: [0.0; 3],
            fill_border_width: 0.0,
        }
    }
}

/// 40-byte storage record matching `DATA_STRIDE: 10u` in `shaders/inserts/read_data.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuPointRecord {
    pub point: [f32; 3],
    pub stroke_rgba: [f32; 4],
    pub stroke_width: f32,
    pub subpath_range: [f32; 2],
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    width: u32,
    height: u32,

    target_tex: wgpu::Texture,
    target: wgpu::TextureView,

    depth_stencil_tex: wgpu::Texture,
    depth_stencil_view: wgpu::TextureView,

    sampler: wgpu::Sampler,

    mob_bgl: wgpu::BindGroupLayout,
    data_bgl: wgpu::BindGroupLayout,

    frame_buf: wgpu::Buffer,
    frame_bg: wgpu::BindGroup,

    fill_count_pipeline: wgpu::RenderPipeline,
    fill_border_pipeline: wgpu::RenderPipeline,
    fill_cover_pipeline: wgpu::RenderPipeline,
    stroke_pipeline: wgpu::RenderPipeline,
    present_pipeline: wgpu::RenderPipeline,
    present_format: wgpu::TextureFormat,

    /// Per-(object, content) GPU buffer + bind group reuse. Without this every
    /// frame allocated fresh storage/uniform buffers per object — allocation
    /// churn that overwhelms iGPUs with large static geometry (e.g. dense
    /// surface3d grids). Content-hashed so animated objects re-upload.
    buf_cache: std::cell::RefCell<BufCache>,
    /// Persistent upload staging: per-frame misses are packed here and copied
    /// into pooled buffers, so new content costs one staging allocation ever
    /// instead of a driver alloc per upload.
    staging: wgpu::Buffer,
    /// Next free offset in `staging`; reset at the top of each frame.
    staging_off: std::cell::Cell<u64>,
    /// Cached present bind group (target view + sampler never change until resize).
    present_bg: std::cell::RefCell<Option<wgpu::BindGroup>>,
    /// Persistent GPU→CPU readback buffer, recreated only on resize.
    readback: std::cell::RefCell<Option<(wgpu::Buffer, u32, u32)>>,
}

struct BufCacheEntry {
    // BindGroups keep their backing buffers alive; the explicit handles let
    // eviction recycle the buffers into the pool instead of freeing them.
    data_bg: wgpu::BindGroup,
    mob_bg: wgpu::BindGroup,
    data_buf: wgpu::Buffer,
    mob_buf: wgpu::Buffer,
    last_seen: u64,
    bytes: u64,
    num_curves: u32,
    has_stroke: bool,
}

#[derive(Default)]
struct BufCache {
    map: std::collections::HashMap<u64, BufCacheEntry>,
    total_bytes: u64,
    frame: u64,
    /// Evicted data buffers keyed by capacity — reused for new content so
    /// device allocations stay flat.
    free_data: std::collections::BTreeMap<u64, Vec<wgpu::Buffer>>,
    free_uniform: Vec<wgpu::Buffer>,
    free_bytes: u64,
}

impl BufCache {
    /// Return an entry's buffers to the reuse pools (bounded); anything over
    /// the pool budget is dropped so evictions still release device memory.
    fn retire(&mut self, e: BufCacheEntry) {
        let BufCacheEntry { data_bg: _, mob_bg: _, data_buf, mob_buf, .. } = e;
        let class = data_buf.size();
        if self.free_bytes + class <= BUF_POOL_MAX_BYTES {
            self.free_bytes += class;
            self.free_data.entry(class).or_default().push(data_buf);
        }
        if self.free_uniform.len() < 64 {
            self.free_uniform.push(mob_buf);
        }
    }
}

const STAGING_BYTES: u64 = 16 * 1024 * 1024;
/// Pool cap for recycled (currently unused) data buffers.
const BUF_POOL_MAX_BYTES: u64 = 4 * 1024 * 1024;
const UNIFORM_CLASS: u64 = 256;

/// Cache key covering everything the GPU upload depends on: the raw point
/// records (positions/stroke color/width) plus subpath structure and the
/// mobject uniforms. One sequential pass over `points` — no recordVec build.
fn group_content_hash(id: &str, g: &VGroup, u: &MobjectUniforms) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut h);
    g.points.len().hash(&mut h);
    h.write(bytemuck::cast_slice(&g.points));
    for sp in &g.subpaths {
        sp.start.hash(&mut h);
        sp.end.hash(&mut h);
    }
    h.write(bytemuck::bytes_of(u));
    h.finish()
}

const BUF_CACHE_MAX_ENTRIES: usize = 512;
const BUF_CACHE_MAX_BYTES: u64 = 64 * 1024 * 1024;
const BUF_CACHE_STALE_FRAMES: u64 = 300;

impl Renderer {
    /// Stage `contents` at the next 256-aligned ring offset and enqueue a
    /// copy into `buf`. Returns false when the ring is exhausted this frame;
    /// the caller then falls back to a direct queue write.
    fn stage_copy(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        buf: &wgpu::Buffer,
        contents: &[u8],
    ) -> bool {
        let align = wgpu::COPY_BUFFER_ALIGNMENT;
        let off = self.staging_off.get();
        let padded = (contents.len() as u64).max(1).div_ceil(align) * align;
        if off + padded > STAGING_BYTES {
            return false;
        }
        self.queue.write_buffer(&self.staging, off, contents);
        encoder.copy_buffer_to_buffer(&self.staging, off, buf, 0, padded);
        self.staging_off.set(off + padded);
        true
    }

    /// Take a pooled storage buffer of at least `bytes`, recycling evicted
    /// buffers (smallest sufficient class first) before allocating new ones.
    fn take_data_buffer(&self, cache: &mut BufCache, bytes: u64) -> wgpu::Buffer {
        let align = wgpu::COPY_BUFFER_ALIGNMENT;
        let class = bytes.max(1).div_ceil(align) * align;
        if let Some((&cls, _)) = cache.free_data.range(class..).next() {
            let v = cache.free_data.get_mut(&cls).unwrap();
            let b = v.pop().unwrap();
            if v.is_empty() {
                cache.free_data.remove(&cls);
            }
            cache.free_bytes -= cls;
            return b;
        }
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mob_data"),
            size: class,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn take_uniform_buffer(&self, cache: &mut BufCache) -> wgpu::Buffer {
        if let Some(b) = cache.free_uniform.pop() {
            return b;
        }
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mob_uniforms"),
            size: UNIFORM_CLASS,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
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

    pub fn new(device: GpuDevice, queue: GpuQueue, width: u32, height: u32) -> Result<Self> {
        let (target_tex, target) = Self::create_target(&device, width, height);
        let (depth_stencil_tex, depth_stencil_view) = Self::create_depth_stencil(&device, width, height);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let frame_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame uniforms"),
            size: std::mem::size_of::<FrameUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let frame_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame_bgl"),
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

        let mob_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mob_bgl"),
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

        let data_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("data_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let frame_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame_bg"),
            layout: &frame_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buf.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene_pipeline_layout"),
            bind_group_layouts: &[&frame_bgl, &mob_bgl, &data_bgl],
            push_constant_ranges: &[],
        });

        let fill_count_pipeline = Self::build_fill_count_pipeline(&device, &pipeline_layout);
        let fill_border_pipeline = Self::build_fill_border_pipeline(&device, &pipeline_layout);
        let fill_cover_pipeline = Self::build_fill_cover_pipeline(&device, &pipeline_layout);
        let stroke_pipeline = Self::build_stroke_pipeline(&device, &pipeline_layout);

        let present_format = wgpu::TextureFormat::Rgba8Unorm;
        let present_pipeline = Self::build_present_pipeline(&device, present_format);

        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("upload staging"),
            size: STAGING_BYTES,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        Ok(Self {
            device,
            queue,
            width,
            height,
            target_tex,
            target,
            depth_stencil_tex,
            depth_stencil_view,
            sampler,
            mob_bgl,
            data_bgl,
            frame_buf,
            frame_bg,
            fill_count_pipeline,
            fill_border_pipeline,
            fill_cover_pipeline,
            stroke_pipeline,
            present_pipeline,
            present_format,
            buf_cache: std::cell::RefCell::new(Default::default()),
            staging,
            staging_off: std::cell::Cell::new(0),
            present_bg: std::cell::RefCell::new(None),
            readback: std::cell::RefCell::new(None),
        })
    }

    fn create_target(device: &wgpu::Device, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene target"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        (tex, view)
    }

    fn create_depth_stencil(device: &wgpu::Device, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene depth stencil"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24PlusStencil8,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        (tex, view)
    }

    /// Resize offscreen target and depth stencil buffers to match native window dimensions.
    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.width == width && self.height == height {
            return;
        }
        self.width = width;
        self.height = height;
        let (target_tex, target) = Self::create_target(&self.device, width, height);
        let (depth_stencil_tex, depth_stencil_view) = Self::create_depth_stencil(&self.device, width, height);
        self.target_tex = target_tex;
        self.target = target;
        self.depth_stencil_tex = depth_stencil_tex;
        self.depth_stencil_view = depth_stencil_view;
        *self.present_bg.borrow_mut() = None;
    }

    fn build_fill_count_pipeline(device: &wgpu::Device, layout: &wgpu::PipelineLayout) -> wgpu::RenderPipeline {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fill_count_shader"),
            source: wgpu::ShaderSource::Wgsl(FILL_WGSL.into()),
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fill_count"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::empty(),
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                bias: Default::default(),
                stencil: wgpu::StencilState {
                    front: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Always,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::IncrementWrap,
                        pass_op: wgpu::StencilOperation::IncrementWrap,
                    },
                    back: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Always,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::DecrementWrap,
                        pass_op: wgpu::StencilOperation::DecrementWrap,
                    },
                    read_mask: 0xff,
                    write_mask: 0xff,
                },
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        })
    }

    fn build_fill_border_pipeline(device: &wgpu::Device, layout: &wgpu::PipelineLayout) -> wgpu::RenderPipeline {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fill_border_shader"),
            source: wgpu::ShaderSource::Wgsl(BORDER_WGSL.into()),
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fill_border"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::all(),
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                bias: Default::default(),
                stencil: wgpu::StencilState {
                    front: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Equal,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::Keep,
                    },
                    back: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Equal,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::Keep,
                    },
                    read_mask: 0xff,
                    write_mask: 0,
                },
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        })
    }

    fn build_fill_cover_pipeline(device: &wgpu::Device, layout: &wgpu::PipelineLayout) -> wgpu::RenderPipeline {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fill_cover_shader"),
            source: wgpu::ShaderSource::Wgsl(FILL_WGSL.into()),
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fill_cover"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::all(),
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                bias: Default::default(),
                stencil: wgpu::StencilState {
                    front: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::NotEqual,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Zero,
                        pass_op: wgpu::StencilOperation::Zero,
                    },
                    back: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::NotEqual,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Zero,
                        pass_op: wgpu::StencilOperation::Zero,
                    },
                    read_mask: 0xff,
                    write_mask: 0xff,
                },
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        })
    }

    fn build_stroke_pipeline(device: &wgpu::Device, layout: &wgpu::PipelineLayout) -> wgpu::RenderPipeline {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("stroke_shader"),
            source: wgpu::ShaderSource::Wgsl(STROKE_WGSL.into()),
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("stroke"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::all(),
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24PlusStencil8,
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
            multisample: Default::default(),
            multiview: None,
            cache: None,
        })
    }

    fn build_present_pipeline(device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        let src = if format.is_srgb() {
            PRESENT_SRGB_WGSL
        } else {
            PRESENT_WGSL
        };
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("present_shader"),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("present_bgl"),
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
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("present_layout"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("present"),
            layout: Some(&layout),
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

    /// Recreate the present pipeline for a surface's color format.
    pub fn set_present_format(&mut self, format: GpuTextureFormat) {
        self.present_format = format;
        self.present_pipeline = Self::build_present_pipeline(&self.device, format);
        *self.present_bg.borrow_mut() = None;
    }

    pub fn default_camera(&self) -> mo_core::camera::Camera {
        let mut cam = mo_core::camera::Camera::default();
        cam.width = self.width as f32;
        cam.height = self.height as f32;
        cam
    }

    /// Record the scene into an encoder (caller submits, can add present pass).
    pub fn draw_scene_pass(
        &self,
        state: &std::collections::HashMap<String, std::sync::Arc<VGroup>>,
        cam: &mo_core::camera::Camera,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let frame: FrameUniforms = cam.uniforms();
        self.queue.write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&frame));

        // Sort items by key for consistent deterministic draw order
        let mut items: Vec<_> = state.iter().collect();
        items.sort_by_key(|(k, _)| *k);

        let frame_no = {
            let mut cache = self.buf_cache.borrow_mut();
            cache.frame += 1;
            cache.frame
        };
        self.staging_off.set(0);

        // Resolve cache hits / uploads BEFORE the render pass — staging
        // copies need the encoder, which the pass borrows exclusively.
        // (data_bg, mob_bg, num_curves, has_stroke, has_fill), None = skip.
        let mut resolved: Vec<Option<(wgpu::BindGroup, wgpu::BindGroup, u32, bool, bool)>> =
            Vec::with_capacity(items.len());
        for (id, group) in items {
            // Hash the raw point data + uniforms instead of materializing
            // GpuPointRecords — unchanged objects skip the record build, the
            // upload, AND the has_stroke scan entirely.
            let mob_u = mobject_uniforms(group);
            let cache_key = group_content_hash(id, group, &mob_u);
            let has_fill = mob_u.fill_rgba[3] > 0.0 || mob_u.fill_rgba_end[3] > 0.0;

            let hit = {
                let mut cache = self.buf_cache.borrow_mut();
                if let Some(e) = cache.map.get_mut(&cache_key) {
                    e.last_seen = frame_no;
                    Some((e.data_bg.clone(), e.mob_bg.clone(), e.num_curves, e.has_stroke))
                } else {
                    let (records, num_curves) = prepare_gpu_records(group);
                    if num_curves == 0 || records.is_empty() {
                        None
                    } else {
                        let bytes =
                            (records.len() * std::mem::size_of::<GpuPointRecord>()) as u64;
                        let data_buf = self.take_data_buffer(&mut cache, bytes);
                        let mob_buf = self.take_uniform_buffer(&mut cache);
                        let data_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("mob_data_bg"),
                            layout: &self.data_bgl,
                            entries: &[wgpu::BindGroupEntry {
                                binding: 0,
                                resource: data_buf.as_entire_binding(),
                            }],
                        });
                        let mob_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("mob_uniforms_bg"),
                            layout: &self.mob_bgl,
                            entries: &[wgpu::BindGroupEntry {
                                binding: 0,
                                resource: mob_buf.as_entire_binding(),
                            }],
                        });
                        // Upload via the persistent staging ring; if the ring
                        // is exhausted this frame (huge scene), fall back to a
                        // direct queue write — correct, just not pooled.
                        let staged = self
                            .stage_copy(encoder, &data_buf, bytemuck::cast_slice(&records))
                            && self.stage_copy(encoder, &mob_buf, bytemuck::bytes_of(&mob_u));
                        if !staged {
                            self.queue
                                .write_buffer(&data_buf, 0, bytemuck::cast_slice(&records));
                            self.queue.write_buffer(&mob_buf, 0, bytemuck::bytes_of(&mob_u));
                        }
                        let has_stroke = records
                            .iter()
                            .any(|r| r.stroke_width > 0.0 && r.stroke_rgba[3] > 0.0);
                        cache.total_bytes += bytes;
                        cache.map.insert(
                            cache_key,
                            BufCacheEntry {
                                data_bg: data_bg.clone(),
                                mob_bg: mob_bg.clone(),
                                data_buf,
                                mob_buf,
                                last_seen: frame_no,
                                bytes,
                                num_curves,
                                has_stroke,
                            },
                        );
                        Some((data_bg, mob_bg, num_curves, has_stroke))
                    }
                }
            };
            resolved.push(hit.map(|(data_bg, mob_bg, num_curves, has_stroke)| {
                (data_bg, mob_bg, num_curves, has_stroke, has_fill)
            }));
        }

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
                view: &self.depth_stencil_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0),
                    store: wgpu::StoreOp::Store,
                }),
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        for (data_bg, mob_bg, num_curves, has_stroke, has_fill) in
            resolved.into_iter().flatten()
        {
            if has_fill {
                // Pass 1: Winding count into stencil buffer (no color write)
                pass.set_pipeline(&self.fill_count_pipeline);
                pass.set_bind_group(0, &self.frame_bg, &[]);
                pass.set_bind_group(1, &mob_bg, &[]);
                pass.set_bind_group(2, &data_bg, &[]);
                pass.set_stencil_reference(0);
                pass.draw(0..num_curves * 6, 0..1);

                // Pass 2: Fill border (anti-aliasing band drawn outside where stencil == 0)
                pass.set_pipeline(&self.fill_border_pipeline);
                pass.set_bind_group(0, &self.frame_bg, &[]);
                pass.set_bind_group(1, &mob_bg, &[]);
                pass.set_bind_group(2, &data_bg, &[]);
                pass.set_stencil_reference(0);
                pass.draw(0..(num_curves + 1) * 186, 0..1);

                // Pass 3: Winding cover (shade inside where stencil != 0, reset stencil to 0)
                pass.set_pipeline(&self.fill_cover_pipeline);
                pass.set_bind_group(0, &self.frame_bg, &[]);
                pass.set_bind_group(1, &mob_bg, &[]);
                pass.set_bind_group(2, &data_bg, &[]);
                pass.set_stencil_reference(0);
                pass.draw(0..num_curves * 6, 0..1);
            }

            if has_stroke {
                // Stroke pass (GPU procedural ribbon quads with analytic AA and round joins)
                pass.set_pipeline(&self.stroke_pipeline);
                pass.set_bind_group(0, &self.frame_bg, &[]);
                pass.set_bind_group(1, &mob_bg, &[]);
                pass.set_bind_group(2, &data_bg, &[]);
                pass.draw(0..num_curves * 186, 0..1);
            }
        }

        // Budget-based eviction: stale animated-frame entries die first; static
        // objects survive animation churn (the old clear-all thrashed them).
        let mut cache = self.buf_cache.borrow_mut();
        if cache.map.len() > BUF_CACHE_MAX_ENTRIES || cache.total_bytes > BUF_CACHE_MAX_BYTES {
            let frame = cache.frame;
            let stale: Vec<u64> = cache
                .map
                .iter()
                .filter(|(_, e)| frame - e.last_seen > BUF_CACHE_STALE_FRAMES)
                .map(|(k, _)| *k)
                .collect();
            for k in stale {
                if let Some(e) = cache.map.remove(&k) {
                    cache.total_bytes -= e.bytes;
                    cache.retire(e);
                }
            }
            if cache.map.len() > BUF_CACHE_MAX_ENTRIES || cache.total_bytes > BUF_CACHE_MAX_BYTES {
                let mut by_age: Vec<(u64, u64)> = cache
                    .map
                    .iter()
                    .map(|(k, e)| (e.last_seen, *k))
                    .collect();
                by_age.sort();
                for (_, k) in by_age {
                    if cache.map.len() <= 64 && cache.total_bytes <= BUF_CACHE_MAX_BYTES {
                        break;
                    }
                    if let Some(e) = cache.map.remove(&k) {
                        cache.total_bytes -= e.bytes;
                        cache.retire(e);
                    }
                }
            }
        }
    }

    /// Render a HashMap state to the offscreen target, return RGBA pixels.
    pub fn render(&self, state: &std::collections::HashMap<String, std::sync::Arc<VGroup>>) -> image::RgbaImage {
        let cam = self.default_camera();
        self.render_with_camera(state, &cam)
    }

    /// Render a HashMap state using an explicit Camera (for 3D tilt/azimuth orbit and zoom).
    pub fn render_with_camera(
        &self,
        state: &std::collections::HashMap<String, std::sync::Arc<VGroup>>,
        camera: &mo_core::camera::Camera,
    ) -> image::RgbaImage {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.draw_scene_pass(state, camera, &mut encoder);
        self.queue.submit(Some(encoder.finish()));
        let bytes = self.read_target();
        image::RgbaImage::from_raw(self.width, self.height, bytes).expect("size mismatch")
    }

    /// Present the offscreen target onto an external surface view (IDE window).
    pub fn present_to(&self, encoder: &mut GpuCommandEncoder, view: &GpuTextureView) {
        // Bind group only changes when the target view / pipeline is replaced
        // (resize, set_present_format) — cache it instead of rebuilding per frame.
        let mut cached = self.present_bg.borrow_mut();
        let bg = cached.get_or_insert_with(|| {
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("present_bg"),
                layout: &self.present_pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.target),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            })
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("present"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.present_pipeline);
        pass.set_bind_group(0, &*bg, &[]);
        pass.draw(0..3, 0..1);
    }

    pub fn read_target(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.read_target_into(&mut out);
        out
    }

    /// Readback into a caller-owned buffer — reused across frames so export
    /// loops don't allocate a fresh GPU staging buffer + Vec per frame.
    pub fn read_target_into(&self, result: &mut Vec<u8>) {
        let bytes_per_row = self.width * 4;
        let padded = (bytes_per_row + 255) / 256 * 256;
        let mut rb = self.readback.borrow_mut();
        if !matches!(&*rb, Some((_, p, h)) if *p == padded && *h == self.height) {
            let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("readback"),
                size: (padded * self.height) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            *rb = Some((buf, padded, self.height));
        }
        let (out, padded, height) = rb.as_ref().unwrap();
        let height = *height;
        let padded = *padded;
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
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));
        let slice = out.slice(..);
        let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = sender.send(r);
        });
        self.device.poll(wgpu::Maintain::Wait);
        pollster::block_on(async { receiver.receive().await.expect("map channel") })
            .expect("map failed");
        let data = slice.get_mapped_range();
        result.clear();
        result.reserve((bytes_per_row * height) as usize);
        for row in 0..height {
            let start = (row * padded) as usize;
            result.extend_from_slice(&data[start..start + bytes_per_row as usize]);
        }
        drop(data);
        out.unmap();
    }

    pub fn texture_view(&self) -> &GpuTextureView {
        &self.target
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn device(&self) -> &GpuDevice {
        &self.device
    }

    pub fn queue(&self) -> &GpuQueue {
        &self.queue
    }

    pub fn target_view(&self) -> &GpuTextureView {
        &self.target
    }
}

/// Pack `VGroup` subpaths into GPU-ready `GpuPointRecord`s, setting `subpath_range`
/// for winding base-point calculations and joint tangent lookups.
fn prepare_gpu_records(g: &VGroup) -> (Vec<GpuPointRecord>, u32) {
    if g.points.is_empty() {
        return (Vec::new(), 0);
    }
    let subpaths = if g.subpaths.is_empty() {
        vec![mo_core::vgroup::SubPath {
            start: 0,
            end: g.points.len(),
        }]
    } else {
        g.subpaths.clone()
    };

    let mut records: Vec<GpuPointRecord> = Vec::new();
    let mut total_curves: u32 = 0;

    for (sp_idx, sp) in subpaths.iter().enumerate() {
        if sp.end <= sp.start {
            continue;
        }
        let sp_pts = &g.points[sp.start..sp.end];
        if sp_pts.is_empty() {
            continue;
        }

        // Between subpaths, insert a null curve separator (controls[0] == controls[1])
        if sp_idx > 0 && !records.is_empty() {
            let mut sep_handle = *records.last().unwrap();
            sep_handle.subpath_range = [0.0, 0.0];
            records.push(sep_handle);
            total_curves += 1;
        }

        let start_rec_idx = records.len();
        let n_curves = (sp_pts.len().saturating_sub(1)) / 2;
        let last_anchor_idx = start_rec_idx + (sp_pts.len() - 1);

        for (i, pt) in sp_pts.iter().enumerate() {
            let rec_idx = start_rec_idx + i;
            let range_x = (rec_idx - start_rec_idx) as f32;
            let range_y = (last_anchor_idx - rec_idx) as f32;
            records.push(GpuPointRecord {
                point: pt.point,
                stroke_rgba: pt.stroke_rgba,
                stroke_width: pt.stroke_width,
                subpath_range: [range_x, range_y],
            });
        }
        total_curves += n_curves as u32;
    }

    if records.is_empty() {
        return (Vec::new(), 0);
    }

    // Append 2 padding records so curve reading record+1, record+2 at the end is safe
    let last = *records.last().unwrap();
    records.push(last);
    records.push(last);

    (records, total_curves)
}

fn mobject_uniforms(g: &VGroup) -> MobjectUniforms {
    let fill = g
        .points
        .iter()
        .find(|p| p.fill_rgba[3] > 0.0)
        .map(|p| p.fill_rgba)
        .unwrap_or([0.0, 0.0, 0.0, 0.0]);

    MobjectUniforms {
        is_fixed_in_frame: 0.0,
        _pad0: [0.0; 3],
        shading: [0.0; 3],
        _pad1: 0.0,
        clip_plane0: [0.0; 4],
        clip_plane1: [0.0; 4],
        clip_plane2: [0.0; 4],
        clip_plane3: [0.0; 4],
        anti_alias_width: 1.5,
        joint_roundness: 0.0,
        flat_stroke: 0.0,
        stroke_width_in_scene_units: 0.0,
        unit_normal: [0.0, 0.0, 1.0],
        _pad2: 0.0,
        fill_rgba: fill,
        fill_rgba_end: fill,
        gradient_start: [0.0; 3],
        _pad3: 0.0,
        gradient_end: [0.0; 3],
        fill_border_width: 0.0,
    }
}
