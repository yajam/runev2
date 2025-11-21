//! Rune Draw configuration system
//!
//! This crate provides centralized configuration management for Rune Draw,
//! loading settings from `rune.toml` as an alternative to environment variables.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Main configuration structure for Rune Draw
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RuneConfig {
    /// Demo application settings
    pub demo: DemoConfig,
    /// Text rendering settings
    pub text: TextConfig,
    /// Rendering engine settings
    pub rendering: RenderingConfig,
    /// IR (Intermediate Representation) settings
    pub ir: IrConfig,
    /// Layout engine settings
    pub layout: LayoutConfig,
}

/// Demo application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DemoConfig {
    /// Default scene to load (zones, images, overlay, shadow, linear, radial, etc.)
    pub scene: Option<String>,
}

/// Text rendering configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextConfig {
    /// Path to custom font file (.ttf)
    pub font: Option<PathBuf>,
    /// Use FreeType provider for LCD rendering (requires freetype_ffi feature)
    pub use_freetype: bool,
    /// Fractional X offset for subpixel text positioning (e.g., 0.33)
    pub subpixel_offset: Option<f32>,
    /// Snap text X position to integer pixels
    pub snap_x: bool,
    /// Default text size in points
    pub text_size: Option<f32>,
    /// Extra baseline spacing between text lines in pixels
    pub line_padding: Option<f32>,
}

/// Rendering engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RenderingConfig {
    /// Enable intermediate texture for smooth resizing (default: true)
    pub use_intermediate: bool,
    /// Enable debug visualization for radial backgrounds
    pub debug_radial: bool,
    /// Lyon tessellation tolerance for path rendering
    pub lyon_tolerance: Option<f32>,
}

/// IR (Intermediate Representation) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IrConfig {
    /// Use IR rendering mode (vs legacy rendering)
    pub use_ir: bool,
    /// Path to a Rune package to load and render (e.g., "examples/sample_first_node")
    pub package_path: Option<PathBuf>,
    /// Enable diagnostic output for IR processing
    pub diagnostics: Option<String>,
    /// Enable user-agent heading margins in HTML rendering
    pub ua_heading_margins: bool,
}

/// Layout engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LayoutConfig {
    // Placeholder for future layout settings
}

impl Default for DemoConfig {
    fn default() -> Self {
        Self { scene: None }
    }
}

impl Default for TextConfig {
    fn default() -> Self {
        Self {
            font: None,
            use_freetype: false,
            subpixel_offset: None,
            snap_x: false,
            text_size: None,
            line_padding: None,
        }
    }
}

impl Default for RenderingConfig {
    fn default() -> Self {
        Self {
            use_intermediate: true, // Default to true for smooth resizing
            debug_radial: false,
            lyon_tolerance: None,
        }
    }
}

impl Default for IrConfig {
    fn default() -> Self {
        Self {
            use_ir: true, // Default to IR mode
            package_path: None,
            diagnostics: None,
            ua_heading_margins: false,
        }
    }
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {}
    }
}

