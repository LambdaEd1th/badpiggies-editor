struct Uniforms {
    screen_size: vec2<f32>,
    camera_center: vec2<f32>,
    zoom: f32,
    cutoff: f32,
    world_center: vec2<f32>,
    world_size: vec2<f32>,
    uv_min: vec2<f32>,
    uv_max: vec2<f32>,
    content_ratio_x: f32,
    _pad0: f32,
    main_tex_st: vec4<f32>,
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
    let world = u.world_center + in.position * u.world_size;
    let ndc = (world - u.camera_center) * u.zoom / (u.screen_size * 0.5);
    out.position = vec4<f32>(ndc, 0.0, 1.0);

    let mapped_x = (in.uv.x - 0.5) * u.content_ratio_x + 0.5;
    let uv = mix(u.uv_min, u.uv_max, vec2(mapped_x, in.uv.y));
    out.uv = uv * u.main_tex_st.xy + u.main_tex_st.zw;
    return out;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let alpha = u.tint_color.a * textureSample(main_tex, main_sampler, in.uv).a;
    return vec4<f32>(u.tint_color.rgb, alpha);
}