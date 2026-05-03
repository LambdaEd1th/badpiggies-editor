//! wgpu + WGSL opaque sprite shader — exact port of Unity `_Custom/Unlit_Color_Geometry`.
//!
//! Renders textured quads matching Unity's Blend Off behavior.
//! Used for `Props_Generic_Sheet_01.png` sprites (CartoonFrameSprite) which use
//! this shader in the original game: `frag: return tex2D(_MainTex, uv) * _Color`.

use std::sync::Arc;

use eframe::egui;
use eframe::wgpu;

use crate::data::sprite_db::UvRect;

// ── WGSL shader source — exact port of Unity _Custom/Unlit_Color_Geometry ──

const WGSL_SOURCE: &str = r#"
// Unity _Custom/Unlit_Color_Geometry — Properties { _MainTex, _Color }
// Original: return tex2D(_MainTex, i.texcoord) * _Color;
// Blend Off, ZWrite Off, Cull Off

struct Uniforms {
    screen_size: vec2<f32>,
    camera_center: vec2<f32>,
    zoom: f32,
    y_offset: f32,          // per-sprite animation offset (world units)
    tint_color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var main_tex: texture_2d<f32>;
@group(0) @binding(2) var main_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // World → NDC: same transform as edge_shader (camera center + zoom + screen size)
    let sx = (in.position.x - u.camera_center.x) * u.zoom;
    let sy = (in.position.y + u.y_offset - u.camera_center.y) * u.zoom;
    out.position = vec4<f32>(
        sx / (u.screen_size.x * 0.5),
        sy / (u.screen_size.y * 0.5),
        0.0, 1.0
    );
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Exact Unity shader: return tex2D(_MainTex, i.texcoord) * _Color;
    let c = textureSample(main_tex, main_sampler, in.uv) * u.tint_color;

    // Editor adaptation: Unity uses Blend Off with shaped meshes (no transparent
    // pixels drawn). The editor uses full quads, so we discard transparent pixels
    // and premultiply for egui's compositing pipeline.
    if (c.a < 0.004) { discard; }
    // Interior pixels (alpha >= ~200/255) → fully opaque, matching Blend Off
    let a = select(c.a, 1.0, c.a >= 0.784);
    return vec4<f32>(c.rgb * a, a);
}
"#;

// ── GPU uniform buffer layout ──

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    screen_size: [f32; 2],
    camera_center: [f32; 2],
    zoom: f32,
    y_offset: f32,
    _pad0: f32,
    _pad1: f32,
    tint_color: [f32; 4], // vec4<f32> needs 16-byte alignment → offset 32
}

// ── Vertex format: position (world) + UV ──

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct OpaqueVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
}

// ── Shared pipeline resources ──

pub struct OpaqueResources {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// Shared quad index buffer: [0, 1, 2, 0, 2, 3].
    index_buffer: wgpu::Buffer,
}