impl RuneConfig {
    /// Load configuration from a TOML file
    ///
    /// # Arguments
    /// * `path` - Path to the rune.toml configuration file
    ///
    /// # Returns
    /// * `Ok(RuneConfig)` - Successfully loaded configuration
    /// * `Err(String)` - Error message if loading failed
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| format!("Failed to read config file: {}", e))?;

        toml::from_str(&content).map_err(|e| format!("Failed to parse config file: {}", e))
    }

    /// Load configuration from the default location (rune.toml in the current directory)
    /// or return default configuration if file doesn't exist
    pub fn load_or_default() -> Self {
        Self::load_from_file("rune.toml").unwrap_or_default()
    }

    /// Merge configuration with environment variables
    ///
    /// Environment variables take precedence over configuration file values.
    /// This allows for temporary overrides without modifying the config file.
    pub fn merge_with_env(&mut self) {
        // Demo settings
        if let Ok(scene) = std::env::var("DEMO_SCENE") {
            self.demo.scene = Some(scene);
        }

        // Text settings
        if let Ok(font) = std::env::var("DEMO_FONT") {
            self.text.font = Some(PathBuf::from(font));
        }
        if let Ok(val) = std::env::var("DEMO_FREETYPE") {
            self.text.use_freetype = val == "1" || val.eq_ignore_ascii_case("true");
        }
        if let Ok(val) = std::env::var("DEMO_SUBPIXEL_OFFSET") {
            if let Ok(offset) = val.parse::<f32>() {
                self.text.subpixel_offset = Some(offset);
            }
        }
        if let Ok(val) = std::env::var("DEMO_SNAP_X") {
            self.text.snap_x = val == "1" || val.eq_ignore_ascii_case("true");
        }
        if let Ok(val) = std::env::var("DEMO_TEXT_SIZE") {
            if let Ok(size) = val.parse::<f32>() {
                self.text.text_size = Some(size);
            }
        }
        if let Ok(val) = std::env::var("DEMO_LINE_PAD") {
            if let Ok(pad) = val.parse::<f32>() {
                self.text.line_padding = Some(pad);
            }
        }

        // Rendering settings
        if let Ok(val) = std::env::var("USE_INTERMEDIATE") {
            self.rendering.use_intermediate = val == "1" || val.eq_ignore_ascii_case("true");
        }
        if let Ok(val) = std::env::var("DEBUG_RADIAL") {
            self.rendering.debug_radial = val == "1" || val.eq_ignore_ascii_case("true");
        }
        if let Ok(val) = std::env::var("LYON_TOLERANCE") {
            if let Ok(tol) = val.parse::<f32>() {
                self.rendering.lyon_tolerance = Some(tol);
            }
        }

        // IR settings
        if let Ok(val) = std::env::var("USE_IR") {
            self.rendering.use_intermediate = val == "1" || val.eq_ignore_ascii_case("true");
        }
        if let Ok(path) = std::env::var("RUNE_PACKAGE_PATH") {
            self.ir.package_path = Some(PathBuf::from(path));
        }
        if let Ok(diagnostics) = std::env::var("RUNE_DIAGNOSTICS") {
            self.ir.diagnostics = Some(diagnostics);
        }
        if let Ok(val) = std::env::var("RUNE_UA_HEADING_MARGINS") {
            self.ir.ua_heading_margins = val == "1" || val.eq_ignore_ascii_case("true");
        }

        // Also check for RUNE_TEXT_FONT as an alternative to DEMO_FONT
        if let Ok(font) = std::env::var("RUNE_TEXT_FONT") {
            self.text.font = Some(PathBuf::from(font));
        }
    }

    /// Load configuration with environment variable overrides
    ///
    /// This is the recommended way to load configuration:
    /// 1. Load from rune.toml (or use defaults if not found)
    /// 2. Override with environment variables if present
    pub fn load() -> Self {
        let mut config = Self::load_or_default();
        config.merge_with_env();
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RuneConfig::default();
        assert!(config.rendering.use_intermediate);
        assert!(!config.rendering.debug_radial);
        assert!(config.ir.use_ir);
    }

    #[test]
    fn test_toml_serialization() {
        let config = RuneConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: RuneConfig = toml::from_str(&toml_str).unwrap();
        assert!(parsed.rendering.use_intermediate);
    }

    #[test]
    fn test_load_or_default() {
        // Should not panic even if rune.toml doesn't exist
        let config = RuneConfig::load_or_default();
        // Verify defaults are set
        assert!(config.rendering.use_intermediate);
        assert!(config.ir.use_ir);
    }

    #[test]
    fn test_merge_with_env() {
        // Set environment variable
        unsafe {
            std::env::set_var("DEMO_SCENE", "test-scene");
            std::env::set_var("USE_INTERMEDIATE", "false");
        }

        let mut config = RuneConfig::default();
        config.merge_with_env();

        assert_eq!(config.demo.scene.as_deref(), Some("test-scene"));
        assert!(!config.rendering.use_intermediate);

        // Clean up
        unsafe {
            std::env::remove_var("DEMO_SCENE");
            std::env::remove_var("USE_INTERMEDIATE");
        }
    }
}
