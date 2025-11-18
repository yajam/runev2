use anyhow::Result;
use bytemuck::{Pod, Zeroable};

use crate::allocator::{BufKey, OwnedBuffer, RenderAllocator};
use crate::display_list::{Command, DisplayList};
use crate::scene::{Brush, FillRule, Path, PathCmd, Rect, RoundedRect, Stroke, Transform2D, TextRun};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub color: [f32; 4],
    pub z_index: f32,
}

pub struct GpuScene {
    pub vertex: OwnedBuffer,
    pub index: OwnedBuffer,
    pub vertices: u32,
    pub indices: u32,
}

/// Extracted text draw from DisplayList
#[derive(Clone, Debug)]
pub struct ExtractedTextDraw {
    pub run: TextRun,
    pub z: i32,
    pub transform: Transform2D,
}

/// Extracted image draw from DisplayList (placeholder for future)
#[derive(Clone, Debug)]
pub struct ExtractedImageDraw {
    pub path: std::path::PathBuf,
    pub origin: [f32; 2],
    pub size: [f32; 2],
    pub z: i32,
    pub transform: Transform2D,
}

/// Extracted SVG draw from DisplayList (placeholder for future)
#[derive(Clone, Debug)]
pub struct ExtractedSvgDraw {
    pub path: std::path::PathBuf,
    pub origin: [f32; 2],
    pub size: [f32; 2],
    pub z: i32,
    pub transform: Transform2D,
}

/// Complete unified scene data extracted from DisplayList
pub struct UnifiedSceneData {
    pub gpu_scene: GpuScene,
    pub text_draws: Vec<ExtractedTextDraw>,
    pub image_draws: Vec<ExtractedImageDraw>,
    pub svg_draws: Vec<ExtractedSvgDraw>,
}

fn apply_transform(p: [f32; 2], t: Transform2D) -> [f32; 2] {
    let [a, b, c, d, e, f] = t.m;
    [a * p[0] + c * p[1] + e, b * p[0] + d * p[1] + f]
}

fn rect_to_verts(rect: Rect, color: [f32; 4], t: Transform2D, z: f32) -> ([Vertex; 4], [u16; 6]) {
    let x0 = rect.x;
    let y0 = rect.y;
    let x1 = rect.x + rect.w;
    let y1 = rect.y + rect.h;
    let p0 = apply_transform([x0, y0], t);
    let p1 = apply_transform([x1, y0], t);
    let p2 = apply_transform([x1, y1], t);
    let p3 = apply_transform([x0, y1], t);
    (
        [
            Vertex { pos: p0, color, z_index: z },
            Vertex { pos: p1, color, z_index: z },
            Vertex { pos: p2, color, z_index: z },
            Vertex { pos: p3, color, z_index: z },
        ],
        [0, 1, 2, 0, 2, 3],
    )
}

