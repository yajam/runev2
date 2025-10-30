//! engine-shaders: WGSL shader sources and helpers.

/// Common WGSL snippet shared across shaders.
pub const COMMON_WGSL: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};
"#;

/// Solid color pipeline for macOS: un-premultiply for straight alpha blending
pub const SOLID_WGSL_MACOS: &str = r#"
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
    let ndc = vec2<f32>(in_pos.x * vp.scale.x + vp.translate.x,
                        in_pos.y * vp.scale.y + vp.translate.y);
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    out.color = in_color; // premultiplied linear color
    return out;
}

@fragment
fn fs_main(inp: VsOut) -> @location(0) vec4<f32> {
    // Un-premultiply for straight alpha blending on Metal
    let alpha = inp.color.a;
    if (alpha > 0.001) {
        return vec4<f32>(inp.color.rgb / alpha, alpha);
    }
    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
}
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

/// Fast blit shader for copying intermediate texture to surface (no filtering, nearest neighbor).
/// This is optimized for the resize use case where we want the fastest possible copy.
pub const BLIT_WGSL: &str = r#"
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
    // Map NDC to UV with correct orientation without needing a fragment flip
    // (-1,-1) -> (0,1), (3,-1) -> (2,1), (-1,3) -> (0,-1)
    var uv = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(2.0, 1.0),
        vec2<f32>(0.0, -1.0),
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
    return textureSample(in_tex, in_smp, inp.uv);
}
"#;

/// Background fill (solid or linear gradient) drawn via fullscreen triangle.
pub const BACKGROUND_WGSL: &str = r#"
const MAX_STOPS: u32 = 8u;

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

// Packed to 16-byte boundaries to avoid platform layout mismatches.
struct BgUniform {
    start_end: vec4<f32>,                // start.xy, end.xy
    center_radius_stop: vec4<f32>,       // center.xy, radius, stop_count (f32)
    flags: vec4<f32>,                    // x: mode(0/1/2), y: debug(0/1), z: aspect_ratio, w: unused
};

