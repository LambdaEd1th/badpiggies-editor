// Port of Unity shader "_Custom/PreAlpha_Unlit_ColorTransparent_Geometry_Shiny".

struct Uniforms {
    object_to_clip: mat4x4<f32>,
    projection_params: vec4<f32>,
    main_tex_st: vec4<f32>,
    color: vec4<f32>,
    center: f32,
    scale: f32,
    _pad0: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var main_tex: texture_2d<f32>;
@group(0) @binding(2) var main_sampler: sampler;

struct VertexInput {
    @location(0) position: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv0: vec2<f32>,
    @location(1) uv1: vec2<f32>,
};

fn transform_tex(uv: vec2<f32>, st: vec4<f32>) -> vec2<f32> {
    return uv * st.xy + st.zw;
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let clip = u.object_to_clip * in.position;
    let half_clip = clip * 0.5;
    out.position = clip;
    out.uv0 = transform_tex(in.uv, u.main_tex_st);
    out.uv1 = vec2<f32>(half_clip.x, half_clip.y * u.projection_params.x) + vec2<f32>(half_clip.w, half_clip.w);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex = textureSample(main_tex, main_sampler, in.uv0);
    let shine = 1.0 - abs((in.uv1.x - u.center) * u.scale);
    return tex + clamp(shine, 0.0, 1.0) * u.color * tex.w;
}