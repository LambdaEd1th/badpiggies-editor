//! WGSL background shader — unified port of Unity's background rendering shaders.
//!
//! Three original Unity shaders, unified with `cutoff` + `tint_color` uniforms:
//!
//! | Shader | Fragment | Blend | Usage |
//! |--------|----------|-------|-------|
//! | `_Custom/Unlit_Color_Geometry` | `tex * _Color` | Off (opaque) | Solid fill layers (Sky, Ground) |
//! | `_Custom/Unlit_ColorTransparent_Geometry` | `tex * _Color` | SrcAlpha 1-SrcAlpha | Far hills, clouds |
//! | `Unlit/Transparent Cutout` (fileID 10750) | `tex` (no _Color!) | Alpha test | Cave shapes, plateaus, fills |
//!
//! **Note**: `Unlit/Transparent Cutout` does NOT use `_Color`. The `_Color` values on
//! materials using this shader (e.g. cave fill layers) are dead data, ignored at runtime.
//!
//! Unified as a single WGSL shader with `cutoff` uniform controlling alpha test.
//! Uses a shared large uniform buffer with dynamic offsets for efficient multi-sprite
//! rendering without per-sprite bind group allocation.

use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui;
use eframe::wgpu;

/// Maximum number of sprite draw calls per frame.
const MAX_DRAW_SLOTS: u32 = 1024;

// ── WGSL source — unified port of three Unity background shaders ──

const WGSL_SOURCE: &str = r#"
// Unified port of Unity background shaders:
//   _Custom/Unlit_Color_Geometry         — opaque fills
//   _Custom/Unlit_ColorTransparent_Geometry — alpha blend (far hills, clouds)
//   Unlit/Transparent Cutout             — alpha test (near hills, beach)
//
// Common original fragment: return tex2D(_MainTex, i.texcoord) * _Color;
// Blend Off / SrcAlpha OneMinusSrcAlpha / alpha-test are handled by `cutoff` uniform.

struct Uniforms {
    screen_size: vec2<f32>,
    camera_center: vec2<f32>,
    zoom: f32,
    cutoff: f32,
    world_center: vec2<f32>,
    world_size: vec2<f32>,
    uv_min: vec2<f32>,
    uv_max: vec2<f32>,
    content_ratio_x: f32,  // original_w / extended_w (1.0 = no extension)
    tint_color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var main_tex: texture_2d<f32>;
@group(0) @binding(2) var main_sampler: sampler;

struct VIn {
    @location(0) position: vec2<f32>,  // unit quad [-0.5, 0.5]
    @location(1) uv: vec2<f32>,        // [0, 1]
};
struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VIn) -> VOut {
    var out: VOut;
    // Unit quad → world position (center + offset * size)
    let world = u.world_center + in.position * u.world_size;
    // World → NDC (same camera transform as edge_shader / opaque_shader)
    let ndc = (world - u.camera_center) * u.zoom / (u.screen_size * 0.5);
    out.position = vec4<f32>(ndc, 0.0, 1.0);
    // UV interpolation: map [0,1] → [uv_min, uv_max]
    // When sprite is extended (content_ratio_x < 1), compress UV towards
    // center so extended portions sample near-center texels — no hard seam.
    let mapped_x = (in.uv.x - 0.5) * u.content_ratio_x + 0.5;
    out.uv = mix(u.uv_min, u.uv_max, vec2(mapped_x, in.uv.y));
    return out;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    // Exact Unity shader: return tex2D(_MainTex, i.texcoord) * _Color;
    let c = textureSample(main_tex, main_sampler, in.uv) * u.tint_color;

    // Texture is already premultiplied at load time (matching Unity's
    // "Alpha Is Transparency" import).  Shader modes:
    //   cutoff < 0     → opaque fill (force alpha = 1)
    //   cutoff ≈ 0.004 → transparent blend (discard fully-transparent only)
    //   cutoff = 0.5   → alpha cutout (Unlit/Transparent Cutout)
    if (u.cutoff < 0.0) {
        return vec4<f32>(c.rgb, 1.0);
    }
    if (c.a < u.cutoff) { discard; }

    // Already premultiplied — pass through for egui compositor
    return vec4<f32>(c.rgb, c.a);
}
"#;

