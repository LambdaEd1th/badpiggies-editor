//! wgpu + WGSL terrain edge shader — port of Unity e2d/Curve shader.
//!
//! Two splat textures blended via a per-node control texture.
//! Works on Metal (macOS), Vulkan (Linux/Windows), DX12, and WebGPU (WASM).

use std::sync::Arc;

use eframe::egui;
use eframe::wgpu;

// ── WGSL shader source — exact port of Unity e2d/Curve ──

const WGSL_SOURCE: &str = r#"
// Uniform buffer: camera + per-terrain parameters
struct Uniforms {
    screen_size: vec2<f32>,
    camera_center: vec2<f32>,
    zoom: f32,
    inv_control_size: f32,
    inv_control_size_half: f32,
    splat_params_x: f32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var control_tex: texture_2d<f32>;
@group(0) @binding(2) var clamp_sampler: sampler;
@group(0) @binding(3) var splat0_tex: texture_2d<f32>;
@group(0) @binding(4) var repeat_sampler: sampler;
@group(0) @binding(5) var splat1_tex: texture_2d<f32>;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,       // x = accumulated distance, y = node index
    @location(2) color: f32,           // 1.0 = outer (surface), 0.0 = inner
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) control_uv: vec2<f32>,
    @location(1) splat_uv: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Transform world → NDC
    // NDC: X -1(left)..+1(right), Y -1(bottom)..+1(top)
    // World Y-up matches NDC Y-up, so no negation needed.
    let sx = (in.position.x - u.camera_center.x) * u.zoom;
    let sy = (in.position.y - u.camera_center.y) * u.zoom;
    let ndc_x = sx / (u.screen_size.x * 0.5);
    let ndc_y = sy / (u.screen_size.y * 0.5);
    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);

    // Control texture UV: sample at texel center for this node
    out.control_uv = vec2<f32>(in.uv.y * u.inv_control_size + u.inv_control_size_half, 0.5);

    // Splat texture UV: x = horizontal tiling, y = gradient position (outer=1, inner=0)
    out.splat_uv = vec2<f32>(in.uv.x * u.splat_params_x, in.color);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let splat1 = textureSample(splat1_tex, repeat_sampler, in.splat_uv);
    // Control green channel selects splat (matches Unity "floor(tex2D(_Control, uv).y)")
    let selector = floor(textureSample(control_tex, clamp_sampler, in.control_uv).y);
    var result = textureSample(splat0_tex, repeat_sampler, in.splat_uv);
    result = vec4<f32>(
        result.xyz + (splat1.xyz - result.xyz) * selector,
        result.w,
    );
    // Premultiply alpha for egui's default blending
    result = vec4<f32>(result.xyz * result.w, result.w);
    return result;
}
"#;

// ── GPU uniform buffer layout (matches WGSL struct Uniforms) ──

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    screen_size: [f32; 2],
    camera_center: [f32; 2],
    zoom: f32,
    inv_control_size: f32,
    inv_control_size_half: f32,
    splat_params_x: f32,
}

// ── Shared pipeline resources (one per wgpu device) ──

/// Shared render pipeline, bind group layout, and samplers.
pub struct EdgeResources {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    clamp_sampler: wgpu::Sampler,
    repeat_sampler: wgpu::Sampler,
}

