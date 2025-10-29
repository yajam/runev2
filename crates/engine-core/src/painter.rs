use crate::display_list::{Command, DisplayList, Viewport};
use crate::scene::*;

pub struct Painter {
    list: DisplayList,
    transform_stack: Vec<Transform2D>,
    clip_depth: usize,
}

impl Painter {
    pub fn begin_frame(viewport: Viewport) -> Self {
        Self { list: DisplayList { viewport, commands: Vec::new() }, transform_stack: vec![Transform2D::identity()], clip_depth: 0 }
    }

    fn current_transform(&self) -> Transform2D { *self.transform_stack.last().unwrap() }

    pub fn push_transform(&mut self, t: Transform2D) { self.list.commands.push(Command::PushTransform(t)); self.transform_stack.push(t); }
    pub fn pop_transform(&mut self) { self.list.commands.push(Command::PopTransform); let _ = self.transform_stack.pop(); }

    pub fn push_clip_rect(&mut self, rect: Rect) { self.clip_depth += 1; self.list.commands.push(Command::PushClip(ClipRect(rect))); }
    pub fn pop_clip(&mut self) { if self.clip_depth > 0 { self.clip_depth -= 1; self.list.commands.push(Command::PopClip); } }

    pub fn rect(&mut self, rect: Rect, brush: Brush, z: i32) {
        let t = self.current_transform();
        self.list.commands.push(Command::DrawRect { rect, brush, z, transform: t });
    }

    pub fn rounded_rect(&mut self, rrect: RoundedRect, brush: Brush, z: i32) {
        let t = self.current_transform();
        self.list.commands.push(Command::DrawRoundedRect { rrect, brush, z, transform: t });
    }

    pub fn text(&mut self, run: TextRun, z: i32) {
        let t = self.current_transform();
        self.list.commands.push(Command::DrawText { run, z, transform: t });
    }

    pub fn ellipse(&mut self, center: [f32; 2], radii: [f32; 2], brush: Brush, z: i32) {
        let t = self.current_transform();
        self.list.commands.push(Command::DrawEllipse { center, radii, brush, z, transform: t });
    }

    pub fn circle(&mut self, center: [f32; 2], radius: f32, brush: Brush, z: i32) {
        self.ellipse(center, [radius, radius], brush, z);
    }

    pub fn finish(self) -> DisplayList { self.list }
}