// ── GPU uniform buffer layout (80 bytes, 16-byte aligned) ──
//
// WGSL auto-pads between content_ratio_x (ending at 60) and tint_color
// (align 16 → offset 64).  Rust repr(C) needs explicit padding to match.

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BgUniforms {
    pub screen_size: [f32; 2],
    pub camera_center: [f32; 2],
    pub zoom: f32,
    pub cutoff: f32,
    pub world_center: [f32; 2],
    pub world_size: [f32; 2],
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    pub content_ratio_x: f32, // original_w / extended_w (1.0 = no extension)
    pub _pad: f32,            // align tint_color to offset 64
    pub tint_color: [f32; 4], // vec4 needs 16-byte alignment
}

// ── Vertex format (unit quad) ──

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct BgVertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

/// Unit quad: TL, TR, BR, BL
const UNIT_QUAD: [BgVertex; 4] = [
    BgVertex {
        pos: [-0.5, 0.5],
        uv: [0.0, 0.0],
    },
    BgVertex {
        pos: [0.5, 0.5],
        uv: [1.0, 0.0],
    },
    BgVertex {
        pos: [0.5, -0.5],
        uv: [1.0, 1.0],
    },
    BgVertex {
        pos: [-0.5, -0.5],
        uv: [0.0, 1.0],
    },
];

// ── Shared pipeline resources ──

pub struct BgResources {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    quad_vbo: wgpu::Buffer,
    quad_ibo: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    /// Byte stride between consecutive uniform slots (aligned to device minimum).
    slot_stride: u64,
}

/// Initialize the wgpu render pipeline and shared resources.
pub fn init_bg_resources(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> BgResources {
    use wgpu::util::DeviceExt;

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("bg_shader"),
        source: wgpu::ShaderSource::Wgsl(WGSL_SOURCE.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("bg_bgl"),
        entries: &[
            // @binding(0) uniform buffer (dynamic offset)
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: std::num::NonZeroU64::new(
                        std::mem::size_of::<BgUniforms>() as u64
                    ),
                },
                count: None,
            },
            // @binding(1) atlas texture
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
            // @binding(2) sampler
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("bg_pl"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("bg_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<BgVertex>() as u64,
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
            cull_mode: None, // Cull Off (all three Unity shaders)
            ..Default::default()
        },
        depth_stencil: None, // ZWrite Off (all three Unity shaders)
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("bg_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let quad_vbo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("bg_quad_vbo"),
        contents: bytemuck::cast_slice(&UNIT_QUAD),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let quad_ibo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("bg_quad_ibo"),
        contents: bytemuck::cast_slice(&[0u16, 1, 2, 0, 2, 3]),
        usage: wgpu::BufferUsages::INDEX,
    });

    // Dynamic uniform offset alignment
    let align = device.limits().min_uniform_buffer_offset_alignment as u64;
    let uniform_size = std::mem::size_of::<BgUniforms>() as u64;
    let slot_stride = uniform_size.div_ceil(align) * align;
    let buffer_size = slot_stride * MAX_DRAW_SLOTS as u64;

    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bg_uniform_buf"),
        size: buffer_size,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    BgResources {
        pipeline,
        bind_group_layout,
        sampler,
        quad_vbo,
        quad_ibo,
        uniform_buffer,
        slot_stride,
    }
}

// ── Per-atlas GPU texture + bind group ──

/// GPU texture for a background atlas, with a bind group referencing the shared
/// uniform buffer (dynamic offset), texture view, and sampler.
pub struct BgAtlasGpu {
    bind_group: wgpu::BindGroup,
}

/// Upload raw RGBA pixels as a wgpu texture, creating a bind group tied to the
/// shared uniform buffer from `resources`.
pub fn upload_bg_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    resources: &BgResources,
    pixels: &[u8],
    width: u32,
    height: u32,
) -> BgAtlasGpu {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bg_atlas_tex"),
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
        label: Some("bg_atlas_bg"),
        layout: &resources.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &resources.uniform_buffer,
                    offset: 0,
                    size: std::num::NonZeroU64::new(std::mem::size_of::<BgUniforms>() as u64),
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

    BgAtlasGpu { bind_group }
}