fn push_rect_linear_gradient(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    rect: Rect,
    stops: &[(f32, [f32; 4])],
    t: Transform2D,
    z: f32,
) {
    if stops.len() < 2 {
        return;
    }
    // ensure sorted
    let mut s = stops.to_vec();
    s.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let y0 = rect.y;
    let y1 = rect.y + rect.h;
    for pair in s.windows(2) {
        let (t0, c0) = (pair[0].0.clamp(0.0, 1.0), pair[0].1);
        let (t1, c1) = (pair[1].0.clamp(0.0, 1.0), pair[1].1);
        if (t1 - t0).abs() < 1e-6 {
            continue;
        }
        let x0 = rect.x + rect.w * t0;
        let x1 = rect.x + rect.w * t1;
        let p0 = apply_transform([x0, y0], t);
        let p1 = apply_transform([x1, y0], t);
        let p2 = apply_transform([x1, y1], t);
        let p3 = apply_transform([x0, y1], t);
        let base = vertices.len() as u16;
        vertices.extend_from_slice(&[
            Vertex { pos: p0, color: c0, z_index: z },
            Vertex { pos: p1, color: c1, z_index: z },
            Vertex { pos: p2, color: c1, z_index: z },
            Vertex { pos: p3, color: c0, z_index: z },
        ]);
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

fn push_ellipse(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    center: [f32; 2],
    radii: [f32; 2],
    color: [f32; 4],
    z: f32,
    t: Transform2D,
) {
    let segs = 64u32;
    let base = vertices.len() as u16;
    let c = apply_transform(center, t);
    vertices.push(Vertex {
        pos: c,
        color,
        z_index: z,
    });

    for i in 0..segs {
        let theta = (i as f32) / (segs as f32) * std::f32::consts::TAU;
        let p = [
            center[0] + radii[0] * theta.cos(),
            center[1] + radii[1] * theta.sin(),
        ];
        let p = apply_transform(p, t);
        vertices.push(Vertex {
            pos: p,
            color,
            z_index: z,
        });
    }
    for i in 0..segs {
        let i0 = base;
        let i1 = base + 1 + i as u16;
        let i2 = base + 1 + ((i + 1) % segs) as u16;
        indices.extend_from_slice(&[i0, i1, i2]);
    }
}

fn push_ellipse_radial_gradient(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    center: [f32; 2],
    radii: [f32; 2],
    stops: &[(f32, [f32; 4])],
    z: f32,
    t: Transform2D,
) {
    if stops.len() < 2 {
        return;
    }
    let mut s = stops.to_vec();
    s.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let segs = 64u32;
    let base_center = vertices.len() as u16;
    // Center vertex with first stop color
    let cpos = apply_transform(center, t);
    vertices.push(Vertex {
        pos: cpos,
        color: s[0].1,
        z_index: z,
    });

    // First ring
    let mut prev_ring_start = vertices.len() as u16;
    let prev_color = s[0].1;
    let prev_t0 = s[0].0.clamp(0.0, 1.0);
    let prev_t = if prev_t0 <= 0.0 { 0.0 } else { prev_t0 };
    for i in 0..segs {
        let theta = (i as f32) / (segs as f32) * std::f32::consts::TAU;
        let p = [
            center[0] + radii[0] * prev_t * theta.cos(),
            center[1] + radii[1] * prev_t * theta.sin(),
        ];
        let p = apply_transform(p, t);
        vertices.push(Vertex {
            pos: p,
            color: prev_color,
            z_index: z,
        });
    }
    // Connect center to first ring if needed
    if prev_t == 0.0 {
        for i in 0..segs {
            let i1 = base_center;
            let i2 = prev_ring_start + i as u16 + 1;
            let i3 = prev_ring_start + ((i + 1) % segs) as u16 + 1;
            indices.extend_from_slice(&[i1, i2, i3]);
        }
    }

    for si in 1..s.len() {
        let (tcur, ccur) = (s[si].0.clamp(0.0, 1.0), s[si].1);
        let ring_start = vertices.len() as u16;
        for i in 0..segs {
            let theta = (i as f32) / (segs as f32) * std::f32::consts::TAU;
            let p = [
                center[0] + radii[0] * tcur * theta.cos(),
                center[1] + radii[1] * tcur * theta.sin(),
            ];
            let p = apply_transform(p, t);
            vertices.push(Vertex {
                pos: p,
                color: ccur,
                z_index: z,
            });
        }
        // stitch prev ring to current ring
        for i in 0..segs {
            let a0 = prev_ring_start + i as u16;
            let a1 = prev_ring_start + ((i + 1) % segs) as u16;
            let b0 = ring_start + i as u16;
            let b1 = ring_start + ((i + 1) % segs) as u16;
            indices.extend_from_slice(&[a0, b0, b1, a0, b1, a1]);
        }
        prev_ring_start = ring_start;
    }
}

fn tessellate_path_fill(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    path: &Path,
    color: [f32; 4],
    z: f32,
    t: Transform2D,
) {
    use lyon_geom::point;
    use lyon_path::Path as LyonPath;
    use lyon_tessellation::{
        BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers,
    };

    // Build lyon path
    let mut builder = lyon_path::Path::builder();
    let mut started = false;
    for cmd in &path.cmds {
        match *cmd {
            PathCmd::MoveTo(p) => {
                if started {
                    builder.end(false);
                }
                builder.begin(point(p[0], p[1]));
                started = true;
            }
            PathCmd::LineTo(p) => {
                if !started {
                    builder.begin(point(p[0], p[1]));
                    started = true;
                } else {
                    builder.line_to(point(p[0], p[1]));
                }
            }
            PathCmd::QuadTo(c, p) => {
                builder.quadratic_bezier_to(point(c[0], c[1]), point(p[0], p[1]));
            }
            PathCmd::CubicTo(c1, c2, p) => {
                builder.cubic_bezier_to(
                    point(c1[0], c1[1]),
                    point(c2[0], c2[1]),
                    point(p[0], p[1]),
                );
            }
            PathCmd::Close => {
                builder.end(true);
                started = false;
            }
        }
    }
    // If the last sub-path wasn't explicitly closed, end it as open.
    if started {
        builder.end(false);
    }
    let lyon_path: LyonPath = builder.build();
    let mut tess = FillTessellator::new();
    // Configurable tessellation tolerance via LYON_TOLERANCE (default 0.1)
    let tol = std::env::var("LYON_TOLERANCE")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.1);
    let base_opts = FillOptions::default().with_tolerance(tol);
    let options = match path.fill_rule {
        FillRule::NonZero => base_opts.with_fill_rule(lyon_tessellation::FillRule::NonZero),
        FillRule::EvenOdd => base_opts.with_fill_rule(lyon_tessellation::FillRule::EvenOdd),
    };
    let mut geom: VertexBuffers<[f32; 2], u16> = VertexBuffers::new();
    let result = tess.tessellate_path(
        lyon_path.as_slice(),
        &options,
        &mut BuffersBuilder::new(&mut geom, |fv: FillVertex| {
            let p = fv.position();
            [p.x, p.y]
        }),
    );
    if result.is_err() {
        return;
    }
    // Transform and append
    let base = vertices.len() as u16;
    for p in &geom.vertices {
        let tp = apply_transform(*p, t);
        vertices.push(Vertex {
            pos: tp,
            color,
            z_index: z,
        });
    }
    indices.extend(geom.indices.iter().map(|i| base + *i));
}

fn tessellate_path_stroke(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    path: &Path,
    stroke: Stroke,
    color: [f32; 4],
    z: f32,
    t: Transform2D,
) {
    use lyon_geom::point;
    use lyon_path::Path as LyonPath;
    use lyon_tessellation::{
        BuffersBuilder, LineCap, LineJoin, StrokeOptions, StrokeTessellator, StrokeVertex,
        VertexBuffers,
    };

    // Build lyon path
    let mut builder = lyon_path::Path::builder();
    let mut started = false;
    for cmd in &path.cmds {
        match *cmd {
            PathCmd::MoveTo(p) => {
                if started {
                    builder.end(false);
                }
                builder.begin(point(p[0], p[1]));
                started = true;
            }
            PathCmd::LineTo(p) => {
                if !started {
                    builder.begin(point(p[0], p[1]));
                    started = true;
                } else {
                    builder.line_to(point(p[0], p[1]));
                }
            }
            PathCmd::QuadTo(c, p) => {
                builder.quadratic_bezier_to(point(c[0], c[1]), point(p[0], p[1]));
            }
            PathCmd::CubicTo(c1, c2, p) => {
                builder.cubic_bezier_to(
                    point(c1[0], c1[1]),
                    point(c2[0], c2[1]),
                    point(p[0], p[1]),
                );
            }
            PathCmd::Close => {
                builder.end(true);
                started = false;
            }
        }
    }
    // End any open sub-path
    if started {
        builder.end(false);
    }
    let lyon_path: LyonPath = builder.build();

    let mut tess = StrokeTessellator::new();
    let tol = std::env::var("LYON_TOLERANCE")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.1);
    let options = StrokeOptions::default()
        .with_line_width(stroke.width.max(0.0))
        .with_tolerance(tol)
        .with_line_join(LineJoin::Round)
        .with_start_cap(LineCap::Round)
        .with_end_cap(LineCap::Round);
    let mut geom: VertexBuffers<[f32; 2], u16> = VertexBuffers::new();
    let result = tess.tessellate_path(
        lyon_path.as_slice(),
        &options,
        &mut BuffersBuilder::new(&mut geom, |sv: StrokeVertex| {
            let p = sv.position();
            [p.x, p.y]
        }),
    );
    if result.is_err() {
        return;
    }
    let base = vertices.len() as u16;
    for p in &geom.vertices {
        let tp = apply_transform(*p, t);
        vertices.push(Vertex {
            pos: tp,
            color,
            z_index: z,
        });
    }
    indices.extend(geom.indices.iter().map(|i| base + *i));
}

