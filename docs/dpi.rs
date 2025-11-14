//! DPI utilities for Rune-Draw
//! Fetch OS DPI scale, keep uniform metrics,
//! convert between logical <-> physical coordinates,
//! and scale cosmic-text and wgpu shapes uniformly.

use winit::dpi::PhysicalSize;
use winit::window::Window;

use cosmic_text::{Buffer, FontSystem, Metrics as TextMetrics};

/// Unified metrics passed across your rendering stack.
#[derive(Debug, Clone, Copy)]
pub struct DeviceMetrics {
    /// OS logical->physical scale factor (winit-provided)
    pub scale_factor: f64,

    /// Scale factor as f32 for wgpu & shaders
    pub scale: f32,

    /// Same as scale; included for semantic clarity
    pub px_per_dp: f32,
}

/// Maintains DPI state and updates it when the OS changes scale.
#[derive(Debug)]
pub struct DpiState {
    pub metrics: DeviceMetrics,
}

impl DpiState {
    /// Create on startup.
    pub fn new(window: &Window) -> Self {
        Self {
            metrics: compute_device_metrics(window),
        }
    }

    /// Update whenever `ScaleFactorChanged` or resize occurs.
    pub fn update(&mut self, window: &Window) {
        self.metrics = compute_device_metrics(window);
    }
}

/// Returns system DPI scale (logical→physical). Cross-platform and correct per winit.
pub fn compute_device_metrics(window: &Window) -> DeviceMetrics {
    let scale_factor = window.scale_factor(); // f64
    let scale = scale_factor as f32;

    DeviceMetrics {
        scale_factor,
        scale,
        px_per_dp: scale,
    }
}

/// Convert logical coordinates into device pixels.
/// Always use these when writing geometry into GPU buffers.
#[inline]
pub fn to_physical(x: f32, y: f32, metrics: &DeviceMetrics) -> (f32, f32) {
    (x * metrics.scale, y * metrics.scale)
}

/// Scale a size from logical px → physical px.
#[inline]
pub fn size_to_physical(v: f32, metrics: &DeviceMetrics) -> f32 {
    v * metrics.scale
}

/// Create a cosmic-text buffer scaled for current DPI.
pub fn create_text_buffer(
    font_system: &mut FontSystem,
    base_font_px: f32,
    base_line_px: f32,
    metrics: &DeviceMetrics,
) -> Buffer {
    let mut buf = Buffer::new(
        font_system,
        TextMetrics::new(base_font_px * metrics.scale, base_line_px * metrics.scale),
    );
    buf
}

/// Update a cosmic-text buffer after DPI change.
pub fn update_text_buffer_dpi(
    buffer: &mut Buffer,
    base_font_px: f32,
    base_line_px: f32,
    metrics: &DeviceMetrics,
) {
    buffer.set_metrics(TextMetrics::new(
        base_font_px * metrics.scale,
        base_line_px * metrics.scale,
    ));
}

/// Optional: Get true monitor DPI (not just scale factor).
/// Some systems report 0 for size_mm, so fallback is needed.
pub fn get_monitor_dpi(window: &Window) -> Option<f64> {
    window.current_monitor().and_then(|m| {
        let size_px: PhysicalSize<u32> = m.size();
        let size_mm = m.size(); // Some platforms report mm here; winit unifies size().

        // Avoid divide by zero
        if size_mm.width > 0 && size_mm.height > 0 {
            let dpi_x = size_px.width as f64 / (size_mm.width as f64 / 25.4);
            Some(dpi_x)
        } else {
            None
        }
    })
}

//
// ---------- EXAMPLES OF USING THIS MODULE ----------
//

/// Example: Convert rect before pushing into wgpu vertex buffer
pub fn example_shape_usage(
    logical_x: f32,
    logical_y: f32,
    logical_w: f32,
    logical_h: f32,
    dpi: &DeviceMetrics,
) -> (f32, f32, f32, f32) {
    let (x, y) = to_physical(logical_x, logical_y, dpi);
    let w = size_to_physical(logical_w, dpi);
    let h = size_to_physical(logical_h, dpi);

    (x, y, w, h)
}

/// Example: Setting cosmic-text after DPI change
pub fn example_update_text(
    buffer: &mut Buffer,
    base_font_px: f32,
    base_line_px: f32,
    dpi: &DeviceMetrics,
) {
    update_text_buffer_dpi(buffer, base_font_px, base_line_px, dpi);
}

//
// ---------------- HOW TO INTEGRATE IN RUNE-DRAW ----------------
//
// In your renderer:
//
//   let mut dpi_state = DpiState::new(&window);
//
// On event:
//
//   WindowEvent::ScaleFactorChanged { scale_factor, new_inner_size } => {
//       dpi_state.update(&window);
//       resize_swapchain();
//       update_cosmic_text_buffers();
//       redraw();
//   }
//
// When drawing wgpu shapes:
//   let (x, y, w, h) = example_shape_usage(x, y, w, h, &dpi_state.metrics);
//
// When using cosmic-text:
//   let mut buf = create_text_buffer(&mut font_system, 16.0, 18.0, &dpi_state.metrics);
//
