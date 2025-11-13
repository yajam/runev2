use anyhow::Result;
use engine_core::{Brush, ColorLinPremul};
use rune_surface::RuneSurface;
use rune_window::{events::RuneWindowEvent, EventHandler, RuneWindow, WindowCtx};

pub mod elements;
pub mod text;

struct SimpleScene {
    surface: Option<RuneSurface>,
    bg: ColorLinPremul,
}

impl Default for SimpleScene {
    fn default() -> Self {
        Self { surface: None, bg: ColorLinPremul::from_lin_rgba(0.1, 0.12, 0.2, 1.0) }
    }
}

impl EventHandler for SimpleScene {
    fn init(&mut self, ctx: &mut WindowCtx) -> Result<()> {
        let format = ctx.surface_config().format;
        self.surface = Some(RuneSurface::new(ctx.device_arc(), ctx.queue_arc(), format));
        ctx.request_redraw();
        Ok(())
    }
    fn on_event(&mut self, _ctx: &mut WindowCtx, _event: RuneWindowEvent) -> Result<()> { Ok(()) }
    fn on_resize(&mut self, ctx: &mut WindowCtx, _size: winit::dpi::PhysicalSize<u32>) -> Result<()> { ctx.request_redraw(); Ok(()) }
    fn on_redraw(&mut self, ctx: &mut WindowCtx) -> Result<()> {
        let Some(surface) = self.surface.as_mut() else { return Ok(()); };
        let frame = ctx.acquire_current_frame()?;
        let size = ctx.size();
        let mut canvas = surface.begin_frame(size.width, size.height);
        canvas.clear(self.bg);
        canvas.fill_rect(
            0.0, 0.0, size.width as f32, size.height as f32,
            Brush::Solid(ColorLinPremul::from_lin_rgba(0.2, 0.5, 0.8, 1.0)),
            0,
        );
        surface.end_frame(frame, canvas)?;
        Ok(())
    }
}

pub fn run() -> Result<()> {
    let win = RuneWindow::new("Rune Scene")?;
    win.run(SimpleScene::default())
}
