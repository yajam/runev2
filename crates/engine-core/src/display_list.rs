use crate::scene::*;

#[derive(Clone, Copy, Debug, Default)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug)]
pub enum Command {
    DrawRect { rect: Rect, brush: Brush, z: i32, transform: Transform2D },
    DrawRoundedRect { rrect: RoundedRect, brush: Brush, z: i32, transform: Transform2D },
    StrokeRect { rect: Rect, stroke: Stroke, brush: Brush, z: i32, transform: Transform2D },
    StrokeRoundedRect { rrect: RoundedRect, stroke: Stroke, brush: Brush, z: i32, transform: Transform2D },
    DrawText { run: TextRun, z: i32, transform: Transform2D },
    DrawEllipse { center: [f32; 2], radii: [f32; 2], brush: Brush, z: i32, transform: Transform2D },
    /// Filled path (solid color only for now)
    FillPath { path: Path, color: ColorLinPremul, z: i32, transform: Transform2D },
    /// Box shadow for a rounded rectangle. This is handled by a dedicated pass in PassManager,
    /// not by the generic solid fill pipeline.
    BoxShadow { rrect: RoundedRect, spec: BoxShadowSpec, z: i32, transform: Transform2D },
    /// Hit-only regions that do not render. Useful for scene-surface hits or custom zones.
    HitRegionRect { id: u32, rect: Rect, z: i32, transform: Transform2D },
    HitRegionRoundedRect { id: u32, rrect: RoundedRect, z: i32, transform: Transform2D },
    HitRegionEllipse { id: u32, center: [f32; 2], radii: [f32; 2], z: i32, transform: Transform2D },
    PushClip(ClipRect),
    PopClip,
    PushTransform(Transform2D),
    PopTransform,
}

#[derive(Clone, Debug, Default)]
pub struct DisplayList {
    pub viewport: Viewport,
    pub commands: Vec<Command>,
}
