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
    DrawText { run: TextRun, z: i32, transform: Transform2D },
    DrawEllipse { center: [f32; 2], radii: [f32; 2], brush: Brush, z: i32, transform: Transform2D },
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
