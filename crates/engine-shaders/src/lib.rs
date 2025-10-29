//! engine-shaders: WGSL shader sources and helpers.

/// Common WGSL snippet shared across shaders.
pub const COMMON_WGSL: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};
"#;

/// Solid color pipeline: vertices carry color in linear space (premultiplied alpha).
pub const SOLID_WGSL: &str = r#"
struct ViewportUniform {
    scale: vec2<f32>,      // 2/W, -2/H
    translate: vec2<f32>,  // (-1, +1)
};

@group(0) @binding(0) var<uniform> vp: ViewportUniform;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(@location(0) in_pos: vec2<f32>, @location(1) in_color: vec4<f32>) -> VsOut {
    var out: VsOut;
    // in_pos is in local/layout pixel coordinates (y-down)
    let ndc = vec2<f32>(in_pos.x * vp.scale.x + vp.translate.x,
                        in_pos.y * vp.scale.y + vp.translate.y);
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    out.color = in_color; // premultiplied linear color
    return out;
}

@fragment
fn fs_main(inp: VsOut) -> @location(0) vec4<f32> {
    return inp.color;
}
"#;

/// Gradient utilities (structure only; evaluated in linear space)
pub const GRADIENT_WGSL: &str = r#"
struct Stop { pos: f32, color: vec4<f32> }; // premultiplied linear RGBA

fn eval_linear_gradient(stops: array<Stop>, t: f32) -> vec4<f32> {
    // Naive two-stop mix for illustration; full implementation will handle N stops.
    let clamped = clamp(t, 0.0, 1.0);
    // Assume two stops for now
    let a = stops[0];
    let b = stops[1];
    let tt = (clamped - a.pos) / max(1e-6, (b.pos - a.pos));
    return mix(a.color, b.color, clamp(tt, 0.0, 1.0));
}
"#;

/// Fullscreen textured compositor (premultiplied alpha expected).
pub const COMPOSITOR_WGSL: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    var uv = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(2.0, 0.0),
        vec2<f32>(0.0, 2.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(pos[vi], 0.0, 1.0);
    out.uv = uv[vi];
    return out;
}

@group(0) @binding(0) var in_tex: texture_2d<f32>;
@group(0) @binding(1) var in_smp: sampler;

@fragment
fn fs_main(inp: VsOut) -> @location(0) vec4<f32> {
    // Flip V to account for render-target vs texture sampling coord systems
    let uv = vec2<f32>(inp.uv.x, 1.0 - inp.uv.y);
    let c = textureSample(in_tex, in_smp, uv);
    return c; // premultiplied color flows through
}
"#;

/// Background fill (solid or linear gradient) drawn via fullscreen triangle.
pub const BACKGROUND_WGSL: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    var uv = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(2.0, 0.0),
        vec2<f32>(0.0, 2.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(pos[vi], 0.0, 1.0);
    out.uv = uv[vi];
    return out;
}

struct BgUniform {
    start: vec2<f32>,
    end: vec2<f32>,
    color_a: vec4<f32>, // premultiplied linear
    color_b: vec4<f32>,
    pos_a: f32,
    pos_b: f32,
    mode: u32, // 0 solid, 1 linear
    _pad: u32,
};

@group(0) @binding(0) var<uniform> bg: BgUniform;

@fragment
fn fs_main(inp: VsOut) -> @location(0) vec4<f32> {
    if (bg.mode == 0u) {
        return bg.color_a;
    }
    // Linear gradient from bg.start to bg.end in UV space.
    let dir = bg.end - bg.start;
    let denom = max(1e-6, dot(dir, dir));
    let t = clamp(dot(inp.uv - bg.start, dir) / denom, 0.0, 1.0);
    // Remap across two stops
    let tt = clamp((t - bg.pos_a) / max(1e-6, (bg.pos_b - bg.pos_a)), 0.0, 1.0);
    return mix(bg.color_a, bg.color_b, tt);
}
"#;