/// Build a Path representing a rounded rectangle using cubic Beziers (kappa approximation).
/// This path is then tessellated by lyon for precise coverage (avoids fan artifacts on small radii).
fn rounded_rect_to_path(rrect: RoundedRect) -> Path {
    let rect = rrect.rect;
    let mut tl = rrect.radii.tl.min(rect.w * 0.5).min(rect.h * 0.5);
    let mut tr = rrect.radii.tr.min(rect.w * 0.5).min(rect.h * 0.5);
    let mut br = rrect.radii.br.min(rect.w * 0.5).min(rect.h * 0.5);
    let mut bl = rrect.radii.bl.min(rect.w * 0.5).min(rect.h * 0.5);

    // Clamp negative or NaN just in case
    for r in [&mut tl, &mut tr, &mut br, &mut bl] {
        if !r.is_finite() || *r < 0.0 {
            *r = 0.0;
        }
    }

    // If radii are effectively zero, fall back to a plain rect path
    if tl <= 0.0 && tr <= 0.0 && br <= 0.0 && bl <= 0.0 {
        return Path {
            cmds: vec![
                PathCmd::MoveTo([rect.x, rect.y]),
                PathCmd::LineTo([rect.x + rect.w, rect.y]),
                PathCmd::LineTo([rect.x + rect.w, rect.y + rect.h]),
                PathCmd::LineTo([rect.x, rect.y + rect.h]),
                PathCmd::Close,
            ],
            fill_rule: FillRule::NonZero,
        };
    }

    // Kappa for quarter circle cubic approximation
    const K: f32 = 0.552_284_749_831;
    let x0 = rect.x;
    let y0 = rect.y;
    let x1 = rect.x + rect.w;
    let y1 = rect.y + rect.h;

    // Start at top-left corner top edge tangent
    let mut cmds: Vec<PathCmd> = Vec::new();
    cmds.push(PathCmd::MoveTo([x0 + tl, y0]));

    // Top edge to before TR arc
    cmds.push(PathCmd::LineTo([x1 - tr, y0]));
    // TR arc (clockwise): from (x1 - tr, y0) to (x1, y0 + tr)
    if tr > 0.0 {
        let c1 = [x1 - tr + K * tr, y0];
        let c2 = [x1, y0 + tr - K * tr];
        let p = [x1, y0 + tr];
        cmds.push(PathCmd::CubicTo(c1, c2, p));
    } else {
        cmds.push(PathCmd::LineTo([x1, y0]));
        cmds.push(PathCmd::LineTo([x1, y0 + tr]));
    }

    // Right edge down to before BR arc
    cmds.push(PathCmd::LineTo([x1, y1 - br]));
    // BR arc: from (x1, y1 - br) to (x1 - br, y1)
    if br > 0.0 {
        let c1 = [x1, y1 - br + K * br];
        let c2 = [x1 - br + K * br, y1];
        let p = [x1 - br, y1];
        cmds.push(PathCmd::CubicTo(c1, c2, p));
    } else {
        cmds.push(PathCmd::LineTo([x1, y1]));
        cmds.push(PathCmd::LineTo([x1 - br, y1]));
    }

    // Bottom edge to before BL arc
    cmds.push(PathCmd::LineTo([x0 + bl, y1]));
    // BL arc: from (x0 + bl, y1) to (x0, y1 - bl)
    if bl > 0.0 {
        let c1 = [x0 + bl - K * bl, y1];
        let c2 = [x0, y1 - bl + K * bl];
        let p = [x0, y1 - bl];
        cmds.push(PathCmd::CubicTo(c1, c2, p));
    } else {
        cmds.push(PathCmd::LineTo([x0, y1]));
        cmds.push(PathCmd::LineTo([x0, y1 - bl]));
    }

    // Left edge up to before TL arc
    cmds.push(PathCmd::LineTo([x0, y0 + tl]));
    // TL arc: from (x0, y0 + tl) to (x0 + tl, y0)
    if tl > 0.0 {
        let c1 = [x0, y0 + tl - K * tl];
        let c2 = [x0 + tl - K * tl, y0];
        let p = [x0 + tl, y0];
        cmds.push(PathCmd::CubicTo(c1, c2, p));
    } else {
        cmds.push(PathCmd::LineTo([x0, y0]));
        cmds.push(PathCmd::LineTo([x0 + tl, y0]));
    }

    cmds.push(PathCmd::Close);
    Path {
        cmds,
        fill_rule: FillRule::NonZero,
    }
}

