//! rune-surface: Canvas-style API on top of engine-core.

mod canvas;
pub mod shapes;
mod surface;

pub use canvas::{Canvas, ImageFitMode, ScrimDraw};
pub use surface::RuneSurface;

/// Resolve an asset path by checking multiple locations:
/// 1. Absolute path (as-is)
/// 2. Relative to current directory
/// 3. In macOS app bundle Resources directory
/// 4. In app bundle Resources with just filename
pub fn resolve_asset_path(source: &std::path::Path) -> std::path::PathBuf {
    // If absolute path exists, use it
    if source.is_absolute() && source.exists() {
        return source.to_path_buf();
    }

    // Try relative to current directory
    if source.exists() {
        return source.to_path_buf();
    }

    // Try in macOS app bundle Resources directory
    if let Ok(exe_path) = std::env::current_exe() {
        // exe is at App.app/Contents/MacOS/binary
        // resources are at App.app/Contents/Resources/
        if let Some(contents_dir) = exe_path.parent().and_then(|p| p.parent()) {
            let resources_path = contents_dir.join("Resources").join(source);
            if resources_path.exists() {
                return resources_path;
            }
            // Also try without the leading directory (e.g., "images/foo.png" -> "foo.png")
            if let Some(filename) = source.file_name() {
                let resources_path = contents_dir.join("Resources").join(filename);
                if resources_path.exists() {
                    return resources_path;
                }
            }
        }
    }

    // Return original path (caller will handle non-existent case)
    source.to_path_buf()
}
