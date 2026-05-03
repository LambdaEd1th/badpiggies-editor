//! wgpu + WGSL transparent sprite shader — port of Unity `_Custom/Unlit_ColorTransparent_Geometry`.
//!
//! Renders textured quads with standard alpha blending: `Blend SrcAlpha OneMinusSrcAlpha`.
//! Used for all IngameAtlas sprites (gameplay objects) which use this shader family:
//!   `frag: return tex2D(_MainTex, uv) * _Color`
//!
//! Also covers the other IngameAtlas shader variants:
//!   - PreAlpha (dimmed tint via _Color.a)
//!   - Shiny (screen-space shimmer, future)
//!   - Gray (luminance desaturation, future)
//!
//! Architecture: dynamic uniform buffer with slots (like bg_shader), shared unit
//! quad VBO, per-atlas bind groups. Each sprite draw uses a unique slot index.

use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui;
use eframe::wgpu;

use crate::data::sprite_db::UvRect;

/// Maximum number of sprite draw calls per frame.
const MAX_DRAW_SLOTS: u32 = 2048;

// ── WGSL source — port of Unity _Custom/Unlit_ColorTransparent_Geometry ──

const WGSL_SOURCE: &str = r#"
// Port of Unity _Custom/Unlit_ColorTransparent_Geometry + Gray + Shiny variants
// mode 0 (normal):  return tex2D(_MainTex, uv) * _Color
// mode 1 (gray):    lum = dot(tex.rgb, vec3(0.2989, 0.587, 0.114));
//                   return vec4(lum,lum,lum,tex.a) * _Color
// mode 2 (shiny):   shine = 1 - abs((screen_x - _Center) * _Scale);
//                   return tex + clamp(shine,0,1) * _Color * tex.a
//
// Blend SrcAlpha OneMinusSrcAlpha, ZWrite Off, Cull Off

struct Uniforms {
    screen_size: vec2<f32>,       // 0..8
    camera_center: vec2<f32>,     // 8..16
    zoom: f32,                    // 16..20
    rotation: f32,                // 20..24    radians (world-space)
    world_center: vec2<f32>,      // 24..32
    half_size: vec2<f32>,         // 32..40    half-extents in world units
    uv_min: vec2<f32>,            // 40..48
    uv_max: vec2<f32>,            // 48..56
    mode: f32,                    // 56..60    0=normal, 1=gray, 2=shiny
    shine_center: f32,            // 60..64    screen-space X for shiny sweep
    tint_color: vec4<f32>,        // 64..80
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
    @location(1) screen_x: f32,        // NDC x → [0,1] for shiny sweep
};

@vertex
fn vs_main(in: VIn) -> VOut {
    var out: VOut;
    // Unit quad → local offset scaled by half_size (* 2 because quad is [-0.5, 0.5])
    let local = in.position * u.half_size * 2.0;

    // Apply rotation
    let cos_r = cos(u.rotation);
    let sin_r = sin(u.rotation);
    let rotated = vec2<f32>(
        local.x * cos_r - local.y * sin_r,
        local.x * sin_r + local.y * cos_r,
    );

    // Local → world
    let world = u.world_center + rotated;

    // World → NDC
    let ndc = (world - u.camera_center) * u.zoom / (u.screen_size * 0.5);
    out.position = vec4<f32>(ndc, 0.0, 1.0);

    // UV interpolation: [0,1] → [uv_min, uv_max]
    out.uv = mix(u.uv_min, u.uv_max, in.uv);

    // Screen-space X for shiny effect: NDC [-1,1] → [0,1]
    out.screen_x = ndc.x * 0.5 + 0.5;
    return out;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let tex = textureSample(main_tex, main_sampler, in.uv);
    var c: vec4<f32>;

    if (u.mode > 1.5) {
        // Shiny: tex + clamp(shine, 0, 1) * tint_color * tex.a
        // Unity: _Scale default = 10.0
        let shine = 1.0 - abs((in.screen_x - u.shine_center) * 10.0);
        c = tex + clamp(shine, 0.0, 1.0) * u.tint_color * tex.a;
    } else if (u.mode > 0.5) {
        // Gray: luminance desaturation then tint
        let lum = 0.2989 * tex.r + 0.587 * tex.g + 0.114 * tex.b;
        c = vec4<f32>(lum, lum, lum, tex.a) * u.tint_color;
    } else {
        // Normal: tex * tint_color
        c = tex * u.tint_color;
    }

    // Discard fully transparent pixels to avoid Z artifacts
    if (c.a < 0.004) { discard; }

    // Premultiply for egui compositor (egui uses premultiplied alpha blending)
    return vec4<f32>(c.rgb * c.a, c.a);
}
"#;