fn push_rounded_rect(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    rrect: RoundedRect,
    color: [f32; 4],
    z: f32,
    t: Transform2D,
) {
    // Delegate to lyon's robust tessellator via our generic path fill
    let path = rounded_rect_to_path(rrect);
    tessellate_path_fill(vertices, indices, &path, color, z, t);
}

fn push_rect_stroke(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    rect: Rect,
    stroke: Stroke,
    color: [f32; 4],
    z: f32,
    t: Transform2D,
) {
    let w = stroke.width.max(0.0);
    if w <= 0.0001 {
        return;
    }
    // Outer corners
    let o0 = apply_transform([rect.x, rect.y], t);
    let o1 = apply_transform([rect.x + rect.w, rect.y], t);
    let o2 = apply_transform([rect.x + rect.w, rect.y + rect.h], t);
    let o3 = apply_transform([rect.x, rect.y + rect.h], t);
    // Inner corners (shrink by width)
    let ix0 = rect.x + w;
    let iy0 = rect.y + w;
    let ix1 = (rect.x + rect.w - w).max(ix0);
    let iy1 = (rect.y + rect.h - w).max(iy0);
    let i0 = apply_transform([ix0, iy0], t);
    let i1 = apply_transform([ix1, iy0], t);
    let i2 = apply_transform([ix1, iy1], t);
    let i3 = apply_transform([ix0, iy1], t);

    let base = vertices.len() as u16;
    vertices.extend_from_slice(&[
        Vertex { pos: o0, color, z_index: z }, // 0
        Vertex { pos: o1, color, z_index: z }, // 1
        Vertex { pos: o2, color, z_index: z }, // 2
        Vertex { pos: o3, color, z_index: z }, // 3
        Vertex { pos: i0, color, z_index: z }, // 4
        Vertex { pos: i1, color, z_index: z }, // 5
        Vertex { pos: i2, color, z_index: z }, // 6
        Vertex { pos: i3, color, z_index: z }, // 7
    ]);
    // Build ring from quads on each edge
    let idx: [u16; 24] = [
        // top edge: o0-o1-i1-i0
        0, 1, 5, 0, 5, 4, // right edge: o1-o2-i2-i1
        1, 2, 6, 1, 6, 5, // bottom edge: o2-o3-i3-i2
        2, 3, 7, 2, 7, 6, // left edge: o3-o0-i0-i3
        3, 0, 4, 3, 4, 7,
    ];
    indices.extend(idx.iter().map(|i| base + i));
}

