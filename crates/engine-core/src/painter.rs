use crate::display_list::{Command, DisplayList, Viewport};
use crate::scene::*;
use std::path::PathBuf;

pub struct Painter {
    list: DisplayList,
    transform_stack: Vec<Transform2D>,
    clip_depth: usize,
}

impl Painter {
    pub fn begin_frame(viewport: Viewport) -> Self {
        Self {
            list: DisplayList {
                viewport,
                commands: Vec::new(),
            },
            transform_stack: vec![Transform2D::identity()],
            clip_depth: 0,
        }
    }

    pub fn current_transform(&self) -> Transform2D {
        *self.transform_stack.last().unwrap()
    }

    pub fn push_transform(&mut self, t: Transform2D) {
        // Compose with current transform so nested pushes multiply.
        let composed = self.current_transform().concat(t);
        self.list.commands.push(Command::PushTransform(composed));
        self.transform_stack.push(composed);
    }
    pub fn pop_transform(&mut self) {
        self.list.commands.push(Command::PopTransform);
        let _ = self.transform_stack.pop();
    }

    pub fn push_clip_rect(&mut self, rect: Rect) {
        self.clip_depth += 1;
        self.list.commands.push(Command::PushClip(ClipRect(rect)));
    }
    pub fn pop_clip(&mut self) {
        if self.clip_depth > 0 {
            self.clip_depth -= 1;
            self.list.commands.push(Command::PopClip);
        }
    }

    pub fn rect(&mut self, rect: Rect, brush: Brush, z: i32) {
        let t = self.current_transform();
        self.list.commands.push(Command::DrawRect {
            rect,
            brush,
            z,
            transform: t,
        });
    }

    pub fn rounded_rect(&mut self, rrect: RoundedRect, brush: Brush, z: i32) {
        let t = self.current_transform();
        self.list.commands.push(Command::DrawRoundedRect {
            rrect,
            brush,
            z,
            transform: t,
        });
    }

    pub fn stroke_rect(&mut self, rect: Rect, stroke: Stroke, brush: Brush, z: i32) {
        let t = self.current_transform();
        self.list.commands.push(Command::StrokeRect {
            rect,
            stroke,
            brush,
            z,
            transform: t,
        });
    }

    pub fn stroke_rounded_rect(
        &mut self,
        rrect: RoundedRect,
        stroke: Stroke,
        brush: Brush,
        z: i32,
    ) {
        let t = self.current_transform();
        self.list.commands.push(Command::StrokeRoundedRect {
            rrect,
            stroke,
            brush,
            z,
            transform: t,
        });
    }

    /// Draw text with an explicit stable id and dynamic flag.
    /// Callers that don't care about ids can use `text`, which passes 0 / false.
    pub fn text_with_id(&mut self, run: TextRun, z: i32, id: u64, dynamic: bool) {
        let t = self.current_transform();
        self.list.commands.push(Command::DrawText {
            run,
            z,
            transform: t,
            id,
            dynamic,
        });
    }

    pub fn text(&mut self, run: TextRun, z: i32) {
        self.text_with_id(run, z, 0, false);
    }

    pub fn ellipse(&mut self, center: [f32; 2], radii: [f32; 2], brush: Brush, z: i32) {
        let t = self.current_transform();
        self.list.commands.push(Command::DrawEllipse {
            center,
            radii,
            brush,
            z,
            transform: t,
        });
    }

    pub fn circle(&mut self, center: [f32; 2], radius: f32, brush: Brush, z: i32) {
        self.ellipse(center, [radius, radius], brush, z);
    }

    /// Queue an SVG to be rasterized and drawn at origin, scaled to fit within max_size.
    /// The path is interpreted relative to the process working directory.
    pub fn svg<P: Into<PathBuf>>(
        &mut self,
        path: P,
        origin: [f32; 2],
        max_size: [f32; 2],
        z: i32,
    ) {
        let t = self.current_transform();
        self.list.commands.push(Command::DrawSvg {
            path: path.into(),
            origin,
            max_size,
            z,
            transform: t,
        });
    }

    /// Queue a raster image (PNG/JPEG/GIF/WebP) to be drawn at origin with the given pixel size.
    /// The path is interpreted relative to the process working directory.
    pub fn image<P: Into<PathBuf>>(
        &mut self,
        path: P,
        origin: [f32; 2],
        size: [f32; 2],
        z: i32,
    ) {
        let t = self.current_transform();
        self.list.commands.push(Command::DrawImage {
            path: path.into(),
            origin,
            size,
            z,
            transform: t,
        });
    }

    pub fn box_shadow(&mut self, rrect: RoundedRect, spec: BoxShadowSpec, z: i32) {
        let t = self.current_transform();
        self.list.commands.push(Command::BoxShadow {
            rrect,
            spec,
            z,
            transform: t,
        });
    }

    /// Fill a path with a solid color. For now we only support solid color fills for paths.
    pub fn fill_path(&mut self, path: Path, color: ColorLinPremul, z: i32) {
        let t = self.current_transform();
        self.list.commands.push(Command::FillPath {
            path,
            color,
            z,
            transform: t,
        });
    }

    /// Stroke a path with uniform width and a solid color.
    pub fn stroke_path(&mut self, path: Path, stroke: Stroke, color: ColorLinPremul, z: i32) {
        let t = self.current_transform();
        self.list.commands.push(Command::StrokePath {
            path,
            stroke,
            color,
            z,
            transform: t,
        });
    }

    // --- Hit-only regions (do not render) ---
    pub fn hit_region_rect(&mut self, id: u32, rect: Rect, z: i32) {
        let t = self.current_transform();
        self.list.commands.push(Command::HitRegionRect {
            id,
            rect,
            z,
            transform: t,
        });
    }

    pub fn hit_region_rounded_rect(&mut self, id: u32, rrect: RoundedRect, z: i32) {
        let t = self.current_transform();
        self.list.commands.push(Command::HitRegionRoundedRect {
            id,
            rrect,
            z,
            transform: t,
        });
    }

    pub fn hit_region_ellipse(&mut self, id: u32, center: [f32; 2], radii: [f32; 2], z: i32) {
        let t = self.current_transform();
        self.list.commands.push(Command::HitRegionEllipse {
            id,
            center,
            radii,
            z,
            transform: t,
        });
    }

    /// Get a reference to the display list (for hit testing before finishing)
    pub fn display_list(&self) -> &DisplayList {
        &self.list
    }

    pub fn finish(self) -> DisplayList {
        self.list
    }
}
