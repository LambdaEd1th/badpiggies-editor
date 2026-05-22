//! WGSL background shaders — split one-to-one with the Unity background shaders.
//!
//! Background materials currently resolve to six Unity shader kinds, so runtime
//! keeps six WGSL shader source files with matching pipeline selection:
//! - `_Custom/Unlit_Monochrome`
//! - `_Custom/Unlit_Color_Geometry`
//! - `_Custom/Unlit_ColorTransparent_Geometry`
//! - `_Custom/Unlit_Alpha8Bit_Color`
//! - `Unlit/Transparent`
//! - `Unlit/Transparent Cutout`
//!
//! The renderer chooses the WGSL file directly from `MaterialShaderKind` rather
//! than multiplexing all modes through one shader branch.

use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui;
use eframe::wgpu;

use crate::domain::level::refs::MaterialShaderKind;

/// Maximum number of sprite draw calls per frame.
const MAX_DRAW_SLOTS: u32 = 1024;

pub const WHITE_TEXTURE_KEY: &str = "__bg_white__";

const CUSTOM_UNLIT_MONOCHROME_WGSL: &str =
    include_str!("../../editor_assets/shader/_custom__unlit_monochrome.wgsl");
const CUSTOM_UNLIT_COLOR_GEOMETRY_WGSL: &str =
    include_str!("../../editor_assets/shader/_custom__unlit_color_geometry.wgsl");
const CUSTOM_UNLIT_COLOR_TRANSPARENT_GEOMETRY_WGSL: &str =
    include_str!("../../editor_assets/shader/_custom__unlit_colortransparent_geometry.wgsl");
const CUSTOM_UNLIT_ALPHA8BIT_COLOR_WGSL: &str =
    include_str!("../../editor_assets/shader/_custom__unlit_alpha8bit_color.wgsl");
const BUILTIN_UNLIT_TRANSPARENT_WGSL: &str =
    include_str!("../../editor_assets/shader/unlit__transparent.wgsl");
const BUILTIN_UNLIT_TRANSPARENT_CUTOUT_WGSL: &str =
    include_str!("../../editor_assets/shader/unlit__transparent_cutout.wgsl");

const BACKGROUND_SHADER_KINDS: [MaterialShaderKind; 6] = [
    MaterialShaderKind::CustomUnlitMonochrome,
    MaterialShaderKind::CustomUnlitColorGeometry,
    MaterialShaderKind::CustomUnlitColorTransparentGeometry,
    MaterialShaderKind::CustomUnlitAlpha8BitColor,
    MaterialShaderKind::BuiltinUnlitTransparent,
    MaterialShaderKind::BuiltinUnlitTransparentCutout,
];

fn background_shader_kinds() -> &'static [MaterialShaderKind; 6] {
    &BACKGROUND_SHADER_KINDS
}

fn background_shader_label(kind: MaterialShaderKind) -> &'static str {
    match kind {
        MaterialShaderKind::CustomUnlitMonochrome => "_custom__unlit_monochrome__background_shader",
        MaterialShaderKind::CustomUnlitColorGeometry => {
            "_custom__unlit_color_geometry__background_shader"
        }
        MaterialShaderKind::CustomUnlitColorTransparentGeometry => {
            "_custom__unlit_colortransparent_geometry__background_shader"
        }
        MaterialShaderKind::CustomUnlitAlpha8BitColor => {
            "_custom__unlit_alpha8bit_color__background_shader"
        }
        MaterialShaderKind::BuiltinUnlitTransparent => "unlit__transparent__background_shader",
        MaterialShaderKind::BuiltinUnlitTransparentCutout => {
            "unlit__transparent_cutout__background_shader"
        }
    }
}

fn background_pipeline_label(kind: MaterialShaderKind) -> &'static str {
    match kind {
        MaterialShaderKind::CustomUnlitMonochrome => {
            "_custom__unlit_monochrome__background_pipeline"
        }
        MaterialShaderKind::CustomUnlitColorGeometry => {
            "_custom__unlit_color_geometry__background_pipeline"
        }
        MaterialShaderKind::CustomUnlitColorTransparentGeometry => {
            "_custom__unlit_colortransparent_geometry__background_pipeline"
        }
        MaterialShaderKind::CustomUnlitAlpha8BitColor => {
            "_custom__unlit_alpha8bit_color__background_pipeline"
        }
        MaterialShaderKind::BuiltinUnlitTransparent => "unlit__transparent__background_pipeline",
        MaterialShaderKind::BuiltinUnlitTransparentCutout => {
            "unlit__transparent_cutout__background_pipeline"
        }
    }
}

fn background_shader_source(kind: MaterialShaderKind) -> &'static str {
    match kind {
        MaterialShaderKind::CustomUnlitMonochrome => CUSTOM_UNLIT_MONOCHROME_WGSL,
        MaterialShaderKind::CustomUnlitColorGeometry => CUSTOM_UNLIT_COLOR_GEOMETRY_WGSL,
        MaterialShaderKind::CustomUnlitColorTransparentGeometry => {
            CUSTOM_UNLIT_COLOR_TRANSPARENT_GEOMETRY_WGSL
        }
        MaterialShaderKind::CustomUnlitAlpha8BitColor => CUSTOM_UNLIT_ALPHA8BIT_COLOR_WGSL,
        MaterialShaderKind::BuiltinUnlitTransparent => BUILTIN_UNLIT_TRANSPARENT_WGSL,
        MaterialShaderKind::BuiltinUnlitTransparentCutout => BUILTIN_UNLIT_TRANSPARENT_CUTOUT_WGSL,
    }
}