fn push_rounded_rect_stroke(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    rrect: RoundedRect,
    stroke: Stroke,
    color: [f32; 4],
    z: f32,
    t: Transform2D,
) {
    let w = stroke.width.max(0.0);
    if w <= 0.0001 {
        return;
    }
    let path = rounded_rect_to_path(rrect);
    tessellate_path_stroke(
        vertices,
        indices,
        &path,
        Stroke { width: w },
        color,
        z,
        t,
    );
}

pub fn upload_display_list(
    allocator: &mut RenderAllocator,
    queue: &wgpu::Queue,
    list: &DisplayList,
) -> Result<GpuScene> {
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();

    // NOTE: Z-index sorting disabled because it breaks clip/transform stacks.
    // For proper z-ordering, we need to either:
    // 1. Use a depth buffer, or
    // 2. Ensure commands are emitted in the correct z-order from the start
    // let mut sorted_list = list.clone();
    // sorted_list.sort_by_z();

    for cmd in &list.commands {
        match cmd {
            Command::DrawRect {
                rect,
                brush,
                transform,
                z,
                ..
            } => {
                match brush {
                    Brush::Solid(col) => {
                        let color = [col.r, col.g, col.b, col.a];
                        let (v, i) = rect_to_verts(*rect, color, *transform, *z as f32);
                        let base = vertices.len() as u16;
                        vertices.extend_from_slice(&v);
                        indices.extend(i.iter().map(|idx| base + idx));
                    }
                    Brush::LinearGradient { stops, .. } => {
                        // Only handle horizontal gradients for now: map t along x within rect
                        let mut packed: Vec<(f32, [f32; 4])> = stops
                            .iter()
                            .map(|(tpos, c)| (*tpos, [c.r, c.g, c.b, c.a]))
                            .collect();
                        if packed.is_empty() {
                            continue;
                        }
                        // Clamp and ensure 0 and 1 exist
                        if packed.first().unwrap().0 > 0.0 {
                            let c = packed.first().unwrap().1;
                            packed.insert(0, (0.0, c));
                        }
                        if packed.last().unwrap().0 < 1.0 {
                            let c = packed.last().unwrap().1;
                            packed.push((1.0, c));
                        }
                        push_rect_linear_gradient(
                            &mut vertices,
                            &mut indices,
                            *rect,
                            &packed,
                            *transform,
                            *z as f32,
                        );
                    }
                    _ => {}
                }
            }
            Command::DrawRoundedRect {
                rrect,
                brush,
                transform,
                z,
                ..
            } => {
                if let Brush::Solid(col) = brush {
                    let color = [col.r, col.g, col.b, col.a];
                    push_rounded_rect(
                        &mut vertices,
                        &mut indices,
                        *rrect,
                        color,
                        *z as f32,
                        *transform,
                    );
                }
            }
            Command::StrokeRect {
                rect,
                stroke,
                brush,
                transform,
                z,
                ..
            } => {
                if let Brush::Solid(col) = brush {
                    let color = [col.r, col.g, col.b, col.a];
                    push_rect_stroke(
                        &mut vertices,
                        &mut indices,
                        *rect,
                        *stroke,
                        color,
                        *z as f32,
                        *transform,
                    );
                }
            }
            Command::StrokeRoundedRect {
                rrect,
                stroke,
                brush,
                transform,
                z,
                ..
            } => {
                if let Brush::Solid(col) = brush {
                    let color = [col.r, col.g, col.b, col.a];
                    push_rounded_rect_stroke(
                        &mut vertices,
                        &mut indices,
                        *rrect,
                        *stroke,
                        color,
                        *z as f32,
                        *transform,
                    );
                }
            }
            Command::DrawEllipse {
                center,
                radii,
                brush,
                transform,
                z,
                ..
            } => match brush {
                Brush::Solid(col) => {
                    let color = [col.r, col.g, col.b, col.a];
                    push_ellipse(
                        &mut vertices,
                        &mut indices,
                        *center,
                        *radii,
                        color,
                        *z as f32,
                        *transform,
                    );
                }
                Brush::RadialGradient {
                    center: _gcenter,
                    radius: _r,
                    stops,
                } => {
                    let mut packed: Vec<(f32, [f32; 4])> = stops
                        .iter()
                        .map(|(t, c)| (*t, [c.r, c.g, c.b, c.a]))
                        .collect();
                    if packed.is_empty() {
                        continue;
                    }
                    if packed.first().unwrap().0 > 0.0 {
                        let c = packed.first().unwrap().1;
                        packed.insert(0, (0.0, c));
                    }
                    if packed.last().unwrap().0 < 1.0 {
                        let c = packed.last().unwrap().1;
                        packed.push((1.0, c));
                    }
                    push_ellipse_radial_gradient(
                        &mut vertices,
                        &mut indices,
                        *center,
                        *radii,
                        &packed,
                        *z as f32,
                        *transform,
                    );
                }
                _ => {}
            },
            Command::FillPath {
                path,
                color,
                transform,
                z,
                ..
            } => {
                let col = [color.r, color.g, color.b, color.a];
                tessellate_path_fill(
                    &mut vertices,
                    &mut indices,
                    path,
                    col,
                    *z as f32,
                    *transform,
                );
            }
            Command::StrokePath {
                path,
                stroke,
                color,
                transform,
                z,
                ..
            } => {
                let col = [color.r, color.g, color.b, color.a];
                tessellate_path_stroke(
                    &mut vertices,
                    &mut indices,
                    path,
                    *stroke,
                    col,
                    *z as f32,
                    *transform,
                );
            }
            // BoxShadow commands are handled by PassManager as a separate pipeline.
            Command::BoxShadow { .. } => {}
            // Hit-only regions: intentionally not rendered.
            Command::HitRegionRect { .. } => {}
            Command::HitRegionRoundedRect { .. } => {}
            Command::HitRegionEllipse { .. } => {}
            _ => {}
        }
    }

    // Ensure index buffer size meets COPY_BUFFER_ALIGNMENT (4 bytes)
    if (indices.len() % 2) != 0 {
        if indices.len() >= 3 {
            let a = indices[indices.len() - 3];
            let b = indices[indices.len() - 2];
            let c = indices[indices.len() - 1];
            indices.extend_from_slice(&[a, b, c]);
        } else {
            indices.push(0);
        }
    }

    // Allocate GPU buffers and upload
    let vsize = (vertices.len() * std::mem::size_of::<Vertex>()) as u64;
    let isize = (indices.len() * std::mem::size_of::<u16>()) as u64;
    let vbuf = allocator.allocate_buffer(BufKey {
        size: vsize.max(4),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });
    let ibuf = allocator.allocate_buffer(BufKey {
        size: isize.max(4),
        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
    });
    if vsize > 0 {
        queue.write_buffer(&vbuf.buffer, 0, bytemuck::cast_slice(&vertices));
    }
    if isize > 0 {
        queue.write_buffer(&ibuf.buffer, 0, bytemuck::cast_slice(&indices));
    }

    Ok(GpuScene {
        vertex: vbuf,
        index: ibuf,
        vertices: vertices.len() as u32,
        indices: indices.len() as u32,
    })
}

