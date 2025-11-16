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
    DrawText { run: TextRun, z: i32, transform: Transform2D, id: u64, dynamic: bool },
    DrawEllipse { center: [f32; 2], radii: [f32; 2], brush: Brush, z: i32, transform: Transform2D },
    /// Filled path (solid color only for now)
    FillPath { path: Path, color: ColorLinPremul, z: i32, transform: Transform2D },
    /// Stroked path (width only; round join/cap for now)
    StrokePath { path: Path, stroke: Stroke, color: ColorLinPremul, z: i32, transform: Transform2D },
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

impl Command {
    /// Get the z-index of this command, or None for non-drawable commands
    pub fn z_index(&self) -> Option<i32> {
        match self {
            Command::DrawRect { z, .. } => Some(*z),
            Command::DrawRoundedRect { z, .. } => Some(*z),
            Command::StrokeRect { z, .. } => Some(*z),
            Command::StrokeRoundedRect { z, .. } => Some(*z),
            Command::DrawText { z, .. } => Some(*z),
            Command::DrawEllipse { z, .. } => Some(*z),
            Command::FillPath { z, .. } => Some(*z),
            Command::StrokePath { z, .. } => Some(*z),
            Command::BoxShadow { z, .. } => Some(*z),
            Command::HitRegionRect { z, .. } => Some(*z),
            Command::HitRegionRoundedRect { z, .. } => Some(*z),
            Command::HitRegionEllipse { z, .. } => Some(*z),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct DisplayList {
    pub viewport: Viewport,
    pub commands: Vec<Command>,
}

impl DisplayList {
    /// Sort commands by z-index while preserving clip/transform stack structure.
    /// This is a simplified implementation that sorts drawable commands but keeps
    /// transform/clip commands in their original order. For proper z-ordering with
    /// transforms and clips, each drawable should store its full transform/clip state.
    pub fn sort_by_z(&mut self) {
        // Sort by z-index. Rust's sort_by is stable, preserving relative order of equal elements.
        // This means transform/clip commands (which have no z-index) will stay in order,
        // but drawable commands will be sorted by z-index.
        self.commands.sort_by(|a, b| {
            match (a.z_index(), b.z_index()) {
                (Some(z_a), Some(z_b)) => z_a.cmp(&z_b),
                (Some(_), None) => std::cmp::Ordering::Greater, // Drawables after non-drawables
                (None, Some(_)) => std::cmp::Ordering::Less,    // Non-drawables before drawables
                (None, None) => std::cmp::Ordering::Equal,      // Preserve order for non-drawables
            }
        });
    }
}
