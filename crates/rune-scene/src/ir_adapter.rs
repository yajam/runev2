//! Adapter module to map rune-ir ViewNodes to rune-scene elements.
//!
//! This module provides conversion functions from rune-ir's ViewDocument/ViewNode
//! types to the element types used in viewport_ir.rs (Button, Checkbox, InputBox, etc.).
//!
//! # Architecture Note
//!
//! rune-ir uses a data-view separation architecture:
//! - **ViewNodes** specify layout, styling, and visual structure
//! - **DataNodes** (in DataDocument) contain actual content (text, values, etc.)
//! - Specs reference data via the ViewNode's `node_id` field
//!
//! This adapter focuses on converting visual specs to rendering elements.
//! Content resolution from DataDocument should be handled by the caller.

use engine_core::{ColorLinPremul, Rect};
use rune_ir::view::{ButtonSpec, CheckboxSpec, FlexContainerSpec, TextStyle, ViewBackground};

use crate::elements;

/// Adapter for converting rune-ir ViewNodes to rune-scene elements.
pub struct IrAdapter;

impl IrAdapter {
    /// Convert a ButtonSpec to a Button element.
    ///
    /// # Arguments
    ///
    /// * `spec` - The ButtonSpec from rune-ir
    /// * `rect` - The computed layout rect (from Taffy or manual layout)
    /// * `label` - The button text (resolved from DataDocument by caller)
    pub fn button_from_spec(
        spec: &ButtonSpec,
        rect: Rect,
        label: Option<String>,
    ) -> elements::Button {
        let bg = spec
            .style
            .background
            .as_ref()
            .and_then(parse_background_color)
            .unwrap_or_else(|| ColorLinPremul::from_srgba_u8([63, 130, 246, 255]));

        let fg = spec
            .label_style
            .color
            .as_ref()
            .and_then(|c| parse_color(c))
            .unwrap_or_else(|| ColorLinPremul::from_srgba_u8([255, 255, 255, 255]));

        elements::Button {
            rect,
            radius: spec.style.corner_radius.unwrap_or(8.0) as f32,
            bg,
            fg,
            label: label.unwrap_or_else(|| "Button".to_string()),
            label_size: spec.label_style.font_size.unwrap_or(16.0) as f32,
            focused: false,
            on_click_intent: spec.on_click_intent.clone(),
        }
    }

    /// Convert a CheckboxSpec to a Checkbox element (basic rect-based rendering).
    ///
    /// # Arguments
    ///
    /// * `spec` - The CheckboxSpec from rune-ir
    /// * `rect` - The computed layout rect
    ///
    /// # Note
    ///
    /// CheckboxSpec only contains size and form metadata. Labels are typically
    /// separate Text nodes in the ViewDocument.
    pub fn checkbox_from_spec(spec: &CheckboxSpec, rect: Rect) -> elements::Checkbox {
        elements::Checkbox {
            rect,
            checked: spec.default_checked.unwrap_or(false),
            focused: false,
            label: None, // Labels are separate Text nodes
            label_size: 16.0,
            color: ColorLinPremul::from_srgba_u8([240, 240, 240, 255]),
        }
    }

    /// Extract background color from a FlexContainerSpec (if solid).
    pub fn background_from_flex_spec(spec: &FlexContainerSpec) -> Option<ColorLinPremul> {
        spec.background.as_ref().and_then(parse_background_color)
    }

    /// Convert a TextStyle to a color for text rendering.
    pub fn color_from_text_style(style: &TextStyle) -> ColorLinPremul {
        style
            .color
            .as_ref()
            .and_then(|c| parse_color(c))
            .unwrap_or_else(|| ColorLinPremul::from_srgba_u8([240, 240, 240, 255]))
    }

    /// Get font size from TextStyle.
    pub fn font_size_from_text_style(style: &TextStyle) -> f32 {
        style.font_size.unwrap_or(16.0) as f32
    }
}

