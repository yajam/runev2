use super::super::html::ComputedStyle as LegacyStyle;
use crate::css::types::ComputedStyle2;

// Delegate to the html module where private fields are accessible.
pub fn apply_cssv2_inline_to_style(style: &mut LegacyStyle, v2: &ComputedStyle2) {
    super::super::html::apply_cssv2_inline_to_style(style, v2)
}
