// Port of Unity shader "Depth Mask/MaskOverlay".
// Blend SrcAlpha OneMinusSrcAlpha, SrcAlpha OneMinusSrcAlpha
// ZWrite Off, Fog Off

struct Uniforms {
    object_to_clip: mat4x4<f32>,
    normal_matrix: mat3x3<f32>,
    _pad0: f32,
    main_tex_st: vec4<f32>,
    color: vec4<f32>,
    ambient_color: vec4<f32>,
    light_position: array<vec4<f32>, 8>,
    light_color: array<vec4<f32>, 8>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var main_tex: texture_2d<f32>;
@group(0) @binding(2) var main_sampler: sampler;

struct VertexInput {
    @location(0) position: vec4<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

fn transform_tex(uv: vec2<f32>, st: vec4<f32>) -> vec2<f32> {
    return uv * st.xy + st.zw;
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let eye_normal = normalize(u.normal_matrix * in.normal);
    var lit_rgb = u.color.xyz * u.ambient_color.xyz;
    for (var i: u32 = 0u; i < 8u; i = i + 1u) {
        let dir_to_light = u.light_position[i].xyz;
        let ndotl = max(dot(eye_normal, dir_to_light), 0.0);
        let contrib = min(((ndotl * u.color.xyz) * u.light_color[i].xyz) * 0.5, vec3<f32>(1.0));
        lit_rgb = lit_rgb + contrib;
    }

    out.position = u.object_to_clip * in.position;
    out.uv = transform_tex(in.uv, u.main_tex_st);
    out.color = clamp(vec4<f32>(lit_rgb, u.color.w), vec4<f32>(0.0), vec4<f32>(1.0));
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex = textureSample(main_tex, main_sampler, in.uv);
    return vec4<f32>((tex * in.color * 2.0).xyz, tex.w * in.color.w);
}// Port of Unity shader "Depth Mask/MaskOverlay".
// Pipeline state such as Blend/ZWrite remains a runtime responsibility.

struct Uniforms {
    object_to_clip: mat4x4<f32>,
    main_tex_st: vec4<f32>,
    color: vec4<f32>,
    ambient: vec4<f32>,
    normal_matrix: mat3x3<f32>,
    _pad0: f32,
    light_position: array<vec4<f32>, 8>,
    light_color: array<vec4<f32>, 8>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var main_tex: texture_2d<f32>;
@group(0) @binding(2) var main_sampler: sampler;

struct VIn {
    @location(0) position: vec4<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

fn transform_tex(uv: vec2<f32>, st: vec4<f32>) -> vec2<f32> {
    return uv * st.xy + st.zw;
}

@vertex
fn vs_main(in: VIn) -> VOut {
    var out: VOut;
    let eye_normal = normalize(u.normal_matrix * in.normal);
    var lit = u.color.xyz * u.ambient.xyz;
    for (var i: i32 = 0; i < 8; i = i + 1) {
        let dir_to_light = u.light_position[i].xyz;
        let lambert = max(dot(eye_normal, dir_to_light), 0.0);
        let contrib = min(((lambert * u.color.xyz) * u.light_color[i].xyz) * 0.5, vec3<f32>(1.0));
        lit = lit + contrib;
    }

    out.position = u.object_to_clip * in.position;
    out.uv = transform_tex(in.uv, u.main_tex_st);
    out.color = clamp(vec4<f32>(lit, u.color.w), vec4<f32>(0.0), vec4<f32>(1.0));
    return out;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let texel = textureSample(main_tex, main_sampler, in.uv);
    let rgb = (texel * in.color * 2.0).xyz;
    let a = texel.a * in.color.a;
    return vec4<f32>(rgb, a);
}