struct Stop { 
    pos: f32, 
    pad0: f32,
    pad1: f32, 
    pad2: f32,
    color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> bg: BgUniform;
@group(0) @binding(1) var<uniform> stops: array<Stop, 8>;

fn eval_stops(t: f32) -> vec4<f32> {
    let stop_count: u32 = u32(bg.center_radius_stop.w + 0.5);
    
    // Handle edge cases
    if (stop_count == 0u) { 
        return vec4<f32>(1.0, 0.0, 1.0, 1.0); // Magenta for error
    }
    if (stop_count == 1u) { 
        return stops[0u].color; 
    }
    
    // Clamp t to valid range
    let t_clamped = clamp(t, 0.0, 1.0);
    
    // Before first stop
    if (t_clamped <= stops[0u].pos) { 
        return stops[0u].color; 
    }
    
    // After last stop
    let last_idx = stop_count - 1u;
    if (t_clamped >= stops[last_idx].pos) { 
        return stops[last_idx].color; 
    }
    
    // Between stops - find the right interval
    for (var i: u32 = 0u; i < last_idx; i = i + 1u) {
        let curr_stop = stops[i];
        let next_stop = stops[i + 1u];
        
        if (t_clamped >= curr_stop.pos && t_clamped <= next_stop.pos) {
            let range = next_stop.pos - curr_stop.pos;
            if (range < 1e-6) {
                // Stops are at same position, return current color
                return curr_stop.color;
            }
            let local_t = (t_clamped - curr_stop.pos) / range;
            return mix(curr_stop.color, next_stop.color, local_t);
        }
    }
    
    // Fallback - should never reach here
    return vec4<f32>(1.0, 1.0, 0.0, 1.0); // Yellow for error
}

@fragment
fn fs_main(inp: VsOut) -> @location(0) vec4<f32> {
    // Normalize UVs to [0,1]
    let uv01 = inp.uv * 0.5;
    let start = bg.start_end.xy;
    let end   = bg.start_end.zw;
    let center = bg.center_radius_stop.xy;
    let radius = bg.center_radius_stop.z;
    let stop_count = u32(bg.center_radius_stop.w + 0.5);
    let mode = u32(bg.flags.x + 0.5);
    let debug = u32(bg.flags.y + 0.5);
    let aspect = bg.flags.z; // width / height

    if (mode == 0u) { return stops[0u].color; }
    if (mode == 1u) {
        let dir = end - start;
        let denom = max(1e-6, dot(dir, dir));
        let t = clamp(dot(uv01 - start, dir) / denom, 0.0, 1.0);
        return eval_stops(t);
    }
    // Radial gradient mode (mode == 2)
    // Aspect-correct radial distance so rings remain circular in screen space.
    // We normalize distances by the smaller screen dimension, so scale the
    // larger axis delta accordingly.
    let dx0 = uv01.x - center.x;
    let dy0 = uv01.y - center.y;
    var d: f32;
    if (aspect >= 1.0) {
        // width >= height: scale X by aspect (W/H)
        let dx = dx0 * aspect;
        d = sqrt(dx * dx + dy0 * dy0);
    } else {
        // height > width: scale Y by 1/aspect (H/W)
        let dy = dy0 / max(1e-6, aspect);
        d = sqrt(dx0 * dx0 + dy * dy);
    }
    let t = clamp(d / max(1e-6, radius), 0.0, 1.0);
    if (debug == 1u) {
        // Debug: show t value as grayscale
        return vec4<f32>(t, t, t, 1.0);
    }
    return eval_stops(t);
} 
"#;

/// Separable Gaussian blur for single-channel mask (R channel). Output is written to the target
/// format; when using `R8Unorm`, only the R component is used.
pub const SHADOW_BLUR_WGSL: &str = r#"
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

struct BlurParams {
    dir: vec2<f32>,      // direction (1,0) or (0,1)
    texel: vec2<f32>,    // 1/width, 1/height
    sigma: f32,          // blur sigma (radius ~ 3*sigma)
    _pad: f32,
};
@group(0) @binding(2) var<uniform> params: BlurParams;

fn gauss(w: f32, s: f32) -> f32 { return exp(-0.5 * (w*w) / max(1e-6, s*s)); }

@fragment
fn fs_main(inp: VsOut) -> @location(0) vec4<f32> {
    // Fixed radius based on sigma
    let sigma = max(0.25, params.sigma);
    // Use a slightly wider kernel to better match CSS box-shadow tails
    // and avoid a "band" look at modest blur values.
    let r = i32(clamp(ceil(6.0 * sigma), 1.0, 64.0));
    var acc: f32 = 0.0;
    var norm: f32 = 0.0;
    for (var i: i32 = -r; i <= r; i = i + 1) {
        let fi = f32(i);
        let w = gauss(fi, sigma);
        let ofs = params.dir * params.texel * fi;
        let c = textureSample(in_tex, in_smp, inp.uv + ofs).r;
        acc = acc + c * w;
        norm = norm + w;
    }
    let v = acc / max(1e-6, norm);
    return vec4<f32>(v, v, v, v);
}
"#;

/// Composite blurred mask tinted with a premultiplied color onto the target.
pub const SHADOW_COMPOSITE_WGSL: &str = r#"
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

@group(0) @binding(0) var mask_tex: texture_2d<f32>;
@group(0) @binding(1) var mask_smp: sampler;

struct ShadowColor {
    color: vec4<f32>, // premultiplied linear RGBA
};
@group(0) @binding(2) var<uniform> sc: ShadowColor;

@fragment
fn fs_main(inp: VsOut) -> @location(0) vec4<f32> {
    // Match compositor orientation: flip V when sampling render-targets as textures
    let uv = vec2<f32>(inp.uv.x, 1.0 - inp.uv.y);
    let a = textureSample(mask_tex, mask_smp, uv).r;
    return vec4<f32>(sc.color.rgb * a, sc.color.a * a);
} 
"#;

/// Text rendering shader: samples an RGB coverage mask (subpixel AA) and tints with a
/// premultiplied linear text color. The output is premultiplied.
///
/// Bindings:
/// - @group(0) @binding(0): Viewport uniform (shared layout with solids)
/// - @group(1) @binding(0): Mask texture (Rgba8Unorm or Rgba16Unorm)
/// - @group(1) @binding(1): Sampler (nearest recommended)
/// - @group(1) @binding(2): Text color uniform (premultiplied linear RGBA)
pub const TEXT_WGSL: &str = r#"
struct ViewportUniform {
    scale: vec2<f32>,      // 2/W, -2/H
    translate: vec2<f32>,  // (-1, +1)
};

@group(0) @binding(0) var<uniform> vp: ViewportUniform;

struct VsIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(inp: VsIn) -> VsOut {
    var out: VsOut;
    let ndc = vec2<f32>(inp.pos.x * vp.scale.x + vp.translate.x,
                        inp.pos.y * vp.scale.y + vp.translate.y);
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = inp.uv;
    return out;
}

@group(1) @binding(0) var mask_tex: texture_2d<f32>;
@group(1) @binding(1) var mask_smp: sampler;

struct TextColor { color: vec4<f32> }; // premultiplied linear RGBA
@group(1) @binding(2) var<uniform> text: TextColor;

@fragment
fn fs_main(inp: VsOut) -> @location(0) vec4<f32> {
    // Nearest sampling prevents color bleeding across subpixels
    let m = textureSample(mask_tex, mask_smp, inp.uv);
    // RGB subpixel coverage; tint the premultiplied color per channel.
    let rgb = vec3<f32>(text.color.r * m.r, text.color.g * m.g, text.color.b * m.b);
    // Derive alpha from the maximum of coverage channels, scaled by color alpha.
    let cov = max(m.r, max(m.g, m.b));
    let a = text.color.a * cov;
    return vec4<f32>(rgb, a);
} 
"#;

/// Image rendering shader: samples an sRGB texture, converts to linear automatically via
/// the sampler, and returns premultiplied color for correct blending. Intended for PNG/JPEG
/// raster images uploaded as `Rgba8UnormSrgb`.
///
/// Bindings:
/// - @group(0) @binding(0): Viewport uniform (shared layout with solids)
/// - @group(1) @binding(0): Source texture (Rgba8UnormSrgb)
/// - @group(1) @binding(1): Sampler (linear recommended)
pub const IMAGE_WGSL: &str = r#"
struct ViewportUniform {
    scale: vec2<f32>,      // 2/W, -2/H
    translate: vec2<f32>,  // (-1, +1)
};

@group(0) @binding(0) var<uniform> vp: ViewportUniform;

struct VsIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(inp: VsIn) -> VsOut {
    var out: VsOut;
    let ndc = vec2<f32>(inp.pos.x * vp.scale.x + vp.translate.x,
                        inp.pos.y * vp.scale.y + vp.translate.y);
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = inp.uv;
    return out;
}

@group(1) @binding(0) var src_tex: texture_2d<f32>;
@group(1) @binding(1) var src_smp: sampler;

@fragment
fn fs_main(inp: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(src_tex, src_smp, inp.uv);
    // Convert straight alpha to premultiplied for correct blending
    let a = c.a;
    return vec4<f32>(c.rgb * a, a);
}
"#;
