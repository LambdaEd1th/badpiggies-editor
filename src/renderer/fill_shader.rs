//! wgpu + WGSL terrain fill shader — exact port of Unity `_Custom/Unlit_Color_Geometry`.
//!
//! Renders terrain fill meshes with tiled ground textures.
//! Unity shader: `frag: return tex2D(_MainTex, texcoord) * _Color`
//! Blend Off (opaque), ZWrite Off, Cull Off
//!
//! Architecture: per-terrain vertex/index buffers with world-space positions,
//! dynamic uniform buffer with slots (like bg_shader), per-texture bind groups,
//! and a **Repeat** wrap sampler for texture tiling at 5×5 world units.

use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui;
use eframe::wgpu;

/// Maximum number of fill mesh draw calls per frame.
const MAX_DRAW_SLOTS: u32 = 256;

// ── WGSL source — port of Unity _Custom/Unlit_Color_Geometry (terrain fill) ──

const WGSL_SOURCE: &str = r#"
// Port of Unity _Custom/Unlit_Color_Geometry (used for terrain fill)
// Original: return tex2D(_MainTex, i.texcoord) * _Color;
// Blend Off, ZWrite Off, Cull Off
//
// UVs tile at 5×5 world units with wrap=Repeat.

struct Uniforms {
    screen_size: vec2<f32>,
    camera_center: vec2<f32>,
    zoom: f32,
    _pad0: f32,
    tint_color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var main_tex: texture_2d<f32>;
@group(0) @binding(2) var main_sampler: sampler;

struct VIn {
    @location(0) position: vec2<f32>,  // world-space position
    @location(1) uv: vec2<f32>,        // tiled UV = (world - offset) / 5.0
};
struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VIn) -> VOut {
    var out: VOut;
    // World → NDC
    let ndc = (in.position - u.camera_center) * u.zoom / (u.screen_size * 0.5);
    out.position = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    // Exact Unity shader: return tex2D(_MainTex, i.texcoord) * _Color;
    let c = textureSample(main_tex, main_sampler, in.uv) * u.tint_color;

    // Opaque fill — discard fully transparent pixels, premultiply for egui compositor.
    if (c.a < 0.004) { discard; }
    let a = select(c.a, 1.0, c.a >= 0.784);
    return vec4<f32>(c.rgb * a, a);
}
"#;

// ── GPU uniform buffer layout (32 bytes) ──

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FillUniforms {
    pub screen_size: [f32; 2],
    pub camera_center: [f32; 2],
    pub zoom: f32,
    pub _pad0: f32,
    pub _pad1: [f32; 2], // align tint_color to 16-byte boundary
    pub tint_color: [f32; 4],
}

// ── Vertex format ──

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FillVertex {
    pub pos: [f32; 2],  // world-space
    pub uv: [f32; 2],   // tiled UV
}

// ── Shared pipeline resources ──

pub struct FillResources {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// Dynamic uniform buffer.
    uniform_buffer: wgpu::Buffer,
    /// Aligned stride per slot.
    slot_stride: u64,
}

pub fn init_fill_resources(
    device: &wgpu::Device,
    target_format: wgpu::TextureFormat,
) -> FillResources {

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("fill_shader"),
        source: wgpu::ShaderSource::Wgsl(WGSL_SOURCE.into()),
    });

    let min_align = device.limits().min_uniform_buffer_offset_alignment as u64;
    let uniform_size = std::mem::size_of::<FillUniforms>() as u64;
    let slot_stride = uniform_size.div_ceil(min_align) * min_align;

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("fill_bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: std::num::NonZeroU64::new(uniform_size),
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("fill_pl"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("fill_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<FillVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 8,
                        shader_location: 1,
                    },
                ],
            }],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    // Repeat-wrap sampler for terrain texture tiling
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("fill_sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("fill_uniform_buf"),
        size: slot_stride * MAX_DRAW_SLOTS as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    FillResources {
        pipeline,
        bind_group_layout,
        sampler,
        uniform_buffer,
        slot_stride,
    }
}