/// Upload a DisplayList extracting all element types for unified rendering.
/// This is the main entry point for the unified rendering system.
///
/// Returns:
/// - GpuScene: Uploaded solid geometry (rectangles, paths, etc.)
/// - text_draws: Text runs with their transforms and z-indices
/// - image_draws: Image draws (currently placeholder, will be implemented)
/// - svg_draws: SVG draws (currently placeholder, will be implemented)
pub fn upload_display_list_unified(
    allocator: &mut RenderAllocator,
    queue: &wgpu::Queue,
    list: &DisplayList,
) -> Result<UnifiedSceneData> {
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();
    let mut text_draws: Vec<ExtractedTextDraw> = Vec::new();
    let mut image_draws: Vec<ExtractedImageDraw> = Vec::new();
    let mut svg_draws: Vec<ExtractedSvgDraw> = Vec::new();

    // Track current transform stack for text extraction
    let mut transform_stack: Vec<Transform2D> = vec![Transform2D::identity()];
    let mut current_transform = Transform2D::identity();

    for cmd in &list.commands {
        match cmd {
            // Handle transform stack
            Command::PushTransform(t) => {
                current_transform = current_transform.concat(*t);
                transform_stack.push(current_transform);
            }
            Command::PopTransform => {
                transform_stack.pop();
                current_transform = transform_stack.last().copied().unwrap_or(Transform2D::identity());
            }

            // Extract text commands
            Command::DrawText { run, z, transform, .. } => {
                // Apply both the command transform and the current transform stack
                let final_transform = current_transform.concat(*transform);
                text_draws.push(ExtractedTextDraw {
                    run: run.clone(),
                    z: *z,
                    transform: final_transform,
                });
            }

            // Process solid geometry commands
            Command::DrawRect {
                rect,
                brush,
                transform,
                z,
                ..
            } => {
                let final_transform = current_transform.concat(*transform);
                match brush {
                    Brush::Solid(col) => {
                        let color = [col.r, col.g, col.b, col.a];
                        let (v, i) = rect_to_verts(*rect, color, final_transform, *z as f32);
                        let base = vertices.len() as u16;
                        vertices.extend_from_slice(&v);
                        indices.extend(i.iter().map(|idx| base + idx));
                    }
                    Brush::LinearGradient { stops, .. } => {
                        // Only handle horizontal gradients for now: map t along x within rect
                        let mut packed: Vec<(f32, [f32; 4])> = stops
                            .iter()
                            .map(|(tpos, c)| (*tpos, [c.r, c.g, c.b, c.a]))
                            .collect();
                        if packed.is_empty() {
                            continue;
                        }
                        // Clamp and ensure 0 and 1 exist
                        if packed.first().unwrap().0 > 0.0 {
                            let c = packed.first().unwrap().1;
                            packed.insert(0, (0.0, c));
                        }
                        if packed.last().unwrap().0 < 1.0 {
                            let c = packed.last().unwrap().1;
                            packed.push((1.0, c));
                        }
                        push_rect_linear_gradient(
                            &mut vertices,
                            &mut indices,
                            *rect,
                            &packed,
                            final_transform,
                            *z as f32,
                        );
                    }
                    _ => {}
                }
            }
            Command::DrawRoundedRect {
                rrect,
                brush,
                transform,
                z,
                ..
            } => {
                let final_transform = current_transform.concat(*transform);
                if let Brush::Solid(col) = brush {
                    let color = [col.r, col.g, col.b, col.a];
                    push_rounded_rect(
                        &mut vertices,
                        &mut indices,
                        *rrect,
                        color,
                        *z as f32,
                        final_transform,
                    );
                }
            }
            Command::StrokeRect {
                rect,
                stroke,
                brush,
                transform,
                z,
                ..
            } => {
                let final_transform = current_transform.concat(*transform);
                if let Brush::Solid(col) = brush {
                    let color = [col.r, col.g, col.b, col.a];
                    push_rect_stroke(
                        &mut vertices,
                        &mut indices,
                        *rect,
                        *stroke,
                        color,
                        *z as f32,
                        final_transform,
                    );
                }
            }
            Command::StrokeRoundedRect {
                rrect,
                stroke,
                brush,
                transform,
                z,
                ..
            } => {
                let final_transform = current_transform.concat(*transform);
                if let Brush::Solid(col) = brush {
                    let color = [col.r, col.g, col.b, col.a];
                    push_rounded_rect_stroke(
                        &mut vertices,
                        &mut indices,
                        *rrect,
                        *stroke,
                        color,
                        *z as f32,
                        final_transform,
                    );
                }
            }
            Command::DrawEllipse {
                center,
                radii,
                brush,
                transform,
                z,
                ..
            } => {
                let final_transform = current_transform.concat(*transform);
                match brush {
                    Brush::Solid(col) => {
                        let color = [col.r, col.g, col.b, col.a];
                        push_ellipse(
                            &mut vertices,
                            &mut indices,
                            *center,
                            *radii,
                            color,
                            *z as f32,
                            final_transform,
                        );
                    }
                    Brush::RadialGradient {
                        center: _gcenter,
                        radius: _r,
                        stops,
                    } => {
                        let mut packed: Vec<(f32, [f32; 4])> = stops
                            .iter()
                            .map(|(t, c)| (*t, [c.r, c.g, c.b, c.a]))
                            .collect();
                        if packed.is_empty() {
                            continue;
                        }
                        if packed.first().unwrap().0 > 0.0 {
                            let c = packed.first().unwrap().1;
                            packed.insert(0, (0.0, c));
                        }
                        if packed.last().unwrap().0 < 1.0 {
                            let c = packed.last().unwrap().1;
                            packed.push((1.0, c));
                        }
                        push_ellipse_radial_gradient(
                            &mut vertices,
                            &mut indices,
                            *center,
                            *radii,
                            &packed,
                            *z as f32,
                            final_transform,
                        );
                    }
                    _ => {}
                }
            }
            Command::FillPath {
                path,
                color,
                transform,
                z,
                ..
            } => {
                let final_transform = current_transform.concat(*transform);
                let col = [color.r, color.g, color.b, color.a];
                tessellate_path_fill(
                    &mut vertices,
                    &mut indices,
                    path,
                    col,
                    *z as f32,
                    final_transform,
                );
            }
            Command::StrokePath {
                path,
                stroke,
                color,
                transform,
                z,
                ..
            } => {
                let final_transform = current_transform.concat(*transform);
                let col = [color.r, color.g, color.b, color.a];
                tessellate_path_stroke(
                    &mut vertices,
                    &mut indices,
                    path,
                    *stroke,
                    col,
                    *z as f32,
                    final_transform,
                );
            }
            Command::DrawImage {
                path,
                origin,
                size,
                z,
                transform,
            } => {
                // Apply current transform stack and the command transform to the image origin.
                let final_transform = current_transform.concat(*transform);
                let world_origin = apply_transform(*origin, final_transform);
                image_draws.push(ExtractedImageDraw {
                    path: path.clone(),
                    origin: world_origin,
                    size: *size,
                    z: *z,
                    transform: final_transform,
                });
            }
            Command::DrawSvg {
                path,
                origin,
                max_size,
                z,
                transform,
            } => {
                // Apply current transform stack and the command transform to the SVG origin.
                let final_transform = current_transform.concat(*transform);
                let world_origin = apply_transform(*origin, final_transform);
                svg_draws.push(ExtractedSvgDraw {
                    path: path.clone(),
                    origin: world_origin,
                    size: *max_size,
                    z: *z,
                    transform: final_transform,
                });
            }
            // BoxShadow commands are handled by PassManager as a separate pipeline.
            Command::BoxShadow { .. } => {}
            // Hit-only regions: intentionally not rendered.
            Command::HitRegionRect { .. } => {}
            Command::HitRegionRoundedRect { .. } => {}
            Command::HitRegionEllipse { .. } => {}
            // Clip commands would need special handling in unified rendering
            Command::PushClip(_) => {}
            Command::PopClip => {}
        }
    }

    // Ensure index buffer size meets COPY_BUFFER_ALIGNMENT (4 bytes)
    if (indices.len() % 2) != 0 {
        if indices.len() >= 3 {
            let a = indices[indices.len() - 3];
            let b = indices[indices.len() - 2];
            let c = indices[indices.len() - 1];
            indices.extend_from_slice(&[a, b, c]);
        } else {
            indices.push(0);
        }
    }

    // Allocate GPU buffers and upload
    let vsize = (vertices.len() * std::mem::size_of::<Vertex>()) as u64;
    let isize = (indices.len() * std::mem::size_of::<u16>()) as u64;
    let vbuf = allocator.allocate_buffer(BufKey {
        size: vsize.max(4),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });
    let ibuf = allocator.allocate_buffer(BufKey {
        size: isize.max(4),
        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
    });
    if vsize > 0 {
        queue.write_buffer(&vbuf.buffer, 0, bytemuck::cast_slice(&vertices));
    }
    if isize > 0 {
        queue.write_buffer(&ibuf.buffer, 0, bytemuck::cast_slice(&indices));
    }

    Ok(UnifiedSceneData {
        gpu_scene: GpuScene {
            vertex: vbuf,
            index: ibuf,
            vertices: vertices.len() as u32,
            indices: indices.len() as u32,
        },
        text_draws,
        image_draws,
        svg_draws,
    })
}
