// Port of Unity _Custom/Unlit_Color_Geometry (used for terrain fill)
// Original: return tex2D(_MainTex, i.texcoord) * _Color;
// Blend Off, ZWrite Off, Cull Off
//
// UVs tile at 5x5 world units with wrap=Repeat.

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
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VIn) -> VOut {
    var out: VOut;
    let ndc = (in.position - u.camera_center) * u.zoom / (u.screen_size * 0.5);
    out.position = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let c = textureSample(main_tex, main_sampler, in.uv) * u.tint_color;
    if (c.a < 0.004) { discard; }
    let a = select(c.a, 1.0, c.a >= 0.784);
    return vec4<f32>(c.rgb * a, a);
}