/// Initialize the wgpu render pipeline and shared resources.
pub fn init_edge_resources(
    device: &wgpu::Device,
    target_format: wgpu::TextureFormat,
) -> EdgeResources {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("edge_shader"),
        source: wgpu::ShaderSource::Wgsl(WGSL_SOURCE.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("edge_bind_group_layout"),
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
            // @binding(1) control texture
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
            // @binding(2) clamp sampler (for control texture)
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            // @binding(3) splat0 texture
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // @binding(4) repeat sampler (splats: repeat in U, clamp in V)
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            // @binding(5) splat1 texture
            wgpu::BindGroupLayoutEntry {
                binding: 5,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("edge_pipeline_layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("edge_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<EdgeVertex>() as u64, // 20
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
                    // @location(2) color: f32
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32,
                        offset: 16,
                        shader_location: 2,
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
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    let clamp_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("edge_clamp_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let repeat_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("edge_repeat_sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    EdgeResources {
        pipeline,
        bind_group_layout,
        clamp_sampler,
        repeat_sampler,
    }
}

// ── Per-terrain GPU resources ──

/// wgpu resources for a single terrain edge mesh.
pub struct EdgeGpuMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// Per-terrain uniform values (merged with camera each frame).
    inv_control_size: f32,
    inv_control_size_half: f32,
    splat_params_x: f32,
    /// Whether both splat textures were available.
    pub has_both_splats: bool,
    /// Whether this terrain is decorative (no collider) — renders earlier.
    pub decorative: bool,
}

/// Interleaved vertex: position (2f) + uv (2f) + color (1f) = 20 bytes.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct EdgeVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: f32,
}

/// Input mesh data for uploading a terrain edge to GPU.
pub struct EdgeMeshInput<'a> {
    pub vertices: &'a [EdgeVertex],
    pub indices: &'a [u16],
    pub control_pixels: &'a [u8],
    pub control_node_count: usize,
    pub splat0_pixels: Option<&'a [u8]>,
    pub splat0_w: u32,
    pub splat0_h: u32,
    pub splat1_pixels: Option<&'a [u8]>,
    pub splat1_w: u32,
    pub splat1_h: u32,
    pub splat_params_x: f32,
    pub decorative: bool,
}

/// Camera/screen parameters for edge paint callbacks.
pub struct EdgeCameraParams {
    pub screen_w: f32,
    pub screen_h: f32,
    pub camera_x: f32,
    pub camera_y: f32,
    pub zoom: f32,
}

/// Build GPU resources for one terrain edge.
pub fn upload_edge_mesh(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    resources: &EdgeResources,
    mesh: &EdgeMeshInput<'_>,
) -> EdgeGpuMesh {
    let EdgeMeshInput {
        vertices,
        indices,
        control_pixels,
        control_node_count,
        splat0_pixels,
        splat0_w,
        splat0_h,
        splat1_pixels,
        splat1_w,
        splat1_h,
        splat_params_x,
        decorative,
    } = *mesh;
    use wgpu::util::DeviceExt;

    // Vertex buffer
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("edge_vbo"),
        contents: bytemuck::cast_slice(vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    // Index buffer
    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("edge_ebo"),
        contents: bytemuck::cast_slice(indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    // Uniform buffer (updated each frame with camera params)
    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("edge_uniforms"),
        contents: bytemuck::bytes_of(&Uniforms {
            screen_size: [1.0, 1.0],
            camera_center: [0.0, 0.0],
            zoom: 40.0,
            inv_control_size: 0.0,
            inv_control_size_half: 0.0,
            splat_params_x: 0.0,
        }),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    // Control texture: pad to next power of 2
    let mut ctrl_size = 1u32;
    while (ctrl_size as usize) < control_node_count {
        ctrl_size *= 2;
    }
    let mut padded_ctrl = vec![0u8; (ctrl_size * 4) as usize];
    let copy_len = control_pixels.len().min(padded_ctrl.len());
    padded_ctrl[..copy_len].copy_from_slice(&control_pixels[..copy_len]);

    let control_view =
        create_rgba_texture(device, queue, &padded_ctrl, ctrl_size, 1, "edge_control");

    // Splat textures (1×1 white fallback if missing)
    let has_splat0 = splat0_pixels.is_some();
    let has_splat1 = splat1_pixels.is_some();

    let splat0_view = if let Some(px) = splat0_pixels {
        create_rgba_texture(device, queue, px, splat0_w, splat0_h, "edge_splat0")
    } else {
        create_rgba_texture(device, queue, &[255, 255, 255, 255], 1, 1, "edge_splat0_fb")
    };

    let splat1_view = if let Some(px) = splat1_pixels {
        create_rgba_texture(device, queue, px, splat1_w, splat1_h, "edge_splat1")
    } else {
        create_rgba_texture(device, queue, &[255, 255, 255, 255], 1, 1, "edge_splat1_fb")
    };

    let inv = 1.0 / ctrl_size as f32;

    // Bind group
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("edge_bind_group"),
        layout: &resources.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&control_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&resources.clamp_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&splat0_view),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::Sampler(&resources.repeat_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::TextureView(&splat1_view),
            },
        ],
    });

    EdgeGpuMesh {
        vertex_buffer,
        index_buffer,
        index_count: indices.len() as u32,
        uniform_buffer,
        bind_group,
        inv_control_size: inv,
        inv_control_size_half: 0.5 * inv,
        splat_params_x,
        has_both_splats: has_splat0 && has_splat1,
        decorative,
    }
}

