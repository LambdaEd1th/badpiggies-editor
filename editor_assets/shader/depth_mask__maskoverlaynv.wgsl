// Port of Unity shader "Depth Mask/MaskOverlayNV".
// Blend SrcColor OneMinusSrcAlpha, SrcColor OneMinusSrcAlpha
// ColorMask RGB

struct Uniforms {
    object_to_clip: mat4x4<f32>,
    projection_params: vec4<f32>,
    main_tex_st: vec4<f32>,
    color: vec4<f32>,
    radius: f32,
    softness: f32,
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
    @location(0) uv: vec2<f32>,
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
    let proj = vec2<f32>(half_clip.x, half_clip.y * u.projection_params.x);

    out.position = clip;
    out.uv = transform_tex(in.uv, u.main_tex_st);
    out.uv1 = proj + vec2<f32>(half_clip.w, half_clip.w);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let base = textureSample(main_tex, main_sampler, in.uv) * u.color;
    let centered = in.uv1 - vec2<f32>(0.5, 0.5);
    let factor = clamp((length(centered) - u.radius) / ((u.radius - u.softness) - u.radius), 0.0, 1.0);
    let smooth = factor * (factor * (3.0 - (2.0 * factor)));
    let rgb = mix(base.xyz, base.xyz * smooth, vec3<f32>(0.5, 0.5, 0.5));
    return vec4<f32>(rgb, base.w);
}// Port of Unity shader "Depth Mask/MaskOverlayNV".

struct Uniforms {
    object_to_clip: mat4x4<f32>,
    main_tex_st: vec4<f32>,
    projection_params: vec4<f32>,
    color: vec4<f32>,
    radius: f32,
    softness: f32,
    _pad0: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var main_tex: texture_2d<f32>;
@group(0) @binding(2) var main_sampler: sampler;

struct VIn {
    @location(0) position: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) projected_uv: vec2<f32>,
};

fn transform_tex(uv: vec2<f32>, st: vec4<f32>) -> vec2<f32> {
    return uv * st.xy + st.zw;
}

@vertex
fn vs_main(in: VIn) -> VOut {
    var out: VOut;
    let clip_pos = u.object_to_clip * in.position;
    let half_clip = clip_pos * 0.5;
    let projected = vec2<f32>(half_clip.x, half_clip.y * u.projection_params.x) + half_clip.w;

    out.position = clip_pos;
    out.uv = transform_tex(in.uv, u.main_tex_st);
    out.projected_uv = projected;
    return out;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let base = textureSample(main_tex, main_sampler, in.uv) * u.color;
    let centered = in.projected_uv - vec2<f32>(0.5, 0.5);
    let t = clamp((sqrt(dot(centered, centered)) - u.radius) / ((u.radius - u.softness) - u.radius), 0.0, 1.0);
    let smooth = t * (t * (3.0 - 2.0 * t));
    let rgb = mix(base.xyz, base.xyz * smooth, vec3<f32>(0.5, 0.5, 0.5));
    return vec4<f32>(rgb, base.w);
}