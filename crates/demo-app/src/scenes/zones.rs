use super::{Scene, SceneKind};
use engine_core::{
    Brush, ColorLinPremul, DisplayList, HitResult, Painter, Rect, RoundedRadii, RoundedRect,
    Stroke, Viewport,
};

pub struct ZonesScene {
    viewport: Viewport,
    // last click feedback
    last_region: Option<u32>,
    last_local: Option<[f32; 2]>,
}

impl Default for ZonesScene {
    fn default() -> Self {
        Self {
            viewport: Viewport {
                width: 1280,
                height: 720,
            },
            last_region: None,
            last_local: None,
        }
    }
}

impl ZonesScene {
    // Region IDs
    const SIDEBAR_RED: u32 = 2001;
    const SIDEBAR_BLUE: u32 = 2002;
    const MAIN_RED: u32 = 2101;
    const MAIN_BLUE: u32 = 2102;

    fn layout(&self) -> (Rect, Rect) {
        // two columns: sidebar fixed, main fills
        let sidebar_w = 280.0f32;
        let sidebar = Rect {
            x: 0.0,
            y: 0.0,
            w: sidebar_w,
            h: self.viewport.height as f32,
        };
        let main = Rect {
            x: sidebar_w,
            y: 0.0,
            w: (self.viewport.width as f32 - sidebar_w).max(0.0),
            h: self.viewport.height as f32,
        };
        (sidebar, main)
    }

    fn zone_rects(&self) -> [(u32, Rect); 4] {
        let (sidebar, main) = self.layout();
        // Sidebar zones with margins
        let margin = 20.0f32;
        let zone_w = (sidebar.w - margin * 2.0).max(20.0);
        let zone_h = 120.0f32;
        let sb_red = Rect {
            x: sidebar.x + margin,
            y: sidebar.y + margin,
            w: zone_w,
            h: zone_h,
        };
        let sb_blue = Rect {
            x: sidebar.x + margin,
            y: sidebar.y + margin * 2.0 + zone_h,
            w: zone_w,
            h: zone_h,
        };

        // Main panel zones
        let main_margin = 40.0f32;
        let main_zone_w = (main.w - main_margin * 2.0).max(40.0);
        let main_zone_h = 160.0f32;
        let main_red = Rect {
            x: main.x + main_margin,
            y: main.y + main_margin,
            w: main_zone_w,
            h: main_zone_h,
        };
        let main_blue = Rect {
            x: main.x + main_margin,
            y: main.y + main_margin * 2.0 + main_zone_h,
            w: main_zone_w,
            h: main_zone_h,
        };

        [
            (Self::SIDEBAR_RED, sb_red),
            (Self::SIDEBAR_BLUE, sb_blue),
            (Self::MAIN_RED, main_red),
            (Self::MAIN_BLUE, main_blue),
        ]
    }

    fn build_display_list(&self) -> DisplayList {
        let mut p = Painter::begin_frame(self.viewport);

        // Base background white
        let white = ColorLinPremul::from_srgba_u8([0xff, 0xff, 0xff, 0xff]);
        p.rect(
            Rect {
                x: 0.0,
                y: 0.0,
                w: self.viewport.width as f32,
                h: self.viewport.height as f32,
            },
            Brush::Solid(white),
            0,
        );

        // Columns
        let (sidebar, main) = self.layout();
        let sidebar_bg = ColorLinPremul::from_srgba_u8([0xf5, 0xf6, 0xf7, 0xff]);
        p.rect(sidebar, Brush::Solid(sidebar_bg), 1);
        // Optional separation line (stroke as thin rect)
        p.rect(
            Rect {
                x: sidebar.x + sidebar.w - 1.0,
                y: 0.0,
                w: 1.0,
                h: sidebar.h,
            },
            Brush::Solid(ColorLinPremul::from_srgba_u8([0xdd, 0xdd, 0xdd, 0xff])),
            2,
        );

        // Zones (red/blue) and their hit regions
        for (id, rect) in self.zone_rects() {
            let fill = match id {
                Self::SIDEBAR_RED | Self::MAIN_RED => {
                    ColorLinPremul::from_srgba_u8([0xf9, 0x2f, 0x26, 0xff])
                }
                _ => ColorLinPremul::from_srgba_u8([0x3b, 0x82, 0xf6, 0xff]),
            };
            let rr = RoundedRect {
                rect,
                radii: RoundedRadii {
                    tl: 10.0,
                    tr: 10.0,
                    br: 10.0,
                    bl: 10.0,
                },
            };
            p.rounded_rect(rr, Brush::Solid(fill), 3);
            p.stroke_rounded_rect(
                rr,
                Stroke { width: 2.0 },
                Brush::Solid(ColorLinPremul::from_srgba(255, 255, 255, 0.6)),
                4,
            );
            p.hit_region_rect(id, rect, 10);
        }

        // Click feedback: draw a small marker inside the clicked zone using local coordinates
        if let (Some(region), Some(local)) = (self.last_region, self.last_local) {
            // Resolve region rect to world
            if let Some((_, r)) = self
                .zone_rects()
                .into_iter()
                .find(|(rid, _)| *rid == region)
            {
                let world = [r.x + local[0], r.y + local[1]];
                let radius = 6.0f32;
                p.ellipse(
                    world,
                    [radius, radius],
                    Brush::Solid(ColorLinPremul::from_srgba_u8([0x00, 0x00, 0x00, 0xff])),
                    20,
                );
            }
        }

        p.finish()
    }
}

impl Scene for ZonesScene {
    fn kind(&self) -> SceneKind {
        SceneKind::Geometry
    }

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
    }

    fn on_click(&mut self, _pos: [f32; 2], hit: Option<&HitResult>) -> Option<DisplayList> {
        if let Some(h) = hit {
            if let Some(region_id) = h.region_id {
                // Only handle our region ids
                match region_id {
                    Self::SIDEBAR_RED | Self::SIDEBAR_BLUE | Self::MAIN_RED | Self::MAIN_BLUE => {
                        if let Some(local) = h.local_pos {
                            self.last_region = Some(region_id);
                            self.last_local = Some(local);
                            return Some(self.build_display_list());
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }
}
