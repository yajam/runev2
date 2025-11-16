use super::{Scene, SceneKind};
use engine_core::{DisplayList, Painter, Viewport, Brush, ColorLinPremul, Rect, Transform2D};

pub struct SvgGeomScene {
    paths: Vec<std::path::PathBuf>,
}

impl Default for SvgGeomScene { fn default() -> Self { Self { paths: Vec::new() } } }

impl SvgGeomScene {
    fn find_svg_paths() -> Vec<std::path::PathBuf> {
        let mut dirs = vec![std::path::PathBuf::from("images"), std::path::PathBuf::from("crates/demo-app/images")];
        let mut out = Vec::new();
        for dir in dirs.drain(..) {
            if let Ok(read) = std::fs::read_dir(&dir) {
                for ent in read.flatten() {
                    let p = ent.path();
                    let ext = p.extension().and_then(|e| e.to_str()).map(|s| s.to_ascii_lowercase());
                    if matches!(ext.as_deref(), Some("svg")) { out.push(p); }
                }
            }
        }
        out
    }
}

impl Scene for SvgGeomScene {
    fn kind(&self) -> SceneKind { SceneKind::Geometry }

    fn init_display_list(&mut self, viewport: Viewport) -> Option<DisplayList> {
        // Discover SVGs once
        if self.paths.is_empty() { self.paths = Self::find_svg_paths(); }

        let mut p = Painter::begin_frame(viewport);

        // Clear background with a subtle dark tone
        let bg = ColorLinPremul::from_srgba(18, 22, 30, 1.0);
        p.rect(Rect { x: 0.0, y: 0.0, w: viewport.width as f32, h: viewport.height as f32 }, Brush::Solid(bg), -1000);

        if self.paths.is_empty() { return Some(p.finish()); }

        // Grid layout similar to images scene
        let n = self.paths.len();
        let cols = (n as f32).sqrt().ceil() as usize;
        let cols = cols.max(1);
        let rows = ((n + cols - 1) / cols).max(1);
        let margin = 16.0f32;
        let cell_w = ((viewport.width as f32) - margin * ((cols + 1) as f32)) / (cols as f32);
        let cell_h = ((viewport.height as f32) - margin * ((rows + 1) as f32)) / (rows as f32);

        for (i, path) in self.paths.iter().enumerate() {
            let r = i / cols;
            let c = i % cols;
            let x0 = margin + c as f32 * (cell_w + margin);
            let y0 = margin + r as f32 * (cell_h + margin);

            // Query intrinsic size to compute scale
            let (w, h) = engine_core::svg_intrinsic_size(path).unwrap_or((128, 128));
            let iw = w as f32;
            let ih = h as f32;
            let scale = (cell_w / iw).min(cell_h / ih).max(0.0);
            let draw_w = (iw * scale).max(1.0);
            let draw_h = (ih * scale).max(1.0);
            let ox = x0 + (cell_w - draw_w) * 0.5;
            let oy = y0 + (cell_h - draw_h) * 0.5;

            // Draw a faint background cell
            let cell_bg = ColorLinPremul::from_srgba(255, 255, 255, 0.03);
            p.rect(Rect { x: x0, y: y0, w: cell_w, h: cell_h }, Brush::Solid(cell_bg), -10);

            // Apply translate+scale transform and import geometry
            let t = Transform2D { m: [scale, 0.0, 0.0, scale, ox, oy] };
            p.push_transform(t);
            if let Some(_stats) = engine_core::import_svg_geometry_to_painter(&mut p, path) {
            }
            p.pop_transform();
        }

        Some(p.finish())
    }

    fn paint_root_background(
        &self,
        _passes: &mut engine_core::PassManager,
        _encoder: &mut wgpu::CommandEncoder,
        _surface_view: &wgpu::TextureView,
        _queue: &wgpu::Queue,
        _width: u32,
        _height: u32,
    ) { }
}
