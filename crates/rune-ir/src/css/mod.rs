//! Experimental CSS v2 style resolver scaffolding (modularized).

mod bridge;
mod cascade;
mod diagnostics;
mod parser;
mod properties;
mod servo_dom;
mod servo_selectors;
mod taffy_mapper;
mod types;
mod ua_defaults;

pub use bridge::apply_cssv2_inline_to_style;
#[allow(unused_imports)]
pub use parser::compute_inline_only;
#[allow(unused_imports)]
pub use taffy_mapper::{TaffyStyleHints, convert_style_to_taffy};
#[allow(unused_imports)]
pub use types::{
    BackgroundLayer2, ComputedStyle2, Display2, GridAutoFlow2, GridTemplate2, GridTrackSize2,
    TextAlign2,
};

// Cascade and stylesheet interfaces (Phase 2)
#[allow(unused_imports)]
pub use cascade::{StyleSheet, build_stylesheet_from_html};
pub use ua_defaults::{apply_ua_defaults, default_display_for_tag};
// Re-export helper type signatures for consumers
#[allow(unused_imports)]
pub use cascade::*;

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::{Html, Selector};

    #[test]
    fn inline_whitelist_parses_basic_values() {
        let s =
            "display:flex; gap: 8px; color: #333; margin: 4px 8px; width:100; object-fit:contain";
        let out = compute_inline_only(s);
        match out.display {
            Some(Display2::Flex) => {}
            other => panic!("unexpected display: {:?}", other),
        }
        assert_eq!(out.gap, Some(8.0));
        assert_eq!(out.color.as_deref(), Some("#333333"));
        assert_eq!(out.margin.top, 4.0);
        assert_eq!(out.margin.right, 8.0);
        assert_eq!(out.width, Some(100.0));
        match out.object_fit {
            Some(crate::view::ImageContentFit::Contain) => {}
            other => panic!("unexpected object_fit: {:?}", other),
        }
    }

    #[test]
    fn color_name_and_rgba_canonicalize() {
        let out = compute_inline_only("color: red; background-color: rgba(0,0,255,0.5)");
        assert_eq!(out.color.as_deref(), Some("#ff0000"));
        assert_eq!(out.background_color.as_deref(), Some("rgba(0,0,255,0.50)"));
    }

    #[cfg(feature = "servo_selectors")]
    #[test]
    fn cascade_descendant_matching_applies_rule() {
        let html = Html::parse_document("<div><span class='a'>x</span></div>");
        let sheet = StyleSheet::from_sources(&["div .a { color: red; }".to_string()]);
        let span = html
            .select(&Selector::parse("span.a").unwrap())
            .next()
            .unwrap();
        let v2 = sheet.compute_for(&span, None);
        assert_eq!(v2.color.as_deref(), Some("#ff0000"));
    }

    #[cfg(feature = "servo_selectors")]
    #[test]
    fn cascade_specificity_prefers_id_chain() {
        let html = Html::parse_document("<div id='x'><span class='a'></span></div>");
        let sheet = StyleSheet::from_sources(&[
            ".a { color: blue; }".to_string(),
            "#x .a { color: red; }".to_string(),
        ]);
        let span = html
            .select(&Selector::parse("span.a").unwrap())
            .next()
            .unwrap();
        let v2 = sheet.compute_for(&span, None);
        assert_eq!(v2.color.as_deref(), Some("#ff0000"));
    }

    #[test]
    fn background_linear_gradient_two_stops() {
        let s = "background: linear-gradient(45deg, red, #00ff00)";
        let v = compute_inline_only(s);
        let g = v.background_gradient.expect("parsed linear gradient");
        assert_eq!(g.angle, 45.0);
        assert_eq!(g.stops.len(), 2);
        assert_eq!(g.stops[0].color, "#ff0000");
        assert_eq!(g.stops[1].color, "#00ff00");
        assert_eq!(g.stops[0].offset, 0.0);
        assert_eq!(g.stops[1].offset, 1.0);
    }

    #[test]
    fn border_radius_parses_length() {
        let v = compute_inline_only("border-radius: 8px");
        assert_eq!(v.corner_radius, Some(8.0));
    }

    #[test]
    fn border_and_box_shadow_parse() {
        let v =
            compute_inline_only("border: 2px solid red; box-shadow: 1px 2px 3px rgba(0,0,0,0.5)");
        assert_eq!(v.border_width, Some(2.0));
        assert_eq!(v.border_color.as_deref(), Some("#ff0000"));
        let sh = v.box_shadow.expect("box-shadow parsed");
        assert_eq!(sh.offset_x, 1.0);
        assert_eq!(sh.offset_y, 2.0);
        assert_eq!(sh.blur, 3.0);
        assert_eq!(sh.color, "rgba(0,0,0,0.50)");
    }

    #[cfg(feature = "servo_selectors")]
    #[test]
    fn var_fallback_and_root_vars() {
        let html = Html::parse_document("<div class='a'></div><div class='b'></div>");
        let sheet = StyleSheet::from_sources(&[
            ":root { --main: red; }".to_string(),
            ".a { color: var(--main, blue); }".to_string(),
            ".b { color: var(--missing, blue); }".to_string(),
        ]);
        let a = html
            .select(&Selector::parse("div.a").unwrap())
            .next()
            .unwrap();
        let b = html
            .select(&Selector::parse("div.b").unwrap())
            .next()
            .unwrap();
        let va = sheet.compute_for(&a, None);
        let vb = sheet.compute_for(&b, None);
        assert_eq!(va.color.as_deref(), Some("#ff0000"));
        assert_eq!(vb.color.as_deref(), Some("#0000ff"));
    }

    #[test]
    fn margin_shorthand_expands() {
        let v = compute_inline_only("margin: 10px 20px");
        assert_eq!(v.margin.top, 10.0);
        assert_eq!(v.margin.right, 20.0);
        assert_eq!(v.margin.bottom, 10.0);
        assert_eq!(v.margin.left, 20.0);
    }

    #[cfg(feature = "servo_selectors")]
    #[test]
    fn gradient_with_css_variables() {
        let html = Html::parse_document("<a class='btn'>Test</a>");
        let sheet = StyleSheet::from_sources(&[
            ":root { --primary: #7c3aed; --primary-2: #22d3ee; }".to_string(),
            ".btn { background: linear-gradient(135deg, var(--primary), var(--primary-2)); }"
                .to_string(),
        ]);
        let btn = html
            .select(&Selector::parse("a.btn").unwrap())
            .next()
            .unwrap();
        let v = sheet.compute_for(&btn, None);
        // Check that background_layers is populated
        assert!(
            !v.background_layers.is_empty(),
            "background_layers should not be empty"
        );
        // Check that the gradient was parsed
        if let Some(BackgroundLayer2::Linear(g)) = v.background_layers.first() {
            assert_eq!(g.angle, 135.0);
            assert_eq!(g.stops.len(), 2);
            assert_eq!(g.stops[0].color, "#7c3aed");
            assert_eq!(g.stops[1].color, "#22d3ee");
        } else {
            panic!("Expected linear gradient in background_layers");
        }
    }
}