/// Initialize the wgpu render pipeline and shared resources.
pub fn init_opaque_resources(
    device: &wgpu::Device,
    target_format: wgpu::TextureFormat,
) -> OpaqueResources {
    use wgpu::util::DeviceExt;

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("opaque_shader"),
        source: wgpu::ShaderSource::Wgsl(WGSL_SOURCE.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("opaque_bgl"),
        entries: &[
            // @binding(0) uniform buffer
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
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
        label: Some("opaque_pl"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("opaque_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<OpaqueVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    // @location(0) position: vec2<f32>
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    },
                    // @location(1) uv: vec2<f32>
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
        label: Some("opaque_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("opaque_quad_ibo"),
        contents: bytemuck::cast_slice(&[0u16, 1, 2, 0, 2, 3]),
        usage: wgpu::BufferUsages::INDEX,
    });

    OpaqueResources {
        pipeline,
        bind_group_layout,
        sampler,
        index_buffer,
    }
}

// ── Per-atlas GPU texture ──

/// GPU texture for a Props atlas (shared across all sprites).
pub struct OpaqueAtlas {
    texture_view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

/// Load and upload the Props_Generic_Sheet_01 atlas as a raw wgpu texture.
pub fn load_props_atlas(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<OpaqueAtlas> {
    let data = crate::data::assets::read_asset("sprites/Props_Generic_Sheet_01.png")
        .or_else(|| crate::data::assets::read_asset("props/Props_Generic_Sheet_01.png"))?;
    let img = image::load_from_memory(&data).ok()?.to_rgba8();
    let width = img.width();
    let height = img.height();
    let pixels = img.into_raw();

    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("opaque_atlas_tex"),
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
        &pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        size,
    );
    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    Some(OpaqueAtlas {
        texture_view,
        width,
        height,
    })
}

// ── Per-level sprite batch (per-sprite uniform buffers + bind groups) ──

/// Per-sprite GPU resources (uniform buffer + bind group).
struct OpaqueSpriteGpu {
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

/// GPU resources for all opaque sprites in the current level.
pub struct OpaqueSpriteBatch {
    vertex_buffer: wgpu::Buffer,
    sprites: Vec<OpaqueSpriteGpu>,
}

/// Build world-space quad vertices for a Props sprite.
/// Geometry parameters for building an opaque sprite quad.
pub struct QuadGeometry {
    pub cx: f32,
    pub cy: f32,
    pub half_w: f32,
    pub half_h: f32,
    pub rotation: f32,
    pub scale_x: f32,
    pub scale_y: f32,
}

pub fn build_quad(
    geom: QuadGeometry,
    uv: &UvRect,
    atlas_w: f32,
    atlas_h: f32,
) -> [OpaqueVertex; 4] {
    let QuadGeometry {
        cx,
        cy,
        half_w,
        half_h,
        rotation,
        scale_x,
        scale_y,
    } = geom;
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
    let (u0, u1) = if scale_x < 0.0 {
        (uv_max_u, uv_min_u)
    } else {
        (uv_min_u, uv_max_u)
    };
    let (v0, v1) = if scale_y < 0.0 {
        (uv_max_v, uv_min_v)
    } else {
        (uv_min_v, uv_max_v)
    };

    // Quad corners: TL, TR, BR, BL in world space (Y-up)
    let corners: [(f32, f32); 4] = [
        (-half_w, half_h),
        (half_w, half_h),
        (half_w, -half_h),
        (-half_w, -half_h),
    ];
    let uvs = [(u0, v0), (u1, v0), (u1, v1), (u0, v1)];

    if rotation.abs() > 0.001 {
        let cos_r = rotation.cos();
        let sin_r = rotation.sin();
        [
            OpaqueVertex {
                pos: [
                    cx + corners[0].0 * cos_r - corners[0].1 * sin_r,
                    cy + corners[0].0 * sin_r + corners[0].1 * cos_r,
                ],
                uv: [uvs[0].0, uvs[0].1],
            },
            OpaqueVertex {
                pos: [
                    cx + corners[1].0 * cos_r - corners[1].1 * sin_r,
                    cy + corners[1].0 * sin_r + corners[1].1 * cos_r,
                ],
                uv: [uvs[1].0, uvs[1].1],
            },
            OpaqueVertex {
                pos: [
                    cx + corners[2].0 * cos_r - corners[2].1 * sin_r,
                    cy + corners[2].0 * sin_r + corners[2].1 * cos_r,
                ],
                uv: [uvs[2].0, uvs[2].1],
            },
            OpaqueVertex {
                pos: [
                    cx + corners[3].0 * cos_r - corners[3].1 * sin_r,
                    cy + corners[3].0 * sin_r + corners[3].1 * cos_r,
                ],
                uv: [uvs[3].0, uvs[3].1],
            },
        ]
    } else {
        [
            OpaqueVertex {
                pos: [cx - half_w, cy + half_h],
                uv: [u0, v0],
            },
            OpaqueVertex {
                pos: [cx + half_w, cy + half_h],
                uv: [u1, v0],
            },
            OpaqueVertex {
                pos: [cx + half_w, cy - half_h],
                uv: [u1, v1],
            },
            OpaqueVertex {
                pos: [cx - half_w, cy - half_h],
                uv: [u0, v1],
            },
        ]
    }
}

/// Upload sprite quads and create per-sprite GPU resources.
pub fn build_opaque_sprites(
    device: &wgpu::Device,
    resources: &OpaqueResources,
    atlas: &OpaqueAtlas,
    vertices: &[OpaqueVertex],
) -> OpaqueSpriteBatch {
    use wgpu::util::DeviceExt;
    let sprite_count = vertices.len() / 4;
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("opaque_sprites_vbo"),
        contents: bytemuck::cast_slice(vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let default_uniforms = Uniforms {
        screen_size: [1.0, 1.0],
        camera_center: [0.0, 0.0],
        zoom: 40.0,
        y_offset: 0.0,
        _pad0: 0.0,
        _pad1: 0.0,
        tint_color: [1.0, 1.0, 1.0, 1.0],
    };

    let mut sprites = Vec::with_capacity(sprite_count);
    for i in 0..sprite_count {
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("opaque_u_{}", i)),
            contents: bytemuck::bytes_of(&default_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("opaque_bg_{}", i)),
            layout: &resources.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas.texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&resources.sampler),
                },
            ],
        });
        sprites.push(OpaqueSpriteGpu {
            uniform_buffer,
            bind_group,
        });
    }

    OpaqueSpriteBatch {
        vertex_buffer,
        sprites,
    }
}

