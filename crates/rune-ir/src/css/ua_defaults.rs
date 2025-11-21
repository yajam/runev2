use super::types::Display2;
use std::collections::BTreeMap;

// Minimal UA defaults layer. Intended to be extended incrementally.
// Applies per-element and per-property defaults when missing from cascade winners.

pub fn apply_ua_defaults(
    tag: &str,
    winners: &BTreeMap<String, String>,
    color_hint: Option<&str>,
) -> BTreeMap<String, String> {
    let mut out = winners.clone();

    // Resolve computed color for currentColor usage
    let computed_color = color_hint
        .map(|s| s.to_string())
        .or_else(|| winners.get("color").cloned())
        .unwrap_or_else(|| "rgb(0, 0, 0)".to_string());

    // Helper: set default only when missing
    fn set_default(map: &mut BTreeMap<String, String>, key: &str, val: &str) {
        map.entry(key.to_string())
            .or_insert_with(|| val.to_string());
    }

    // Border defaults (all sides)
    for side in ["top", "right", "bottom", "left"] {
        let style_key = format!("border-{}-style", side);
        let width_key = format!("border-{}-width", side);
        let color_key = format!("border-{}-color", side);
        set_default(&mut out, &style_key, "none");
        set_default(&mut out, &width_key, "0px");
        out.entry(color_key).or_insert(computed_color.clone());
    }

    // Aggregate borders, in case consumers look for them (optional)
    set_default(&mut out, "border-style", "none");
    set_default(&mut out, "border-width", "0px");
    out.entry("border-color".to_string())
        .or_insert(computed_color.clone());

    // Display defaults (limited): body/block-level elements default to block
    if !out.contains_key("display") {
        if is_default_block(tag) {
            out.insert("display".to_string(), "block".to_string());
        } else if tag.eq_ignore_ascii_case("span") {
            out.insert("display".to_string(), "inline".to_string());
        }
    }

    // Body margin UA default (8px) if truly unspecified. Many browsers set this; keep parity.
    if tag.eq_ignore_ascii_case("body") {
        let any_margin = out.contains_key("margin")
            || out.contains_key("margin-top")
            || out.contains_key("margin-right")
            || out.contains_key("margin-bottom")
            || out.contains_key("margin-left");
        if !any_margin {
            set_default(&mut out, "margin-top", "8px");
            set_default(&mut out, "margin-right", "8px");
            set_default(&mut out, "margin-bottom", "8px");
            set_default(&mut out, "margin-left", "8px");
        }
    }

    // Generic UA defaults (subset; provided when missing). These mirror CSS UA defaults
    // conceptually; many are currently informational until the property whitelist expands.
    set_default(&mut out, "position", "static");
    set_default(&mut out, "z-index", "auto");
    set_default(&mut out, "top", "auto");
    set_default(&mut out, "left", "auto");
    set_default(&mut out, "right", "auto");
    set_default(&mut out, "bottom", "auto");
    set_default(&mut out, "float", "none");
    set_default(&mut out, "clear", "none");
    set_default(&mut out, "overflow", "visible");
    set_default(&mut out, "box-sizing", "content-box");
    set_default(&mut out, "width", "auto");
    set_default(&mut out, "height", "auto");
    set_default(&mut out, "min-width", "0");
    set_default(&mut out, "min-height", "0");
    set_default(&mut out, "max-width", "none");
    set_default(&mut out, "max-height", "none");
    // Do not inject a generic `margin: 0`; rely on CSS initial (0) or
    // element-specific UA rules (e.g., headings/paragraphs) when added.
    set_default(&mut out, "padding", "0");
    set_default(&mut out, "outline", "none");
    // Background
    set_default(&mut out, "background-image", "none");
    set_default(&mut out, "background-repeat", "repeat");
    set_default(&mut out, "background-position", "0% 0%");
    // Color and opacity
    set_default(&mut out, "opacity", "1");
    out.entry("color".to_string()).or_insert(computed_color);
    // Typography
    // Use system sans-serif at 15px as default. Authors can override via CSS.
    set_default(&mut out, "font-family", "sans-serif");
    set_default(&mut out, "font-size", "15px");
    set_default(&mut out, "font-style", "normal");
    set_default(&mut out, "font-weight", "normal");
    set_default(&mut out, "font-variant", "normal");
    set_default(&mut out, "line-height", "normal");
    set_default(&mut out, "text-align", "start");
    set_default(&mut out, "text-indent", "0");
    set_default(&mut out, "text-transform", "none");
    set_default(&mut out, "text-decoration", "none");
    set_default(&mut out, "letter-spacing", "normal");
    set_default(&mut out, "word-spacing", "normal");
    set_default(&mut out, "white-space", "normal");
    set_default(&mut out, "direction", "ltr");
    set_default(&mut out, "unicode-bidi", "normal");
    // Visibility and interaction
    set_default(&mut out, "visibility", "visible");
    set_default(&mut out, "cursor", "auto");
    set_default(&mut out, "user-select", "text");
    set_default(&mut out, "vertical-align", "baseline");
    // Lists and tables (mostly for completeness)
    set_default(&mut out, "list-style-type", "disc");
    set_default(&mut out, "list-style-position", "outside");
    set_default(&mut out, "list-style-image", "none");
    set_default(&mut out, "border-collapse", "separate");
    set_default(&mut out, "border-spacing", "2px");
    set_default(&mut out, "caption-side", "top");
    set_default(&mut out, "empty-cells", "show");
    // Misc
    set_default(&mut out, "appearance", "auto");
    set_default(&mut out, "resize", "none");
    set_default(&mut out, "quotes", "\u{201C} \u{201D} \u{2018} \u{2019}");
    set_default(&mut out, "content", "normal");
    set_default(&mut out, "orphans", "2");
    set_default(&mut out, "widows", "2");
    set_default(&mut out, "tab-size", "8");
    set_default(&mut out, "text-rendering", "auto");

    // --- UA typographic element defaults (subset) ---
    // Headings: provide explicit px defaults close to modern UA sheets.
    // These apply only when authors did not specify sizes themselves.
    let t = tag.to_ascii_lowercase();
    match t.as_str() {
        "h1" => {
            set_default(&mut out, "font-size", "32px");
            set_default(&mut out, "line-height", "40px");
            set_default(&mut out, "font-weight", "700");
        }
        "h2" => {
            set_default(&mut out, "font-size", "28px");
            set_default(&mut out, "line-height", "36px");
            set_default(&mut out, "font-weight", "700");
        }
        "h3" => {
            set_default(&mut out, "font-size", "24px");
            set_default(&mut out, "line-height", "32px");
            set_default(&mut out, "font-weight", "700");
        }
        "h4" => {
            set_default(&mut out, "font-size", "20px");
            set_default(&mut out, "line-height", "28px");
            set_default(&mut out, "font-weight", "700");
        }
        "h5" => {
            set_default(&mut out, "font-size", "18px");
            set_default(&mut out, "line-height", "26px");
            set_default(&mut out, "font-weight", "700");
        }
        // h6 (and any deeper heading tag we may encounter) keeps stepping down
        "h6" | "h7" => {
            set_default(&mut out, "font-size", "16px");
            set_default(&mut out, "line-height", "22px");
            set_default(&mut out, "font-weight", "700");
        }
        // strong/b default to bold when not explicitly set.
        "strong" | "b" => {
            set_default(&mut out, "font-weight", "700");
        }
        // small defaults to 0.875em of our 15px base (~13px). Keep line-height proportional.
        "small" => {
            set_default(&mut out, "font-size", "13px");
            // line-height stays "normal"; allow downstream resolver to compute.
        }
        _ => {}
    }

    out
}

fn is_default_block(tag: &str) -> bool {
    matches!(
        &tag.to_ascii_lowercase()[..],
        "html"
            | "body"
            | "div"
            | "nav"
            | "section"
            | "article"
            | "main"
            | "header"
            | "footer"
            | "form"
            | "p"
            | "ul"
            | "ol"
            | "li"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
    )
}

/// Return the UA default `display` for an element tag, approximating Safari/WebKit defaults.
/// This is a minimal mapping sufficient for common inline vs. block distinctions used in the
/// demo and renderer. When unresolved, falls back to `inline` for phrasing content and `block`
/// for typical sectioning/root elements.
pub fn default_display_for_tag(tag: &str) -> Display2 {
    let t = tag.to_ascii_lowercase();
    match t.as_str() {
        // Phrasing content → inline
        "a" | "span" | "strong" | "em" | "b" | "i" | "u" | "small" | "mark" | "abbr" | "code"
        | "time" | "label" | "img" => Display2::Inline,
        // Sectioning and flow content → block (kept minimal; expand as needed)
        _ => {
            if is_default_block(&t) {
                Display2::Block
            } else {
                // Default to inline for unknown/other phrasing elements
                Display2::Inline
            }
        }
    }
}