// ── GPU uniform buffer layout (80 bytes, 16-byte aligned) ──

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpriteUniforms {
    pub screen_size: [f32; 2],
    pub camera_center: [f32; 2],
    pub zoom: f32,
    pub rotation: f32,
    pub world_center: [f32; 2],
    pub half_size: [f32; 2],
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    /// 0.0 = normal, 1.0 = gray (luminance), 2.0 = shiny (screen-space sweep)
    pub mode: f32,
    /// Screen-space X center for shiny sweep (only used when mode == 2.0)
    pub shine_center: f32,
    pub tint_color: [f32; 4],
}

// ── Shared pipeline resources ──

/// Shared GPU resources for the sprite shader pipeline (created once at init).
pub struct SpriteResources {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// Shared unit quad VBO: [-0.5, 0.5] with UVs [0, 1].
    quad_vbo: wgpu::Buffer,
    /// Shared quad IBO: [0, 1, 2, 0, 2, 3].
    quad_ibo: wgpu::Buffer,
    /// Dynamic uniform buffer (MAX_DRAW_SLOTS × aligned stride).
    uniform_buffer: wgpu::Buffer,
    /// Aligned stride per slot (uniform size rounded up to device alignment).
    slot_stride: u64,
}

/// Initialize the transparent sprite shader pipeline.
pub fn init_sprite_resources(
    device: &wgpu::Device,
    target_format: wgpu::TextureFormat,
) -> SpriteResources {
    use wgpu::util::DeviceExt;

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("sprite_shader"),
        source: wgpu::ShaderSource::Wgsl(WGSL_SOURCE.into()),
    });

    // Minimum uniform buffer offset alignment (typically 256 bytes)
    let min_align = device.limits().min_uniform_buffer_offset_alignment as u64;
    let uniform_size = std::mem::size_of::<SpriteUniforms>() as u64;
    let slot_stride = uniform_size.div_ceil(min_align) * min_align;

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("sprite_bgl"),
        entries: &[
            // @binding(0) uniform buffer (dynamic offset)
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
        label: Some("sprite_pl"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("sprite_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: 16, // 2 × f32 pos + 2 × f32 uv
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
            cull_mode: None, // Cull Off
            ..Default::default()
        },
        depth_stencil: None, // ZWrite Off
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("sprite_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    // Unit quad VBO: [-0.5, 0.5] with UVs [0, 1]
    #[rustfmt::skip]
    let quad_vertices: [f32; 16] = [
        // pos.x  pos.y   u    v
        -0.5,  0.5,   0.0, 0.0,  // TL
         0.5,  0.5,   1.0, 0.0,  // TR
         0.5, -0.5,   1.0, 1.0,  // BR
        -0.5, -0.5,   0.0, 1.0,  // BL
    ];
    let quad_vbo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("sprite_quad_vbo"),
        contents: bytemuck::cast_slice(&quad_vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let quad_ibo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("sprite_quad_ibo"),
        contents: bytemuck::cast_slice(&[0u16, 1, 2, 0, 2, 3]),
        usage: wgpu::BufferUsages::INDEX,
    });

    // Dynamic uniform buffer large enough for all draw slots
    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("sprite_uniform_buf"),
        size: slot_stride * MAX_DRAW_SLOTS as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    SpriteResources {
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

/// GPU texture for a sprite atlas, with a bind group referencing the shared
/// uniform buffer (dynamic offset), texture view, and sampler.
pub struct SpriteAtlasGpu {
    bind_group: wgpu::BindGroup,
    pub width: u32,
    pub height: u32,
}

/// Upload raw RGBA pixels as a wgpu texture for a sprite atlas.
pub fn upload_sprite_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    resources: &SpriteResources,
    pixels: &[u8],
    width: u32,
    height: u32,
) -> SpriteAtlasGpu {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sprite_atlas_tex"),
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
        label: Some("sprite_atlas_bg"),
        layout: &resources.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &resources.uniform_buffer,
                    offset: 0,
                    size: std::num::NonZeroU64::new(std::mem::size_of::<SpriteUniforms>() as u64),
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

    SpriteAtlasGpu {
        bind_group,
        width,
        height,
    }
}

// ── Atlas cache ──

/// Cache of loaded sprite atlas textures, keyed by filename.
pub struct SpriteAtlasCache {
    atlases: HashMap<String, Arc<SpriteAtlasGpu>>,
}

impl SpriteAtlasCache {
    pub fn new() -> Self {
        Self {
            atlases: HashMap::new(),
        }
    }

    /// Get or load a sprite atlas. Returns None if the asset isn't found.
    pub fn get_or_load(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &SpriteResources,
        filename: &str,
    ) -> Option<Arc<SpriteAtlasGpu>> {
        if let Some(arc) = self.atlases.get(filename) {
            return Some(arc.clone());
        }
        let atlas = load_sprite_atlas(device, queue, resources, filename)?;
        let arc = Arc::new(atlas);
        self.atlases.insert(filename.to_string(), arc.clone());
        Some(arc)
    }
}

/// Load a sprite atlas from embedded assets.
fn load_sprite_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    resources: &SpriteResources,
    filename: &str,
) -> Option<SpriteAtlasGpu> {
    let path = format!("sprites/{}", filename);
    let data = crate::data::assets::read_asset(&path)
        .or_else(|| crate::data::assets::read_asset(&format!("props/{}", filename)))?;
    let img = image::load_from_memory(&data).ok()?.to_rgba8();
    let (w, h) = (img.width(), img.height());
    let pixels = img.into_raw();
    Some(upload_sprite_atlas(device, queue, resources, &pixels, w, h))
}

// ── Uniform builder helper ──

/// Compute UV coordinates for a sprite, handling Y-flip and half-texel inset.
pub fn compute_uvs(
    uv: &UvRect,
    atlas_w: f32,
    atlas_h: f32,
    flip_x: bool,
    flip_y: bool,
) -> ([f32; 2], [f32; 2]) {
    // UV Y-flip: Unity V=0 at bottom, wgpu V=0 at top
    let uv_min_u = uv.x;
    let uv_max_u = uv.x + uv.w;
    let uv_min_v = 1.0 - uv.y - uv.h;
    let uv_max_v = 1.0 - uv.y;

    // Half-texel UV inset to prevent atlas bleeding
    let htu = 0.5 / atlas_w;
    let htv = 0.5 / atlas_h;
    let uv_min_u = uv_min_u + htu;
    let uv_max_u = uv_max_u - htu;
    let uv_min_v = uv_min_v + htv;
    let uv_max_v = uv_max_v - htv;

    // Handle flip via UV swap
    let (u0, u1) = if flip_x {
        (uv_max_u, uv_min_u)
    } else {
        (uv_min_u, uv_max_u)
    };
    let (v0, v1) = if flip_y {
        (uv_max_v, uv_min_v)
    } else {
        (uv_min_v, uv_max_v)
    };

    ([u0, v0], [u1, v1])
}

/// Maximum draw slot count (exposed for external counter validation).
pub const fn max_draw_slots() -> u32 {
    MAX_DRAW_SLOTS
}

// ── Batched paint callback ──

/// A single draw within a batched sprite callback.
pub struct SpriteBatchDraw {
    pub atlas: Arc<SpriteAtlasGpu>,
    pub slot: u32,
    pub uniforms: SpriteUniforms,
}

struct SpriteBatchPaintCallback {
    resources: Arc<SpriteResources>,
    draws: Vec<SpriteBatchDraw>,
}

impl egui_wgpu::CallbackTrait for SpriteBatchPaintCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        _callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        for draw in &self.draws {
            let offset = draw.slot as u64 * self.resources.slot_stride;
            queue.write_buffer(
                &self.resources.uniform_buffer,
                offset,
                bytemuck::bytes_of(&draw.uniforms),
            );
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        _callback_resources: &egui_wgpu::CallbackResources,
    ) {
        if self.draws.is_empty() {
            return;
        }
        render_pass.set_pipeline(&self.resources.pipeline);
        render_pass.set_vertex_buffer(0, self.resources.quad_vbo.slice(..));
        render_pass.set_index_buffer(self.resources.quad_ibo.slice(..), wgpu::IndexFormat::Uint16);
        for draw in &self.draws {
            let offset = draw.slot as u64 * self.resources.slot_stride;
            render_pass.set_bind_group(0, &draw.atlas.bind_group, &[offset as u32]);
            render_pass.draw_indexed(0..6, 0, 0..1);
        }
    }
}

/// Build a single batched paint callback for multiple transparent sprites.
///
/// All sprite draws execute in one render pass, avoiding per-sprite render pass
/// switches that cause frame drops in large levels.
pub fn make_sprite_batch_callback(
    clip_rect: egui::Rect,
    resources: Arc<SpriteResources>,
    draws: Vec<SpriteBatchDraw>,
) -> egui::Shape {
    let cb = SpriteBatchPaintCallback { resources, draws };
    egui::Shape::Callback(egui_wgpu::Callback::new_paint_callback(clip_rect, cb))
}