/// Create an RGBA texture and return its view.
fn create_rgba_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pixels: &[u8],
    width: u32,
    height: u32,
    label: &str,
) -> wgpu::TextureView {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
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
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

// ── Paint callback (egui_wgpu integration) ──

/// PaintCallback implementation for rendering terrain edges via wgpu.
struct EdgePaintCallback {
    resources: Arc<EdgeResources>,
    meshes: Arc<Vec<EdgeGpuMesh>>,
    screen_w: f32,
    screen_h: f32,
    camera_x: f32,
    camera_y: f32,
    zoom: f32,
    /// If `Some(true)`, only decorative; `Some(false)` only collider; `None` all.
    decorative_filter: Option<bool>,
    /// If `Some(idx)`, only render the mesh at this index (for per-terrain interleaving).
    target_mesh_index: Option<usize>,
}

impl egui_wgpu::CallbackTrait for EdgePaintCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        _callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        // Update each mesh's uniform buffer with current camera + per-terrain params
        for (i, mesh) in self.meshes.iter().enumerate() {
            if !mesh.has_both_splats {
                continue;
            }
            if let Some(want) = self.decorative_filter
                && mesh.decorative != want
            {
                continue;
            }
            if let Some(target) = self.target_mesh_index
                && i != target
            {
                continue;
            }
            let uniforms = Uniforms {
                screen_size: [self.screen_w, self.screen_h],
                camera_center: [self.camera_x, self.camera_y],
                zoom: self.zoom,
                inv_control_size: mesh.inv_control_size,
                inv_control_size_half: mesh.inv_control_size_half,
                splat_params_x: mesh.splat_params_x,
            };
            queue.write_buffer(&mesh.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        _callback_resources: &egui_wgpu::CallbackResources,
    ) {
        render_pass.set_pipeline(&self.resources.pipeline);

        for (i, mesh) in self.meshes.iter().enumerate() {
            if !mesh.has_both_splats {
                continue;
            }
            if let Some(want) = self.decorative_filter
                && mesh.decorative != want
            {
                continue;
            }
            if let Some(target) = self.target_mesh_index
                && i != target
            {
                continue;
            }
            render_pass.set_bind_group(0, &mesh.bind_group, &[]);
            render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }
}

/// Build a PaintCallback shape for rendering a single terrain edge mesh by index.
pub fn make_single_edge_paint_callback(
    clip_rect: egui::Rect,
    resources: Arc<EdgeResources>,
    meshes: Arc<Vec<EdgeGpuMesh>>,
    cam: EdgeCameraParams,
    mesh_index: usize,
) -> egui::Shape {
    let cb = EdgePaintCallback {
        resources,
        meshes,
        screen_w: cam.screen_w,
        screen_h: cam.screen_h,
        camera_x: cam.camera_x,
        camera_y: cam.camera_y,
        zoom: cam.zoom,
        decorative_filter: None,
        target_mesh_index: Some(mesh_index),
    };

    egui::Shape::Callback(egui_wgpu::Callback::new_paint_callback(clip_rect, cb))
}
