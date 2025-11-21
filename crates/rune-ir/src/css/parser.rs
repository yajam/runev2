#[cfg(feature = "cssv2")]
use lightningcss::stylesheet::{ParserOptions, StyleSheet};

use crate::css::diagnostics::diagnostics_enabled;
use crate::css::properties::canonical::{apply_whitelisted_property, is_whitelisted_property};
use crate::css::types::ComputedStyle2;

/// Convert an inline style string (from the `style` attribute) into a
/// whitelisted, canonical `ComputedStyle2`. No selector cascade; inline only.
#[allow(dead_code)]
pub fn compute_inline_only(style_attr: &str) -> ComputedStyle2 {
    let mut out = ComputedStyle2::default();
    let pairs = parse_inline_declarations_lightningcss(style_attr)
        .unwrap_or_else(|| parse_inline_declarations(style_attr));
    for (name, value) in pairs {
        apply_whitelisted_property(&mut out, &name, &value);
    }
    if diagnostics_enabled("css") {
        for (name, value) in parse_inline_declarations(style_attr) {
            if !is_whitelisted_property(&name) && !name.starts_with("--") && !name.starts_with('-')
            {
                tracing::info!(property = %name, value = %value, "diagnostics: cssv2 ignored inline property");
            }
        }
    }
    out
}

/// Extremely small, permissive inline declaration parser used as a stopgap
/// until we switch to lightningcss for all parsing. Returns lowercase property
/// names and raw values.
fn parse_inline_declarations(source: &str) -> Vec<(String, String)> {
    source
        .split(';')
        .filter_map(|decl| {
            let (name, value) = decl.split_once(':')?;
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim().to_string();
            if name.is_empty() || value.is_empty() {
                return None;
            }
            Some((name, value))
        })
        .collect()
}

/// Attempt to parse the `style=` attribute using lightningcss.
#[cfg(feature = "cssv2")]
fn parse_inline_declarations_lightningcss(source: &str) -> Option<Vec<(String, String)>> {
    let css = format!("rune-inline{{{}}}", source);
    let _sheet = StyleSheet::parse(&css, ParserOptions::default()).ok()?;
    Some(parse_inline_declarations(source))
}

#[cfg(not(feature = "cssv2"))]
fn parse_inline_declarations_lightningcss(_source: &str) -> Option<Vec<(String, String)>> {
    None
}
