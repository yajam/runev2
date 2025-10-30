use engine_core::{Brush, ColorLinPremul, Rect, RoundedRadii, RoundedRect, Stroke};
use engine_core::{DisplayList, Painter, PassManager, Viewport};

use super::{Scene, SceneKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToggleId {
    RGB = 1,
    BGR = 2,
    ColWhite = 10,
    ColYellow = 11,
    ColCyan = 12,
    SizeSmall = 20,
    SizeMedium = 21,
    SizeLarge = 22,
    SizeXLarge = 23,
}

pub struct TextDemoScene {
    pub orientation: engine_core::SubpixelOrientation,
    pub text_color: ColorLinPremul,
    pub text_size: f32,
    last_viewport: Option<Viewport>,
}

impl Default for TextDemoScene {
    fn default() -> Self {
        // Optional env override for initial size
        let size_env = std::env::var("DEMO_TEXT_SIZE").ok().and_then(|s| s.parse::<f32>().ok());
        Self {
            orientation: engine_core::SubpixelOrientation::RGB,
            text_color: ColorLinPremul::from_srgba_u8([255, 255, 255, 255]),
            text_size: size_env.unwrap_or(28.0),
            last_viewport: None,
        }
    }
}

impl Scene for TextDemoScene {
    fn kind(&self) -> SceneKind {
        SceneKind::Geometry
    }

    fn init_display_list(&mut self, viewport: Viewport) -> Option<DisplayList> {
        // Track viewport for future rebuilds
        self.last_viewport = Some(viewport);
        // Build UI with two toggle buttons and section backgrounds
        let mut p = Painter::begin_frame(viewport);
        // Background cards
        let left = Rect {
            x: 40.0,
            y: 120.0,
            w: (viewport.width as f32) * 0.5 - 60.0,
            h: viewport.height as f32 - 160.0,
        };
        let right = Rect {
            x: (viewport.width as f32) * 0.5 + 20.0,
            y: 120.0,
            w: (viewport.width as f32) * 0.5 - 60.0,
            h: viewport.height as f32 - 160.0,
        };
        let r = RoundedRect {
            rect: left,
            radii: RoundedRadii {
                tl: 12.0,
                tr: 12.0,
                br: 12.0,
                bl: 12.0,
            },
        };
        p.rounded_rect(
            r,
            Brush::Solid(ColorLinPremul::from_srgba(255, 255, 255, 0.04)),
            0,
        );
        let r2 = RoundedRect {
            rect: right,
            radii: RoundedRadii {
                tl: 12.0,
                tr: 12.0,
                br: 12.0,
                bl: 12.0,
            },
        };
        p.rounded_rect(
            r2,
            Brush::Solid(ColorLinPremul::from_srgba(255, 255, 255, 0.04)),
            0,
        );

        // Toggle buttons at top-right (RGB/BGR)
        let bx = viewport.width as f32 - 220.0;
        let by = 40.0;
        let btn_size = [80.0, 36.0];
        let rgb_rect = Rect {
            x: bx,
            y: by,
            w: btn_size[0],
            h: btn_size[1],
        };
        let bgr_rect = Rect {
            x: bx + 100.0,
            y: by,
            w: btn_size[0],
            h: btn_size[1],
        };
        let active = match self.orientation {
            engine_core::SubpixelOrientation::RGB => ToggleId::RGB,
            engine_core::SubpixelOrientation::BGR => ToggleId::BGR,
        };
        let on = ColorLinPremul::from_srgba_u8([0x52, 0xA7, 0xE8, 255]);
        let off = ColorLinPremul::from_srgba(255, 255, 255, 0.10);
        p.rounded_rect(
            RoundedRect {
                rect: rgb_rect,
                radii: RoundedRadii {
                    tl: 8.0,
                    tr: 8.0,
                    br: 8.0,
                    bl: 8.0,
                },
            },
            Brush::Solid(if active == ToggleId::RGB { on } else { off }),
            1,
        );
        p.rounded_rect(
            RoundedRect {
                rect: bgr_rect,
                radii: RoundedRadii {
                    tl: 8.0,
                    tr: 8.0,
                    br: 8.0,
                    bl: 8.0,
                },
            },
            Brush::Solid(if active == ToggleId::BGR { on } else { off }),
            1,
        );
        // Hit regions for toggles
        p.hit_region_rect(ToggleId::RGB as u32, rgb_rect, 2);
        p.hit_region_rect(ToggleId::BGR as u32, bgr_rect, 2);

        // Color toggles (top-left)
        let cx = 40.0;
        let cy = 40.0;
        let csize = [36.0, 36.0];
        let col_white = Rect {
            x: cx,
            y: cy,
            w: csize[0],
            h: csize[1],
        };
        let col_yellow = Rect {
            x: cx + 46.0,
            y: cy,
            w: csize[0],
            h: csize[1],
        };
        let col_cyan = Rect {
            x: cx + 92.0,
            y: cy,
            w: csize[0],
            h: csize[1],
        };
        let (cw, cyw, cc) = (
            ColorLinPremul::from_srgba_u8([255, 255, 255, 255]),
            ColorLinPremul::from_srgba_u8([255, 240, 120, 255]),
            ColorLinPremul::from_srgba_u8([120, 240, 255, 255]),
        );
        p.rounded_rect(
            RoundedRect {
                rect: col_white,
                radii: RoundedRadii {
                    tl: 6.0,
                    tr: 6.0,
                    br: 6.0,
                    bl: 6.0,
                },
            },
            Brush::Solid(cw),
            1,
        );
        p.rounded_rect(
            RoundedRect {
                rect: col_yellow,
                radii: RoundedRadii {
                    tl: 6.0,
                    tr: 6.0,
                    br: 6.0,
                    bl: 6.0,
                },
            },
            Brush::Solid(cyw),
            1,
        );
        p.rounded_rect(
            RoundedRect {
                rect: col_cyan,
                radii: RoundedRadii {
                    tl: 6.0,
                    tr: 6.0,
                    br: 6.0,
                    bl: 6.0,
                },
            },
            Brush::Solid(cc),
            1,
        );
        // Active stroke
        let stroke = Stroke { width: 2.0 };
        let ac = self.text_color;
        let is_white = (ac.r, ac.g, ac.b, ac.a) == (cw.r, cw.g, cw.b, cw.a);
        let is_yellow = (ac.r, ac.g, ac.b, ac.a) == (cyw.r, cyw.g, cyw.b, cyw.a);
        let white_stroke = Brush::Solid(ColorLinPremul::from_srgba_u8([30, 30, 30, 255]));
        if is_white {
            p.stroke_rounded_rect(
                RoundedRect {
                    rect: col_white,
                    radii: RoundedRadii {
                        tl: 6.0,
                        tr: 6.0,
                        br: 6.0,
                        bl: 6.0,
                    },
                },
                stroke,
                white_stroke.clone(),
                2,
            );
        }
        if is_yellow {
            p.stroke_rounded_rect(
                RoundedRect {
                    rect: col_yellow,
                    radii: RoundedRadii {
                        tl: 6.0,
                        tr: 6.0,
                        br: 6.0,
                        bl: 6.0,
                    },
                },
                stroke,
                white_stroke.clone(),
                2,
            );
        }
        if !is_white && !is_yellow {
            p.stroke_rounded_rect(
                RoundedRect {
                    rect: col_cyan,
                    radii: RoundedRadii {
                        tl: 6.0,
                        tr: 6.0,
                        br: 6.0,
                        bl: 6.0,
                    },
                },
                stroke,
                white_stroke,
                2,
            );
        }
        // Hit regions
        p.hit_region_rect(ToggleId::ColWhite as u32, col_white, 2);
        p.hit_region_rect(ToggleId::ColYellow as u32, col_yellow, 2);
        p.hit_region_rect(ToggleId::ColCyan as u32, col_cyan, 2);

        // Size toggles (below color)
        let sy = cy + 50.0;
        let sbtn = [48.0, 32.0];
        let sz_small = Rect {
            x: cx,
            y: sy,
            w: sbtn[0],
            h: sbtn[1],
        };
        let sz_medium = Rect {
            x: cx + 58.0,
            y: sy,
            w: sbtn[0],
            h: sbtn[1],
        };
        let sz_large = Rect {
            x: cx + 116.0,
            y: sy,
            w: sbtn[0],
            h: sbtn[1],
        };
        let sz_xl = Rect {
            x: cx + 174.0,
            y: sy,
            w: sbtn[0],
            h: sbtn[1],
        };
        let on2 = ColorLinPremul::from_srgba(255, 255, 255, 0.18);
        let off2 = ColorLinPremul::from_srgba(255, 255, 255, 0.08);
        let active_size = if (self.text_size - 16.0).abs() < 0.5 {
            ToggleId::SizeSmall
        } else if (self.text_size - 22.0).abs() < 0.5 {
            ToggleId::SizeMedium
        } else if (self.text_size - 28.0).abs() < 0.5 {
            ToggleId::SizeLarge
        } else if (self.text_size - 48.0).abs() < 0.5 {
            ToggleId::SizeXLarge
        } else {
            ToggleId::SizeLarge
        };
        p.rounded_rect(
            RoundedRect {
                rect: sz_small,
                radii: RoundedRadii {
                    tl: 6.0,
                    tr: 6.0,
                    br: 6.0,
                    bl: 6.0,
                },
            },
            Brush::Solid(if active_size == ToggleId::SizeSmall {
                on2
            } else {
                off2
            }),
            1,
        );
        p.rounded_rect(
            RoundedRect {
                rect: sz_medium,
                radii: RoundedRadii {
                    tl: 6.0,
                    tr: 6.0,
                    br: 6.0,
                    bl: 6.0,
                },
            },
            Brush::Solid(if active_size == ToggleId::SizeMedium {
                on2
            } else {
                off2
            }),
            1,
        );
        p.rounded_rect(
            RoundedRect {
                rect: sz_large,
                radii: RoundedRadii {
                    tl: 6.0,
                    tr: 6.0,
                    br: 6.0,
                    bl: 6.0,
                },
            },
            Brush::Solid(if active_size == ToggleId::SizeLarge {
                on2
            } else {
                off2
            }),
            1,
        );
        p.rounded_rect(
            RoundedRect {
                rect: sz_xl,
                radii: RoundedRadii { tl: 6.0, tr: 6.0, br: 6.0, bl: 6.0 },
            },
            Brush::Solid(if active_size == ToggleId::SizeXLarge { on2 } else { off2 }),
            1,
        );
        p.hit_region_rect(ToggleId::SizeSmall as u32, sz_small, 2);
        p.hit_region_rect(ToggleId::SizeMedium as u32, sz_medium, 2);
        p.hit_region_rect(ToggleId::SizeLarge as u32, sz_large, 2);
        p.hit_region_rect(ToggleId::SizeXLarge as u32, sz_xl, 2);

        Some(p.finish())
    }

    fn on_resize(&mut self, viewport: Viewport) -> Option<DisplayList> {
        self.init_display_list(viewport)
    }

    fn paint_root_background(
        &self,
        passes: &mut PassManager,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
    ) {
        // Soft gradient background
        passes.paint_root_linear_gradient_multi(
            encoder,
            surface_view,
            [0.0, 0.0],
            [1.0, 1.0],
            &[
                (0.0, ColorLinPremul::from_srgba_u8([32, 36, 44, 255])),
                (1.0, ColorLinPremul::from_srgba_u8([16, 18, 22, 255])),
            ],
            queue,
        );
    }

    fn on_click(
        &mut self,
        _pos: [f32; 2],
        hit: Option<&engine_core::HitResult>,
    ) -> Option<DisplayList> {
        if let Some(h) = hit {
            match h.region_id {
                Some(id) if id == ToggleId::RGB as u32 => {
                    self.orientation = engine_core::SubpixelOrientation::RGB;
                    // Rebuild UI to reflect toggle state
                    if let Some(vp) = self.last_viewport {
                        return self.init_display_list(vp);
                    }
                }
                Some(id) if id == ToggleId::BGR as u32 => {
                    self.orientation = engine_core::SubpixelOrientation::BGR;
                    if let Some(vp) = self.last_viewport {
                        return self.init_display_list(vp);
                    }
                }
                Some(id) if id == ToggleId::ColWhite as u32 => {
                    self.text_color = ColorLinPremul::from_srgba_u8([255, 255, 255, 255]);
                    if let Some(vp) = self.last_viewport {
                        return self.init_display_list(vp);
                    }
                }
                Some(id) if id == ToggleId::ColYellow as u32 => {
                    self.text_color = ColorLinPremul::from_srgba_u8([255, 240, 120, 255]);
                    if let Some(vp) = self.last_viewport {
                        return self.init_display_list(vp);
                    }
                }
                Some(id) if id == ToggleId::ColCyan as u32 => {
                    self.text_color = ColorLinPremul::from_srgba_u8([120, 240, 255, 255]);
                    if let Some(vp) = self.last_viewport {
                        return self.init_display_list(vp);
                    }
                }
                Some(id) if id == ToggleId::SizeSmall as u32 => {
                    self.text_size = 16.0;
                    if let Some(vp) = self.last_viewport {
                        return self.init_display_list(vp);
                    }
                }
                Some(id) if id == ToggleId::SizeMedium as u32 => {
                    self.text_size = 22.0;
                    if let Some(vp) = self.last_viewport {
                        return self.init_display_list(vp);
                    }
                }
                Some(id) if id == ToggleId::SizeLarge as u32 => {
                    self.text_size = 28.0;
                    if let Some(vp) = self.last_viewport { return self.init_display_list(vp); }
                }
                Some(id) if id == ToggleId::SizeXLarge as u32 => {
                    self.text_size = 48.0;
                    if let Some(vp) = self.last_viewport { return self.init_display_list(vp); }
                }
                _ => {}
            }
        }
        None
    }

    fn paint_text_overlay(
        &self,
        passes: &mut PassManager,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        provider_rgb: Option<&dyn engine_core::TextProvider>,
        provider_bgr: Option<&dyn engine_core::TextProvider>,
        provider_gray: Option<&dyn engine_core::TextProvider>,
    ) {
        let vp = Viewport { width, height };
        // Left column: grayscale
        if let Some(pgray) = provider_gray {
            let mut p = Painter::begin_frame(vp);
            p.text(
                engine_core::TextRun {
                    text: "Grayscale AA".to_string(),
                    pos: [60.0, 90.0],
                    size: 20.0,
                    color: ColorLinPremul::from_srgba_u8([220, 220, 220, 255]),
                },
                2,
            );
            p.text(
                engine_core::TextRun {
                    text: SAMPLE_TEXT1.to_string(),
                    pos: [60.0, 160.0],
                    size: self.text_size,
                    color: ColorLinPremul::from_srgba_u8([255, 255, 255, 255]),
                },
                2,
            );
            p.text(
                engine_core::TextRun {
                    text: SAMPLE_TEXT2.to_string(),
                    pos: [60.0, 210.0],
                    size: (self.text_size * 0.64).max(10.0),
                    color: ColorLinPremul::from_srgba_u8([255, 255, 255, 200]),
                },
                2,
            );
            let dl = p.finish();
            passes.render_text_for_list(encoder, surface_view, &dl, queue, pgray);
        }
        // Right column: subpixel (RGB/BGR toggle)
        let px = (width as f32) * 0.5 + 40.0;
        let provider = match self.orientation {
            engine_core::SubpixelOrientation::RGB => provider_rgb,
            engine_core::SubpixelOrientation::BGR => provider_bgr,
        };
        // Use a small fractional X offset to make subpixel orientation differences easier to see
        let frac_off: f32 = std::env::var("DEMO_SUBPIXEL_OFFSET")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.33);
        if let Some(psub) = provider {
            let mut p = Painter::begin_frame(vp);
            let label = match self.orientation {
                engine_core::SubpixelOrientation::RGB => "Subpixel AA (RGB)",
                engine_core::SubpixelOrientation::BGR => "Subpixel AA (BGR)",
            };
            p.text(
                engine_core::TextRun {
                    text: label.to_string(),
                    pos: [px + frac_off, 90.0],
                    size: 20.0,
                    color: ColorLinPremul::from_srgba_u8([220, 220, 220, 255]),
                },
                2,
            );
            p.text(
                engine_core::TextRun {
                    text: SAMPLE_TEXT1.to_string(),
                    pos: [px + frac_off, 160.0],
                    size: self.text_size,
                    color: self.text_color,
                },
                2,
            );
            p.text(
                engine_core::TextRun {
                    text: SAMPLE_TEXT2.to_string(),
                    pos: [px + frac_off, 210.0],
                    size: (self.text_size * 0.64).max(10.0),
                    color: self.text_color,
                },
                2,
            );
            let dl = p.finish();
            passes.render_text_for_list(encoder, surface_view, &dl, queue, psub);
        }
    }
}

const SAMPLE_TEXT1: &str = "The quick brown fox jumps over the lazy dog 0123456789";
const SAMPLE_TEXT2: &str = "Aa Ee Ii Oo Uu Mm Ww — Visualize fringe/contrast and stem clarity";

// Utility to satisfy compiler (we return None in on_click so no rebuild requested). No extra helpers.
