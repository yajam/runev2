//! Shared style and color helpers for the IR renderer.

use engine_core::{Brush, ColorLinPremul};
use taffy::prelude::{
    AlignItems, Dimension, JustifyContent, LengthPercentage, LengthPercentageAuto,
};

/// Helper to convert `ViewBackground` to an engine-core `Brush`.
pub(crate) fn brush_from_background(bg: &rune_ir::view::ViewBackground) -> Brush {
    match bg {
        rune_ir::view::ViewBackground::Solid { color } => {
            // Parse color string (simplified, assuming hex format)
            if let Some(rgba) = parse_color(color) {
                Brush::Solid(rgba)
            } else {
                Brush::Solid(ColorLinPremul::from_srgba_u8([128, 128, 128, 255]))
            }
        }
        // TODO: Implement LinearGradient and RadialGradient
        _ => Brush::Solid(ColorLinPremul::from_srgba_u8([128, 128, 128, 255])),
    }
}

/// Parse a color string (simplified implementation).
///
/// Supports:
/// - Hex: "#RRGGBB", "#RRGGBBAA"
/// - RGB: "rgb(r, g, b)"
/// - RGBA: "rgba(r, g, b, a)"
pub(crate) fn parse_color(color_str: &str) -> Option<ColorLinPremul> {
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

    // TODO: Parse rgb() and rgba()

    None
}

/// Helper to create a Taffy `LengthPercentageAuto` dimension (for margin).
pub(crate) fn length(value: f32) -> LengthPercentageAuto {
    LengthPercentageAuto::Length(value)
}

/// Helper to create a Taffy `LengthPercentage` dimension (for padding, gap).
pub(crate) fn length_percentage(value: f32) -> LengthPercentage {
    LengthPercentage::Length(value)
}

/// Helper to create a Taffy `Dimension` (for size).
pub(crate) fn dimension(value: f32) -> Dimension {
    Dimension::Length(value)
}

/// Helper to create an auto `Dimension`.
pub(crate) fn auto_dimension() -> Dimension {
    Dimension::Auto
}

/// Convert rune-ir `LayoutAlign` to Taffy `AlignItems`.
pub(crate) fn layout_align_to_taffy(align: rune_ir::view::LayoutAlign) -> Option<AlignItems> {
    Some(match align {
        rune_ir::view::LayoutAlign::Start => AlignItems::Start,
        rune_ir::view::LayoutAlign::Center => AlignItems::Center,
        rune_ir::view::LayoutAlign::End => AlignItems::End,
        rune_ir::view::LayoutAlign::Stretch => AlignItems::Stretch,
    })
}

/// Convert rune-ir `LayoutJustify` to Taffy `JustifyContent`.
pub(crate) fn layout_justify_to_taffy(
    justify: rune_ir::view::LayoutJustify,
) -> Option<JustifyContent> {
    Some(match justify {
        rune_ir::view::LayoutJustify::Start => JustifyContent::Start,
        rune_ir::view::LayoutJustify::Center => JustifyContent::Center,
        rune_ir::view::LayoutJustify::End => JustifyContent::End,
        rune_ir::view::LayoutJustify::SpaceBetween => JustifyContent::SpaceBetween,
    })
}
