// Port of Unity _Custom/Unlit_ColorTransparent_Geometry + Gray + Shiny variants
// mode 0 (normal):       return tex2D(_MainTex, uv) * _Color
// mode 1 (gray):         lum = dot(tex.rgb, vec3(0.2989, 0.587, 0.114));
//                        return vec4(lum,lum,lum,tex.a) * _Color
// mode 2 (shiny):        shine = 1 - abs((screen_x - _Center) * _Scale);
//                        return tex + clamp(shine,0,1) * _Color * tex.a
// mode 3 (prealpha):     same as normal, but sampled RGB is already premultiplied
//                        in the source atlas, so the fragment must not multiply
//                        RGB by alpha a second time before blending.
//
// Blend SrcAlpha OneMinusSrcAlpha, ZWrite Off, Cull Off

struct Uniforms {
    screen_size: vec2<f32>,
    camera_center: vec2<f32>,
    zoom: f32,
    rotation: f32,
    world_center: vec2<f32>,
    half_size: vec2<f32>,
    uv_min: vec2<f32>,
    uv_max: vec2<f32>,
    mode: f32,
    shine_center: f32,
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
    @location(1) screen_x: f32,
};

@vertex
fn vs_main(in: VIn) -> VOut {
    var out: VOut;
    let local = in.position * u.half_size * 2.0;

    let cos_r = cos(u.rotation);
    let sin_r = sin(u.rotation);
    let rotated = vec2<f32>(
        local.x * cos_r - local.y * sin_r,
        local.x * sin_r + local.y * cos_r,
    );

    let world = u.world_center + rotated;
    let ndc = (world - u.camera_center) * u.zoom / (u.screen_size * 0.5);
    out.position = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = mix(u.uv_min, u.uv_max, in.uv);
    out.screen_x = ndc.x * 0.5 + 0.5;
    return out;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let tex = textureSample(main_tex, main_sampler, in.uv);
    var c: vec4<f32>;
    let prealpha = u.mode > 2.5;
    var render_mode = u.mode;
    if (prealpha) {
        render_mode = u.mode - 3.0;
    }

    if (render_mode > 1.5) {
        let shine = 1.0 - abs((in.screen_x - u.shine_center) * 10.0);
        c = tex + clamp(shine, 0.0, 1.0) * u.tint_color * tex.a;
    } else if (render_mode > 0.5) {
        let lum = 0.2989 * tex.r + 0.587 * tex.g + 0.114 * tex.b;
        c = vec4<f32>(lum, lum, lum, tex.a) * u.tint_color;
    } else {
        c = tex * u.tint_color;
    }

    if (prealpha) {
        return vec4<f32>(c.rgb, c.a);
    }
    return vec4<f32>(c.rgb * c.a, c.a);
}