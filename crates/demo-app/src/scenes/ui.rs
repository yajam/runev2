use engine_core::{Brush, ColorLinPremul as Color, DisplayList, Painter, Path, PathCmd, Rect, RoundedRadii, RoundedRect, Stroke, Viewport};

use super::{Scene, SceneKind};

pub struct UiElementsScene {
    viewport: Viewport,
}

impl Default for UiElementsScene {
    fn default() -> Self { Self { viewport: Viewport { width: 1280, height: 800 } } }
}

impl UiElementsScene {
    fn build(&self) -> DisplayList {
        let mut p = Painter::begin_frame(self.viewport);

        // Background
        p.rect(
            Rect { x: 0.0, y: 0.0, w: self.viewport.width as f32, h: self.viewport.height as f32 },
            Brush::Solid(Color::from_srgba_u8([24, 28, 40, 255])),
            0,
        );

        // All sizes are authored in logical pixels; PassManager applies DPI scaling centrally.

        let col1_x = 40.0f32;
        let col2_x = (self.viewport.width as f32 * 0.5).max(320.0) + 40.0;
        let mut y = 40.0f32;

        // Title
        p.text(
            engine_core::TextRun { text: "UI Elements".to_string(), pos: [col1_x, y], size: 24.0, color: Color::from_srgba_u8([235, 240, 255, 255]) },
            2,
        );
        y += 36.0;

        // Buttons
        let btn_r = RoundedRect { rect: Rect { x: col1_x, y, w: 160.0, h: 36.0 }, radii: RoundedRadii { tl: 8.0, tr: 8.0, br: 8.0, bl: 8.0 } };
        p.rounded_rect(btn_r, Brush::Solid(Color::from_srgba_u8([63, 130, 246, 255])), 3);
        p.text(engine_core::TextRun { text: "Primary".to_string(), pos: [col1_x + 12.0, y + 24.0], size: 16.0, color: Color::from_srgba_u8([255, 255, 255, 255]) }, 4);
        let btn2_r = RoundedRect { rect: Rect { x: col1_x + 176.0, y, w: 180.0, h: 36.0 }, radii: RoundedRadii { tl: 8.0, tr: 8.0, br: 8.0, bl: 8.0 } };
        p.rounded_rect(btn2_r, Brush::Solid(Color::from_srgba_u8([99, 104, 118, 255])), 3);
        p.text(engine_core::TextRun { text: "Secondary".to_string(), pos: [col1_x + 188.0, y + 24.0], size: 16.0, color: Color::from_srgba_u8([255, 255, 255, 255]) }, 4);
        y += 46.0;

        // Checkboxes
        let cb_size = 18.0f32;
        let cb0 = Rect { x: col1_x, y, w: cb_size, h: cb_size };
        p.rect(cb0, Brush::Solid(Color::from_srgba_u8([240, 240, 240, 255])), 3);
        p.rect(Rect { x: cb0.x, y: cb0.y, w: cb0.w, h: 1.0 }, Brush::Solid(Color::from_srgba_u8([180, 180, 180, 255])), 4);
        p.text(engine_core::TextRun { text: "Checkbox".to_string(), pos: [col1_x + 28.0, y + 16.0], size: 16.0, color: Color::from_srgba_u8([240, 240, 240, 255]) }, 4);

        // Checked + focus outline
        let cb1 = Rect { x: col1_x + 160.0, y, w: cb_size, h: cb_size };
        // Base box with small rounded corners to match focus border
        let cb1_base = RoundedRect { rect: cb1, radii: RoundedRadii { tl: 2.0, tr: 2.0, br: 2.0, bl: 2.0 } };
        p.rounded_rect(cb1_base, Brush::Solid(Color::from_srgba_u8([240, 240, 240, 255])), 3);
        // Focus border
        let cb1_rr = RoundedRect { rect: cb1, radii: RoundedRadii { tl: 2.0, tr: 2.0, br: 2.0, bl: 2.0 } };
        p.stroke_rounded_rect(cb1_rr, Stroke { width: 2.0 }, Brush::Solid(Color::from_srgba_u8([63, 130, 246, 255])), 4);
        // Fill inner square (checked state background)
        let inset = 2.0f32;
        let inner = Rect { x: cb1.x + inset, y: cb1.y + inset, w: (cb1.w - 2.0 * inset).max(0.0), h: (cb1.h - 2.0 * inset).max(0.0) };
        let inner_r = RoundedRadii { tl: 1.5, tr: 1.5, br: 1.5, bl: 1.5 };
        p.rounded_rect(RoundedRect { rect: inner, radii: inner_r }, Brush::Solid(Color::from_srgba_u8([63, 130, 246, 255])), 5);
        // Tick rendered in paint_text_overlay via SVG to ensure exact geometry
        p.text(engine_core::TextRun { text: "Checked + Focus".to_string(), pos: [col1_x + 188.0, y + 16.0], size: 16.0, color: Color::from_srgba_u8([240, 240, 240, 255]) }, 4);
        y += 42.0;

        // Radio buttons
        let r0c = [col1_x + 10.0, y + 12.0];
        p.ellipse(r0c, [9.0, 9.0], Brush::Solid(Color::from_srgba_u8([240, 240, 240, 255])), 3);
        p.text(engine_core::TextRun { text: "Option A".to_string(), pos: [col1_x + 28.0, y + 16.0], size: 16.0, color: Color::from_srgba_u8([240, 240, 240, 255]) }, 4);
        let r1c = [col1_x + 150.0, y + 12.0];
        p.ellipse(r1c, [9.0, 9.0], Brush::Solid(Color::from_srgba_u8([240, 240, 240, 255])), 3);
        p.ellipse(r1c, [5.0, 5.0], Brush::Solid(Color::from_srgba_u8([40, 120, 220, 255])), 5);
        p.text(engine_core::TextRun { text: "Option B (focus)".to_string(), pos: [col1_x + 168.0, y + 16.0], size: 16.0, color: Color::from_srgba_u8([240, 240, 240, 255]) }, 4);
        y += 42.0;

        // Input box
        let ib = Rect { x: col1_x, y, w: 300.0, h: 34.0 };
        p.rect(ib, Brush::Solid(Color::from_srgba_u8([255, 255, 255, 255])), 3);
        p.stroke_rect(ib, Stroke { width: 1.0 }, Brush::Solid(Color::from_srgba_u8([200, 200, 200, 255])), 4);
        p.text(engine_core::TextRun { text: "Type here...".to_string(), pos: [ib.x + 10.0, y + 22.0], size: 16.0, color: Color::from_srgba_u8([16, 16, 16, 255]) }, 5);
        y += 44.0;

        // Text area
        let ta = Rect { x: col1_x, y, w: 300.0, h: 110.0 };
        p.rect(ta, Brush::Solid(Color::from_srgba_u8([255, 255, 255, 255])), 3);
        p.stroke_rect(ta, Stroke { width: 1.0 }, Brush::Solid(Color::from_srgba_u8([200, 200, 200, 255])), 4);
        p.text(engine_core::TextRun { text: "Multi-line text area".to_string(), pos: [ta.x + 8.0, y + 20.0], size: 15.0, color: Color::from_srgba_u8([16, 16, 16, 255]) }, 5);
        p.text(engine_core::TextRun { text: "Line two wraps or clips".to_string(), pos: [ta.x + 8.0, y + 40.0], size: 15.0, color: Color::from_srgba_u8([16, 16, 16, 255]) }, 5);
        y += 120.0;

        // Selects
        let sel1 = Rect { x: col1_x, y, w: 220.0, h: 34.0 };
        p.rect(sel1, Brush::Solid(Color::from_srgba_u8([255, 255, 255, 255])), 3);
        p.stroke_rect(sel1, Stroke { width: 1.0 }, Brush::Solid(Color::from_srgba_u8([200, 200, 200, 255])), 4);
        p.text(engine_core::TextRun { text: "Select option".to_string(), pos: [sel1.x + 10.0, y + 22.0], size: 16.0, color: Color::from_srgba_u8([16, 16, 16, 255]) }, 5);
        // caret
        let mut tri = Path { cmds: Vec::new(), fill_rule: engine_core::FillRule::NonZero };
        let ax = sel1.x + sel1.w - 14.0; let ay = y + sel1.h * 0.5;
        tri.cmds.push(PathCmd::MoveTo([ax - 6.0, ay - 3.0]));
        tri.cmds.push(PathCmd::LineTo([ax, ay + 3.0]));
        tri.cmds.push(PathCmd::LineTo([ax + 6.0, ay - 3.0]));
        tri.cmds.push(PathCmd::Close);
        p.fill_path(tri, Color::from_srgba_u8([80, 80, 80, 255]), 6);

        let sel2 = Rect { x: col1_x + 240.0, y, w: 220.0, h: 34.0 };
        p.rect(sel2, Brush::Solid(Color::from_srgba_u8([255, 255, 255, 255])), 3);
        p.stroke_rect(sel2, Stroke { width: 2.0 }, Brush::Solid(Color::from_srgba_u8([63, 130, 246, 255])), 4);
        p.text(engine_core::TextRun { text: "Open + Focused".to_string(), pos: [sel2.x + 10.0, y + 22.0], size: 16.0, color: Color::from_srgba_u8([16, 16, 16, 255]) }, 5);
        let mut tri2 = Path { cmds: Vec::new(), fill_rule: engine_core::FillRule::NonZero };
        let ax2 = sel2.x + sel2.w - 14.0; let ay2 = y + sel2.h * 0.5;
        tri2.cmds.push(PathCmd::MoveTo([ax2 - 6.0, ay2 + 3.0]));
        tri2.cmds.push(PathCmd::LineTo([ax2, ay2 - 3.0]));
        tri2.cmds.push(PathCmd::LineTo([ax2 + 6.0, ay2 + 3.0]));
        tri2.cmds.push(PathCmd::Close);
        p.fill_path(tri2, Color::from_srgba_u8([80, 80, 80, 255]), 6);
        // Right column label + text
        p.text(engine_core::TextRun { text: "Text + Label".to_string(), pos: [col2_x, 40.0], size: 20.0, color: Color::from_srgba_u8([220, 230, 250, 255]) }, 2);
        p.text(engine_core::TextRun { text: "Label element".to_string(), pos: [col2_x, 40.0 + 36.0], size: 16.0, color: Color::from_srgba_u8([230, 230, 235, 255]) }, 3);
        p.text(engine_core::TextRun { text: "Plain Text element".to_string(), pos: [col2_x, 40.0 + 36.0 + 28.0], size: 16.0, color: Color::from_srgba_u8([210, 245, 210, 255]) }, 3);

        // Image placeholder
        let img = Rect { x: col2_x, y: 40.0 + 36.0 + 28.0 + 40.0, w: 180.0, h: 120.0 };
        p.rect(img, Brush::Solid(Color::from_srgba_u8([80, 160, 220, 255])), 3);
        p.text(engine_core::TextRun { text: "Image placeholder".to_string(), pos: [img.x, img.y + img.h + 20.0], size: 14.0, color: Color::from_srgba_u8([235, 240, 255, 200]) }, 4);

        p.finish()
    }
}

