use engine_core::{Brush, ColorLinPremul, Rect, RoundedRadii, RoundedRect, Viewport, Painter, DisplayList, HitResult, HitKind};
use super::{Scene, SceneKind};

pub struct OverlayScene {
    viewport: Viewport,
    is_open: bool,
}

impl Default for OverlayScene {
    fn default() -> Self {
        Self {
            viewport: Viewport { width: 1280, height: 720 },
            is_open: false,
        }
    }
}

impl OverlayScene {
    const OPEN_REGION: u32 = 1001;
    const CLOSE_REGION: u32 = 1002;

    fn build_display_list(&self) -> DisplayList {
        let mut p = Painter::begin_frame(self.viewport);

        // Base background color: white
        let bg = ColorLinPremul::from_srgba_u8([0xff, 0xff, 0xff, 0xff]);
        p.rect(
            Rect { x: 0.0, y: 0.0, w: self.viewport.width as f32, h: self.viewport.height as f32 },
            Brush::Solid(bg),
            0,
        );

        if self.is_open {
            // Dim scrim over the scene
            let scrim = ColorLinPremul::from_srgba(0, 0, 0, 0.45);
            p.rect(
                Rect { x: 0.0, y: 0.0, w: self.viewport.width as f32, h: self.viewport.height as f32 },
                Brush::Solid(scrim),
                10,
            );

            // Centered overlay panel
            let panel_w = 420.0;
            let panel_h = 300.0;
            let x = (self.viewport.width as f32 - panel_w) * 0.5;
            let y = (self.viewport.height as f32 - panel_h) * 0.5;
            let panel = RoundedRect {
                rect: Rect { x, y, w: panel_w, h: panel_h },
                radii: RoundedRadii { tl: 14.0, tr: 14.0, br: 14.0, bl: 14.0 },
            };
            let panel_fill = ColorLinPremul::from_srgba_u8([0xff, 0xff, 0xff, 0xff]);
            // A close-zone hit region above scrim and below panel
            p.hit_region_rect(Self::CLOSE_REGION, Rect { x: 0.0, y: 0.0, w: self.viewport.width as f32, h: self.viewport.height as f32 }, 15);
            // Panel on top
            p.rounded_rect(panel, Brush::Solid(panel_fill), 20);
        }

        if !self.is_open {
            // Clickable rectangle to open the modal
            let bw = 220.0;
            let bh = 80.0;
            let bx = (self.viewport.width as f32 - bw) * 0.5;
            let by = (self.viewport.height as f32 - bh) * 0.5;
            let r = RoundedRect {
                rect: Rect { x: bx, y: by, w: bw, h: bh },
                radii: RoundedRadii { tl: 10.0, tr: 10.0, br: 10.0, bl: 10.0 },
            };
            let fill = ColorLinPremul::from_srgba_u8([0xee, 0xee, 0xee, 0xff]);
            p.rounded_rect(r, Brush::Solid(fill), 2);
            // Topmost hit region for the button
            p.hit_region_rounded_rect(Self::OPEN_REGION, r, 5);
        }

        p.finish()
    }
}

impl Scene for OverlayScene {
    fn kind(&self) -> SceneKind { SceneKind::Geometry }

    fn init_display_list(&mut self, viewport: Viewport) -> Option<DisplayList> {
        self.viewport = viewport;
        Some(self.build_display_list())
    }

    fn on_resize(&mut self, viewport: Viewport) -> Option<DisplayList> {
        self.viewport = viewport;
        Some(self.build_display_list())
    }

    fn paint_root_background(
        &self,
        _passes: &mut engine_core::PassManager,
        _encoder: &mut wgpu::CommandEncoder,
        _surface_view: &wgpu::TextureView,
        _queue: &wgpu::Queue,
        _width: u32,
        _height: u32,
    ) {
        // Background is drawn via display list above.
    }

    fn on_click(&mut self, _pos: [f32; 2], hit: Option<&HitResult>) -> Option<DisplayList> {
        if let Some(h) = hit {
            if !self.is_open {
                // Open when clicking the button region
                if matches!(h.kind, HitKind::HitRegion) && h.region_id == Some(Self::OPEN_REGION) {
                    self.is_open = true;
                    return Some(self.build_display_list());
                }
            } else {
                // Close when clicking outside the panel (close-zone) or the fallback root region
                if matches!(h.kind, HitKind::HitRegion)
                    && (h.region_id == Some(Self::CLOSE_REGION) || h.region_id == Some(u32::MAX))
                {
                    self.is_open = false;
                    return Some(self.build_display_list());
                }
            }
        }
        None
    }
}