/// Load a background texture from embedded assets using the given path prefix
/// (e.g. `"bg"` or `"sky"`).
pub fn load_bg_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    resources: &BgResources,
    prefix: &str,
    filename: &str,
) -> Option<BgAtlasGpu> {
    let path = format!("{}/{}", prefix, filename);
    let data = crate::assets::read_asset(&path)?;
    let img = image::load_from_memory(&data).ok()?.to_rgba8();
    let (w, h) = (img.width(), img.height());
    // Premultiply alpha into RGB — matches Unity's "Alpha Is Transparency"
    // import setting.  This ensures bilinear filtering between alpha=0 and
    // alpha=255 texels produces the same (dark) colour as Unity.
    let mut pixels = img.into_raw();
    for chunk in pixels.chunks_exact_mut(4) {
        let a = chunk[3] as u16;
        chunk[0] = ((chunk[0] as u16 * a) / 255) as u8;
        chunk[1] = ((chunk[1] as u16 * a) / 255) as u8;
        chunk[2] = ((chunk[2] as u16 * a) / 255) as u8;
    }
    Some(upload_bg_atlas(device, queue, resources, &pixels, w, h))
}

// ── Atlas cache management ──

/// Cache of loaded background atlas textures, keyed by filename.
pub struct BgAtlasCache {
    atlases: HashMap<String, Arc<BgAtlasGpu>>,
}

impl BgAtlasCache {
    pub fn new() -> Self {
        Self {
            atlases: HashMap::new(),
        }
    }

    /// Get or load a background atlas. Returns None if the asset isn't found.
    pub fn get_or_load(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &BgResources,
        filename: &str,
    ) -> Option<Arc<BgAtlasGpu>> {
        if let Some(arc) = self.atlases.get(filename) {
            return Some(arc.clone());
        }
        // Determine prefix from filename
        let prefix = if filename.contains("Sky_Texture") || filename.contains("_sky.") {
            "sky"
        } else {
            "bg"
        };
        let atlas = load_bg_texture(device, queue, resources, prefix, filename)?;
        let arc = Arc::new(atlas);
        self.atlases.insert(filename.to_string(), arc.clone());
        Some(arc)
    }
}

// ── Paint callback ──

struct BgPaintCallback {
    resources: Arc<BgResources>,
    atlas: Arc<BgAtlasGpu>,
    slot: u32,
    uniforms: BgUniforms,
}

impl egui_wgpu::CallbackTrait for BgPaintCallback {
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
        render_pass.set_bind_group(0, &self.atlas.bind_group, &[offset as u32]);
        render_pass.set_vertex_buffer(0, self.resources.quad_vbo.slice(..));
        render_pass.set_index_buffer(self.resources.quad_ibo.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..6, 0, 0..1);
    }
}

/// Build a paint callback shape for one background sprite draw.
///
/// `slot` must be a unique index in `0..MAX_DRAW_SLOTS` for this frame.
pub fn make_bg_callback(
    clip_rect: egui::Rect,
    resources: Arc<BgResources>,
    atlas: Arc<BgAtlasGpu>,
    slot: u32,
    uniforms: BgUniforms,
) -> egui::Shape {
    let cb = BgPaintCallback {
        resources,
        atlas,
        slot,
        uniforms,
    };
    egui::Shape::Callback(egui_wgpu::Callback::new_paint_callback(clip_rect, cb))
}

/// Maximum draw slot count (exposed for external slot counter validation).
pub const fn max_draw_slots() -> u32 {
    MAX_DRAW_SLOTS
}
