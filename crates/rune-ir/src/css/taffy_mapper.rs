use crate::view::{EdgeInsets, LayoutAlign, LayoutDirection, LayoutJustify};

use super::types::{ComputedStyle2, Display2, TextAlign2};

/// Minimal Taffy-like hints derived from ComputedStyle2 for layout.
/// This avoids introducing a direct dependency on `taffy` in this crate
/// while giving the layout engine all the inputs it needs.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct TaffyStyleHints {
    pub display: Option<Display2>,
    pub direction: LayoutDirection,
    pub wrap: bool,
    pub justify: LayoutJustify,
    pub align: LayoutAlign,
    pub gap: f64,
    // Preserve text-align intent for non-flex centering heuristics
    pub text_align: Option<TextAlign2>,

    pub margin: EdgeInsets,
    pub margin_left_auto: bool,
    pub margin_right_auto: bool,
    pub padding: EdgeInsets,

    pub width: Option<f64>,
    pub height: Option<f64>,

    // Reserved for future use; ComputedStyle2 currently has no min/max
    pub min_width: Option<f64>,
    pub min_height: Option<f64>,
    pub max_width: Option<f64>,
    pub max_height: Option<f64>,
}

pub fn convert_style_to_taffy(v: &ComputedStyle2) -> TaffyStyleHints {
    // CSS default: flex-direction defaults to row for flex containers; grid does not imply flex behavior.
    let default_dir = match v.display {
        Some(Display2::Flex) => LayoutDirection::Row,
        _ => LayoutDirection::Column,
    };
    let direction = v.flex_direction.unwrap_or(default_dir);
    // Do not treat grid as flex-wrap: avoid implicit flex fallback for grid containers.
    let wrap = v.wrap.unwrap_or(false);
    // If container doesn’t specify justify-content, fall back to text-align:center for
    // simple horizontal centering of inline content in column containers.
    let justify = match v.justify_content {
        Some(j) => j,
        None => match v.text_align {
            Some(TextAlign2::Center) => LayoutJustify::Center,
            Some(TextAlign2::End) => LayoutJustify::End,
            _ => LayoutJustify::Start,
        },
    };
    // Cross-axis alignment: if not specified, approximate text-align:center as
    // horizontal centering for column containers (common hero pattern).
    let mut align = v.align_items.unwrap_or(LayoutAlign::Stretch);
    if v.align_items.is_none() {
        if matches!(direction, LayoutDirection::Column) {
            if matches!(v.text_align, Some(TextAlign2::Center)) {
                align = LayoutAlign::Center;
            } else if matches!(v.text_align, Some(TextAlign2::End)) {
                align = LayoutAlign::End;
            }
        }
    }
    let gap = v.gap.unwrap_or(0.0);

    let margin = EdgeInsets {
        top: v.margin.top,
        right: v.margin.right,
        bottom: v.margin.bottom,
        left: v.margin.left,
    };
    let padding = EdgeInsets {
        top: v.padding.top,
        right: v.padding.right,
        bottom: v.padding.bottom,
        left: v.padding.left,
    };

    TaffyStyleHints {
        display: v.display,
        direction,
        wrap,
        justify,
        text_align: v.text_align,
        align,
        gap,
        margin,
        margin_left_auto: v.margin_left_auto,
        margin_right_auto: v.margin_right_auto,
        padding,
        width: v.width,
        height: v.height,
        min_width: v.min_width,
        min_height: v.min_height,
        max_width: v.max_width,
        max_height: v.max_height,
    }
}