// ── Per-texture GPU bind group ──

pub struct FillTextureGpu {
    bind_group: wgpu::BindGroup,
}

fn upload_fill_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    resources: &FillResources,
    pixels: &[u8],
    width: u32,
    height: u32,
) -> FillTextureGpu {
    let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("fill_tex"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        size,
    );
    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("fill_tex_bg"),
        layout: &resources.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &resources.uniform_buffer,
                    offset: 0,
                    size: std::num::NonZeroU64::new(std::mem::size_of::<FillUniforms>() as u64),
                }),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&resources.sampler),
            },
        ],
    });
    FillTextureGpu { bind_group }
}

// ── Texture cache ──

pub struct FillTextureCache {
    textures: HashMap<String, Arc<FillTextureGpu>>,
}

impl FillTextureCache {
    pub fn new() -> Self {
        Self { textures: HashMap::new() }
    }

    pub fn get_or_load(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &FillResources,
        filename: &str,
    ) -> Option<Arc<FillTextureGpu>> {
        if let Some(arc) = self.textures.get(filename) {
            return Some(arc.clone());
        }
        let path = format!("ground/{}", filename);
        let data = crate::assets::read_asset(&path)?;
        let img = image::load_from_memory(&data).ok()?.to_rgba8();
        let (w, h) = (img.width(), img.height());
        let pixels = img.into_raw();
        let gpu = upload_fill_texture(device, queue, resources, &pixels, w, h);
        let arc = Arc::new(gpu);
        self.textures.insert(filename.to_string(), arc.clone());
        Some(arc)
    }
}

// ── Per-terrain fill GPU mesh ──

pub struct FillGpuMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

/// Build GPU vertex/index buffers for a terrain fill mesh.
pub fn build_fill_gpu_mesh(
    device: &wgpu::Device,
    vertices: &[FillVertex],
    indices: &[u32],
) -> FillGpuMesh {
    use wgpu::util::DeviceExt;
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("fill_vbo"),
        contents: bytemuck::cast_slice(vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("fill_ibo"),
        contents: bytemuck::cast_slice(indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    FillGpuMesh {
        vertex_buffer,
        index_buffer,
        index_count: indices.len() as u32,
    }
}

// ── Paint callback ──

struct FillPaintCallback {
    resources: Arc<FillResources>,
    texture: Arc<FillTextureGpu>,
    gpu_mesh: Arc<FillGpuMesh>,
    slot: u32,
    uniforms: FillUniforms,
}

impl egui_wgpu::CallbackTrait for FillPaintCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        _callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let offset = self.slot as u64 * self.resources.slot_stride;
        queue.write_buffer(
            &self.resources.uniform_buffer,
            offset,
            bytemuck::bytes_of(&self.uniforms),
        );
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        _callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let offset = self.slot as u64 * self.resources.slot_stride;
        render_pass.set_pipeline(&self.resources.pipeline);
        render_pass.set_bind_group(0, &self.texture.bind_group, &[offset as u32]);
        render_pass.set_vertex_buffer(0, self.gpu_mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(
            self.gpu_mesh.index_buffer.slice(..),
            wgpu::IndexFormat::Uint32,
        );
        render_pass.draw_indexed(0..self.gpu_mesh.index_count, 0, 0..1);
    }
}

/// Build a paint callback shape for one terrain fill mesh.
pub fn make_fill_callback(
    clip_rect: egui::Rect,
    resources: Arc<FillResources>,
    texture: Arc<FillTextureGpu>,
    gpu_mesh: Arc<FillGpuMesh>,
    slot: u32,
    uniforms: FillUniforms,
) -> egui::Shape {
    let cb = FillPaintCallback {
        resources,
        texture,
        gpu_mesh,
        slot,
        uniforms,
    };
    egui::Shape::Callback(egui_wgpu::Callback::new_paint_callback(clip_rect, cb))
}

pub const fn max_draw_slots() -> u32 {
    MAX_DRAW_SLOTS
}