// ── Batched paint callback ──

/// A single draw within a batched opaque callback.
pub struct OpaqueBatchDraw {
    pub sprite_index: u32,
    pub cam_x: f32,
    pub cam_y: f32,
    pub y_offset: f32,
}

struct OpaqueBatchPaintCallback {
    resources: Arc<OpaqueResources>,
    batch: Arc<OpaqueSpriteBatch>,
    screen_w: f32,
    screen_h: f32,
    zoom: f32,
    tint_color: [f32; 4],
    draws: Vec<OpaqueBatchDraw>,
}

impl egui_wgpu::CallbackTrait for OpaqueBatchPaintCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        _callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        for draw in &self.draws {
            let uniforms = Uniforms {
                screen_size: [self.screen_w, self.screen_h],
                camera_center: [draw.cam_x, draw.cam_y],
                zoom: self.zoom,
                y_offset: draw.y_offset,
                _pad0: 0.0,
                _pad1: 0.0,
                tint_color: self.tint_color,
            };
            let sprite = &self.batch.sprites[draw.sprite_index as usize];
            queue.write_buffer(&sprite.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
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
        render_pass.set_vertex_buffer(0, self.batch.vertex_buffer.slice(..));
        render_pass.set_index_buffer(
            self.resources.index_buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        for draw in &self.draws {
            let sprite = &self.batch.sprites[draw.sprite_index as usize];
            render_pass.set_bind_group(0, &sprite.bind_group, &[]);
            render_pass.draw_indexed(0..6, (draw.sprite_index * 4) as i32, 0..1);
        }
    }
}

/// Build a single batched paint callback for multiple opaque sprites.
///
/// All opaque sprites draw in one render pass, avoiding per-sprite render pass
/// switches that cause frame drops in large levels.
/// Parameters for constructing an opaque batch draw call.
pub struct OpaqueBatchParams {
    pub screen_w: f32,
    pub screen_h: f32,
    pub zoom: f32,
    pub tint_color: [f32; 4],
}

pub fn make_opaque_batch_callback(
    clip_rect: egui::Rect,
    resources: Arc<OpaqueResources>,
    batch: Arc<OpaqueSpriteBatch>,
    params: OpaqueBatchParams,
    draws: Vec<OpaqueBatchDraw>,
) -> egui::Shape {
    let cb = OpaqueBatchPaintCallback {
        resources,
        batch,
        screen_w: params.screen_w,
        screen_h: params.screen_h,
        zoom: params.zoom,
        tint_color: params.tint_color,
        draws,
    };
    egui::Shape::Callback(egui_wgpu::Callback::new_paint_callback(clip_rect, cb))
}
