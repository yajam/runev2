use anyhow::Result;
use bytemuck::{Pod, Zeroable};

use crate::allocator::{BufKey, OwnedBuffer, RenderAllocator};
use crate::display_list::{Command, DisplayList};
use crate::scene::{Brush, Rect, RoundedRect, Transform2D};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub color: [f32; 4],
}

pub struct GpuScene {
    pub vertex: OwnedBuffer,
    pub index: OwnedBuffer,
    pub vertices: u32,
    pub indices: u32,
}

fn apply_transform(p: [f32; 2], t: Transform2D) -> [f32; 2] {
    let [a, b, c, d, e, f] = t.m;
    [a * p[0] + c * p[1] + e, b * p[0] + d * p[1] + f]
}

fn rect_to_verts(rect: Rect, color: [f32; 4], t: Transform2D) -> ([Vertex; 4], [u16; 6]) {
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
            Vertex { pos: p0, color },
            Vertex { pos: p1, color },
            Vertex { pos: p2, color },
            Vertex { pos: p3, color },
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
) {
    if stops.len() < 2 { return; }
    // ensure sorted
    let mut s = stops.to_vec();
    s.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let y0 = rect.y;
    let y1 = rect.y + rect.h;
    for pair in s.windows(2) {
        let (t0, c0) = (pair[0].0.clamp(0.0, 1.0), pair[0].1);
        let (t1, c1) = (pair[1].0.clamp(0.0, 1.0), pair[1].1);
        if (t1 - t0).abs() < 1e-6 { continue; }
        let x0 = rect.x + rect.w * t0;
        let x1 = rect.x + rect.w * t1;
        let p0 = apply_transform([x0, y0], t);
        let p1 = apply_transform([x1, y0], t);
        let p2 = apply_transform([x1, y1], t);
        let p3 = apply_transform([x0, y1], t);
        let base = vertices.len() as u16;
        vertices.extend_from_slice(&[
            Vertex { pos: p0, color: c0 },
            Vertex { pos: p1, color: c1 },
            Vertex { pos: p2, color: c1 },
            Vertex { pos: p3, color: c0 },
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
    t: Transform2D,
) {
    let segs = 64u32;
    let base = vertices.len() as u16;
    let c = apply_transform(center, t);
    vertices.push(Vertex { pos: c, color });

    for i in 0..segs {
        let theta = (i as f32) / (segs as f32) * std::f32::consts::TAU;
        let p = [center[0] + radii[0] * theta.cos(), center[1] + radii[1] * theta.sin()];
        let p = apply_transform(p, t);
        vertices.push(Vertex { pos: p, color });
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
    t: Transform2D,
) {
    if stops.len() < 2 { return; }
    let mut s = stops.to_vec();
    s.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let segs = 64u32;
    let base_center = vertices.len() as u16;
    // Center vertex with first stop color
    let cpos = apply_transform(center, t);
    vertices.push(Vertex { pos: cpos, color: s[0].1 });

    // First ring
    let mut prev_ring_start = vertices.len() as u16;
    let mut prev_color = s[0].1;
    let mut prev_t = s[0].0.clamp(0.0, 1.0);
    if prev_t <= 0.0 { prev_t = 0.0; }
    for i in 0..segs {
        let theta = (i as f32) / (segs as f32) * std::f32::consts::TAU;
        let p = [center[0] + radii[0] * prev_t * theta.cos(), center[1] + radii[1] * prev_t * theta.sin()];
        let p = apply_transform(p, t);
        vertices.push(Vertex { pos: p, color: prev_color });
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
            let p = [center[0] + radii[0] * tcur * theta.cos(), center[1] + radii[1] * tcur * theta.sin()];
            let p = apply_transform(p, t);
            vertices.push(Vertex { pos: p, color: ccur });
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
        prev_color = ccur;
        prev_t = tcur;
    }
}

fn push_rounded_rect(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    rrect: RoundedRect,
    color: [f32; 4],
    t: Transform2D,
) {
    let rect = rrect.rect;
    let tl = rrect.radii.tl.min(rect.w * 0.5).min(rect.h * 0.5);
    let tr = rrect.radii.tr.min(rect.w * 0.5).min(rect.h * 0.5);
    let br = rrect.radii.br.min(rect.w * 0.5).min(rect.h * 0.5);
    let bl = rrect.radii.bl.min(rect.w * 0.5).min(rect.h * 0.5);

    let center_px = [rect.x + rect.w * 0.5, rect.y + rect.h * 0.5];
    let center = apply_transform(center_px, t);

    let segs = 24u32;
    let mut ring: Vec<[f32; 2]> = Vec::new();

    fn arc_append(ring: &mut Vec<[f32; 2]>, c: [f32; 2], r: f32, start: f32, end: f32, segs: u32, include_start: bool) {
        if r <= 0.0 { return; }
        for i in 0..=segs {
            if i == 0 && !include_start { continue; }
            let t = (i as f32) / (segs as f32);
            let ang = start + t * (end - start);
            let p = [c[0] + r * ang.cos(), c[1] - r * ang.sin()];
            ring.push(p);
        }
    }

    // Build ring clockwise, starting at TL top tangent
    // TL: 90° -> 180° (top -> left)
    if tl > 0.0 { arc_append(&mut ring, [rect.x + tl, rect.y + tl], tl, std::f32::consts::FRAC_PI_2, std::f32::consts::PI, segs, true); }
    else { ring.push([rect.x + 0.0, rect.y + 0.0]); }

    // BL: 180° -> 270° (left -> bottom), skip start to avoid duplicate
    if bl > 0.0 { arc_append(&mut ring, [rect.x + bl, rect.y + rect.h - bl], bl, std::f32::consts::PI, std::f32::consts::FRAC_PI_2 * 3.0, segs, false); }
    else { ring.push([rect.x + 0.0, rect.y + rect.h]); }

    // BR: 270° -> 360° (bottom -> right)
    if br > 0.0 { arc_append(&mut ring, [rect.x + rect.w - br, rect.y + rect.h - br], br, std::f32::consts::FRAC_PI_2 * 3.0, std::f32::consts::TAU, segs, false); }
    else { ring.push([rect.x + rect.w, rect.y + rect.h]); }

    // TR: 0° -> 90° (right -> top)
    if tr > 0.0 { arc_append(&mut ring, [rect.x + rect.w - tr, rect.y + tr], tr, 0.0, std::f32::consts::FRAC_PI_2, segs, false); }
    else { ring.push([rect.x + rect.w, rect.y + 0.0]); }

    let base = vertices.len() as u16;
    vertices.push(Vertex { pos: center, color });
    for p in ring {
        let p = apply_transform(p, t);
        vertices.push(Vertex { pos: p, color });
    }
    let ring_len = (vertices.len() as u16) - base - 1;
    for i in 0..ring_len {
        let i0 = base;
        let i1 = base + 1 + i;
        let i2 = base + 1 + ((i + 1) % ring_len);
        indices.extend_from_slice(&[i0, i1, i2]);
    }
}

pub fn upload_display_list(
    allocator: &mut RenderAllocator,
    queue: &wgpu::Queue,
    list: &DisplayList,
) -> Result<GpuScene> {
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();

    for cmd in &list.commands {
        match cmd {
            Command::DrawRect { rect, brush, transform, .. } => {
                match brush {
                    Brush::Solid(col) => {
                        let color = [col.r, col.g, col.b, col.a];
                        let (v, i) = rect_to_verts(*rect, color, *transform);
                        let base = vertices.len() as u16;
                        vertices.extend_from_slice(&v);
                        indices.extend(i.map(|idx| idx + base));
                    }
                    Brush::LinearGradient { start, end, stops } => {
                        // Only handle horizontal gradients for now: map t along x within rect
                        let mut packed: Vec<(f32, [f32; 4])> = stops
                            .iter()
                            .map(|(tpos, c)| (*tpos, [c.r, c.g, c.b, c.a]))
                            .collect();
                        if packed.is_empty() { continue; }
                        // Clamp and ensure 0 and 1 exist
                        if packed.first().unwrap().0 > 0.0 {
                            let c = packed.first().unwrap().1;
                            packed.insert(0, (0.0, c));
                        }
                        if packed.last().unwrap().0 < 1.0 {
                            let c = packed.last().unwrap().1;
                            packed.push((1.0, c));
                        }
                        push_rect_linear_gradient(&mut vertices, &mut indices, *rect, &packed, *transform);
                    }
                    _ => {}
                }
            }
            Command::DrawRoundedRect { rrect, brush, transform, .. } => {
                if let Brush::Solid(col) = brush {
                    let color = [col.r, col.g, col.b, col.a];
                    push_rounded_rect(&mut vertices, &mut indices, *rrect, color, *transform);
                }
            }
            Command::DrawEllipse { center, radii, brush, transform, .. } => {
                match brush {
                    Brush::Solid(col) => {
                        let color = [col.r, col.g, col.b, col.a];
                        push_ellipse(&mut vertices, &mut indices, *center, *radii, color, *transform);
                    }
                    Brush::RadialGradient { center: _gcenter, radius: _r, stops } => {
                        let mut packed: Vec<(f32, [f32;4])> = stops.iter().map(|(t,c)| (*t, [c.r,c.g,c.b,c.a])).collect();
                        if packed.is_empty() { continue; }
                        if packed.first().unwrap().0 > 0.0 {
                            let c = packed.first().unwrap().1;
                            packed.insert(0, (0.0, c));
                        }
                        if packed.last().unwrap().0 < 1.0 {
                            let c = packed.last().unwrap().1;
                            packed.push((1.0, c));
                        }
                        push_ellipse_radial_gradient(&mut vertices, &mut indices, *center, *radii, &packed, *transform);
                    }
                    _ => {}
                }
            }
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
    let vbuf = allocator.allocate_buffer(BufKey { size: vsize.max(4), usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST });
    let ibuf = allocator.allocate_buffer(BufKey { size: isize.max(4), usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST });
    if vsize > 0 { queue.write_buffer(&vbuf.buffer, 0, bytemuck::cast_slice(&vertices)); }
    if isize > 0 { queue.write_buffer(&ibuf.buffer, 0, bytemuck::cast_slice(&indices)); }

    Ok(GpuScene { vertex: vbuf, index: ibuf, vertices: vertices.len() as u32, indices: indices.len() as u32 })
}