/// Parse a ViewBackground to extract a solid color (if present).
fn parse_background_color(bg: &ViewBackground) -> Option<ColorLinPremul> {
    match bg {
        ViewBackground::Solid { color } => parse_color(color),
        _ => None,
    }
}

/// Parse a color string to ColorLinPremul.
///
/// Supports:
/// - Hex: "#RRGGBB", "#RRGGBBAA"
/// - RGB: "rgb(r, g, b)"
/// - RGBA: "rgba(r, g, b, a)"
/// - Named colors: "red", "blue", etc.
pub fn parse_color(color_str: &str) -> Option<ColorLinPremul> {
    let trimmed = color_str.trim();

    // Hex color
    if let Some(hex) = trimmed.strip_prefix('#') {
        let hex = hex.trim();
        if hex.len() == 6 {
            // #RRGGBB
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(ColorLinPremul::from_srgba_u8([r, g, b, 255]));
        } else if hex.len() == 8 {
            // #RRGGBBAA
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            return Some(ColorLinPremul::from_srgba_u8([r, g, b, a]));
        }
    }

    // RGB/RGBA format
    if let Some(rgb) = trimmed.strip_prefix("rgb(") {
        if let Some(rgba) = rgb.strip_suffix(')') {
            let parts: Vec<&str> = rgba.split(',').map(|s| s.trim()).collect();
            if parts.len() == 3 {
                let r = parts[0].parse::<u8>().ok()?;
                let g = parts[1].parse::<u8>().ok()?;
                let b = parts[2].parse::<u8>().ok()?;
                return Some(ColorLinPremul::from_srgba_u8([r, g, b, 255]));
            }
        }
    }

    if let Some(rgba) = trimmed.strip_prefix("rgba(") {
        if let Some(rgba_inner) = rgba.strip_suffix(')') {
            let parts: Vec<&str> = rgba_inner.split(',').map(|s| s.trim()).collect();
            if parts.len() == 4 {
                let r = parts[0].parse::<u8>().ok()?;
                let g = parts[1].parse::<u8>().ok()?;
                let b = parts[2].parse::<u8>().ok()?;
                let a = (parts[3].parse::<f32>().ok()? * 255.0) as u8;
                return Some(ColorLinPremul::from_srgba_u8([r, g, b, a]));
            }
        }
    }

    // Named colors (basic set)
    match trimmed.to_lowercase().as_str() {
        "red" => Some(ColorLinPremul::from_srgba_u8([255, 0, 0, 255])),
        "green" => Some(ColorLinPremul::from_srgba_u8([0, 255, 0, 255])),
        "blue" => Some(ColorLinPremul::from_srgba_u8([0, 0, 255, 255])),
        "white" => Some(ColorLinPremul::from_srgba_u8([255, 255, 255, 255])),
        "black" => Some(ColorLinPremul::from_srgba_u8([0, 0, 0, 255])),
        "yellow" => Some(ColorLinPremul::from_srgba_u8([255, 255, 0, 255])),
        "cyan" => Some(ColorLinPremul::from_srgba_u8([0, 255, 255, 255])),
        "magenta" => Some(ColorLinPremul::from_srgba_u8([255, 0, 255, 255])),
        "gray" | "grey" => Some(ColorLinPremul::from_srgba_u8([128, 128, 128, 255])),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_color() {
        assert!(parse_color("#ff0000").is_some());
        assert!(parse_color("#00ff00").is_some());
        assert!(parse_color("#0000ff80").is_some());
    }

    #[test]
    fn test_parse_rgb_color() {
        assert!(parse_color("rgb(255, 0, 0)").is_some());
        assert!(parse_color("rgba(0, 255, 0, 0.5)").is_some());
    }

    #[test]
    fn test_parse_named_color() {
        assert!(parse_color("red").is_some());
        assert!(parse_color("blue").is_some());
        assert!(parse_color("green").is_some());
    }

    #[test]
    fn test_parse_color_invalid() {
        assert_eq!(parse_color("invalid"), None);
        assert_eq!(parse_color("#ff"), None);
        assert_eq!(parse_color("#gggggg"), None);
    }
}
