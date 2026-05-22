// Port of Unity shader "_Custom/DailyChallengeMask".

struct Uniforms {
    object_to_clip: mat4x4<f32>,
    main_tex_st: vec4<f32>,
    grayness: f32,
    _pad0: vec3<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var main_tex: texture_2d<f32>;
@group(0) @binding(2) var mask_tex: texture_2d<f32>;
@group(0) @binding(3) var layer_tex: texture_2d<f32>;
@group(0) @binding(4) var linear_sampler: sampler;

struct VertexInput {
    @location(0) position: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

fn transform_tex(uv: vec2<f32>, st: vec4<f32>) -> vec2<f32> {
    return uv * st.xy + st.zw;
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = u.object_to_clip * in.position;
    out.uv = transform_tex(in.uv, u.main_tex_st);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let base = textureSample(main_tex, linear_sampler, in.uv);
    let layer = textureSample(layer_tex, linear_sampler, in.uv);
    let mask = textureSample(mask_tex, linear_sampler, in.uv);
    let lum = 0.2989 * base.x + 0.587 * base.y + 0.114 * base.z;
    let gray_rgb = base.xyz * (1.0 - u.grayness) + vec3<f32>(lum, lum, lum) * u.grayness;
    let layer_mix = gray_rgb * (1.0 - layer.w) + layer.xyz * layer.w;
    return vec4<f32>(layer_mix, max(mask.w, layer.w));
}// Port of Unity shader "_Custom/DailyChallengeMask".

struct Uniforms {
    object_to_clip: mat4x4<f32>,
    main_tex_st: vec4<f32>,
    grayness: f32,
    _pad0: vec3<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var main_tex: texture_2d<f32>;
@group(0) @binding(2) var main_sampler: sampler;
@group(0) @binding(3) var mask_tex: texture_2d<f32>;
@group(0) @binding(4) var mask_sampler: sampler;
@group(0) @binding(5) var layer_tex: texture_2d<f32>;
@group(0) @binding(6) var layer_sampler: sampler;

struct VIn {
    @location(0) position: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

fn transform_tex(uv: vec2<f32>, st: vec4<f32>) -> vec2<f32> {
    return uv * st.xy + st.zw;
}

@vertex
fn vs_main(in: VIn) -> VOut {
    var out: VOut;
    out.position = u.object_to_clip * in.position;
    out.uv = transform_tex(in.uv, u.main_tex_st);
    return out;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let base = textureSample(main_tex, main_sampler, in.uv);
    let layer = textureSample(layer_tex, layer_sampler, in.uv);
    let lum = 0.2989 * base.x + 0.587 * base.y + 0.114 * base.z;
    let gray_rgb = vec3<f32>(lum, lum, lum);
    let mixed_rgb = base.xyz * (1.0 - u.grayness) + gray_rgb * u.grayness;
    let masked_alpha = textureSample(mask_tex, mask_sampler, in.uv).w;
    let composed_rgb = mixed_rgb * (1.0 - layer.w) + layer.xyz * layer.w;
    let composed_alpha = max(masked_alpha, layer.w);
    return vec4<f32>(composed_rgb, composed_alpha);
}