fn is_transparent_shader(kind: MaterialShaderKind) -> bool {
    matches!(
        kind,
        MaterialShaderKind::CustomUnlitColorTransparentGeometry
            | MaterialShaderKind::CustomUnlitAlpha8BitColor
            | MaterialShaderKind::BuiltinUnlitTransparent
    )
}

fn unity_alpha_blend_state() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
    }
}

fn background_shader_blend(kind: MaterialShaderKind) -> Option<wgpu::BlendState> {
    is_transparent_shader(kind).then(unity_alpha_blend_state)
}

// ── GPU uniform buffer layout (96 bytes, 16-byte aligned) ──
//
// WGSL aligns `main_tex_st` to 16 bytes, so Rust keeps one explicit 4-byte pad
// after `content_ratio_x` to land `main_tex_st` at offset 64.

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
    pub _pad0: f32,
    pub main_tex_st: [f32; 4],
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
    pipelines: HashMap<MaterialShaderKind, wgpu::RenderPipeline>,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    quad_vbo: wgpu::Buffer,
    quad_ibo: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    /// Byte stride between consecutive uniform slots (aligned to device minimum).
    slot_stride: u64,
}

impl BgResources {
    fn pipeline_for(&self, kind: MaterialShaderKind) -> &wgpu::RenderPipeline {
        self.pipelines
            .get(&kind)
            .unwrap_or_else(|| panic!("missing background pipeline for {:?}", kind))
    }
}

/// Initialize the wgpu render pipeline and shared resources.
pub fn init_bg_resources(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> BgResources {
    use wgpu::util::DeviceExt;

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("background_bind_group_layout"),
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
        label: Some("background_pipeline_layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });

    let create_pipeline = |kind: MaterialShaderKind| {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(background_shader_label(kind)),
            source: wgpu::ShaderSource::Wgsl(background_shader_source(kind).into()),
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(background_pipeline_label(kind)),
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
                    blend: background_shader_blend(kind),
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
        })
    };

    let pipelines = background_shader_kinds()
        .iter()
        .copied()
        .map(|kind| (kind, create_pipeline(kind)))
        .collect();

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("background_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let quad_vbo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("background_quad_vertex_buffer"),
        contents: bytemuck::cast_slice(&UNIT_QUAD),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let quad_ibo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("background_quad_index_buffer"),
        contents: bytemuck::cast_slice(&[0u16, 1, 2, 0, 2, 3]),
        usage: wgpu::BufferUsages::INDEX,
    });

    // Dynamic uniform offset alignment
    let align = device.limits().min_uniform_buffer_offset_alignment as u64;
    let uniform_size = std::mem::size_of::<BgUniforms>() as u64;
    let slot_stride = uniform_size.div_ceil(align) * align;
    let buffer_size = slot_stride * MAX_DRAW_SLOTS as u64;

    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("background_uniform_buffer"),
        size: buffer_size,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    BgResources {
        pipelines,
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
        label: Some("background_atlas_texture"),
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
        label: Some("background_atlas_bind_group"),
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

fn background_texture_asset_path(filename: &str) -> String {
    format!("Assets/Texture2D/{filename}")
}

/// Load a background texture from embedded Unity assets.
pub fn load_bg_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    resources: &BgResources,
    filename: &str,
) -> Option<BgAtlasGpu> {
    if filename == WHITE_TEXTURE_KEY {
        return Some(upload_bg_atlas(
            device,
            queue,
            resources,
            &[255, 255, 255, 255],
            1,
            1,
        ));
    }

    let data = crate::data::assets::read_pathname(&background_texture_asset_path(filename))?;
    let img = image::load_from_memory(&data).ok()?.to_rgba8();
    let (w, h) = (img.width(), img.height());
    let pixels = img.into_raw();
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
        let atlas = load_bg_texture(device, queue, resources, filename)?;
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
    shader_kind: MaterialShaderKind,
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
        let pipeline = self.resources.pipeline_for(self.shader_kind);
        render_pass.set_pipeline(pipeline);
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
    shader_kind: MaterialShaderKind,
) -> egui::Shape {
    let cb = BgPaintCallback {
        resources,
        atlas,
        slot,
        uniforms,
        shader_kind,
    };
    egui::Shape::Callback(egui_wgpu::Callback::new_paint_callback(clip_rect, cb))
}

/// Maximum draw slot count (exposed for external slot counter validation).
pub const fn max_draw_slots() -> u32 {
    MAX_DRAW_SLOTS
}

#[cfg(test)]
mod tests {
    use super::{background_shader_kinds, background_shader_source, background_texture_asset_path};

    #[test]
    fn background_textures_load_from_unity_texture2d_namespace() {
        let atlas_name = crate::data::bg_data::bg_atlas_files()
            .first()
            .expect("expected embedded background atlas asset");
        let sky_name = crate::data::bg_data::sky_texture_files()
            .first()
            .expect("expected embedded background sky asset");

        assert!(
            crate::data::assets::read_pathname(&background_texture_asset_path(atlas_name))
                .is_some(),
            "expected background atlas {} to exist under Assets/Texture2D",
            atlas_name
        );
        assert!(
            crate::data::assets::read_pathname(&background_texture_asset_path(sky_name)).is_some(),
            "expected background sky {} to exist under Assets/Texture2D",
            sky_name
        );
    }

    #[test]
    fn background_wgsl_shader_file_count_matches_unity_material_modes() {
        assert_eq!(background_shader_kinds().len(), 6);
        for kind in background_shader_kinds() {
            let source = background_shader_source(*kind);
            assert!(source.contains("@vertex"));
            assert!(source.contains("@fragment"));
        }
    }
}