impl Scene for UiElementsScene {
    fn kind(&self) -> SceneKind { SceneKind::Geometry }

    fn init_display_list(&mut self, viewport: Viewport) -> Option<DisplayList> {
        self.viewport = viewport;
        Some(self.build())
    }
    fn on_resize(&mut self, viewport: Viewport) -> Option<DisplayList> {
        self.viewport = viewport;
        Some(self.build())
    }
    fn paint_root_background(
        &self,
        _passes: &mut engine_core::PassManager,
        _encoder: &mut wgpu::CommandEncoder,
        _surface_view: &wgpu::TextureView,
        _queue: &wgpu::Queue,
        _width: u32,
        _height: u32,
    ) {}

    fn paint_text_overlay(
        &self,
        passes: &mut engine_core::PassManager,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        queue: &wgpu::Queue,
        _width: u32,
        _height: u32,
        _provider_rgb: Option<&dyn engine_core::TextProvider>,
        _provider_bgr: Option<&dyn engine_core::TextProvider>,
        _provider_gray: Option<&dyn engine_core::TextProvider>,
    ) {
        // Recompute the checkbox geometry to draw the tick via SVG overlay.
        let col1_x = 40.0f32;
        let mut y = 40.0f32; // top margin
        y += 36.0; // title
        y += 46.0; // buttons row
        let cb_size = 18.0f32;
        let cb1_x = col1_x + 160.0;
        let cb1_y = y;
        let inset = 2.0f32;
        let inner_x = cb1_x + inset;
        let inner_y = cb1_y + inset;
        let inner_w = (cb_size - 2.0 * inset).max(0.0);
        let inner_h = (cb_size - 2.0 * inset).max(0.0);
        // Rasterize and draw the white SVG tick into the inner rect.
        if let Some((view, _sw, _sh)) = passes.rasterize_svg_to_view(std::path::Path::new("images/check_white.svg"), 1.0, queue) {
            passes.draw_image_quad(
                encoder,
                surface_view,
                [inner_x, inner_y],
                [inner_w, inner_h],
                &view,
                queue,
                self.viewport.width,
                self.viewport.height,
            );
        }
    }
}
