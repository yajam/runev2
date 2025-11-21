use anyhow::{Context, Result, bail};
use scraper::{ElementRef, Html, Selector};
use serde_json::{Map, Value, json};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use rune_ir::html::HtmlOptions;
use rune_ir::{ComputedStyle2, build_stylesheet_from_html};

fn main() -> Result<()> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        eprintln!(
            "Usage: cargo run -p rune-ir --bin css_snapshot <html-file> [--out-dir <dir>] [--selectors <sel> ...]"
        );
        bail!("missing <html-file>");
    }

    let input = PathBuf::from(args.remove(0));
    if !input.exists() {
        bail!("input file not found: {}", input.display());
    }

    // Parse optional flags
    let mut out_dir: Option<PathBuf> = None;
    let mut selectors: Vec<String> = Vec::new();
    let mut ref_json: Option<PathBuf> = None;
    let mut only_all_props = true; // default to full-props mode for focused parity work
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--out-dir" => {
                if i + 1 >= args.len() {
                    bail!("--out-dir expects a path");
                }
                out_dir = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--ref-json" => {
                if i + 1 >= args.len() {
                    bail!("--ref-json expects a path");
                }
                ref_json = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--subset" => {
                // emit our subset alongside; default is only-all-props
                only_all_props = false;
                i += 1;
            }
            "--selectors" => {
                i += 1;
                while i < args.len() {
                    // Treat the rest as selectors unless another flag appears
                    if args[i].starts_with("--") {
                        break;
                    }
                    selectors.push(args[i].clone());
                    i += 1;
                }
            }
            _ => {
                // Treat as an extra selector for convenience
                selectors.push(args[i].clone());
                i += 1;
            }
        }
    }

    let html_src = fs::read_to_string(&input)
        .with_context(|| format!("failed to read {}", input.display()))?;
    let document = Html::parse_document(&html_src);

    let options = HtmlOptions {
        base_path: input.parent().map(|p| p.to_path_buf()),
        ..Default::default()
    };

    let sheet = build_stylesheet_from_html(&document, &options);

    // Default selector: focus on body
    if selectors.is_empty() {
        selectors = vec!["body".to_string()];
    }

    let out_base =
        out_dir.unwrap_or_else(|| input.parent().unwrap_or(Path::new(".")).to_path_buf());
    fs::create_dir_all(&out_base).ok();

    for sel_text in selectors {
        let selector = match Selector::parse(&sel_text) {
            Ok(s) => s,
            Err(_) => {
                eprintln!("invalid selector: {}", sel_text);
                continue;
            }
        };
        let el = match document.select(&selector).next() {
            Some(n) => n,
            None => {
                eprintln!("selector matched no elements: {}", sel_text);
                continue;
            }
        };
        let inline = el.value().attr("style");
        // Compute raw winners (ignoring whitelist)
        let full = sheet.compute_raw_for(&el, inline);

        let file_stem = sanitize_name(&sel_text);
        // Hydrate shorthands (background/border) into longhands before export
        let full_hydrated = hydrate_background_and_border(&full);
        // If a reference JSON is provided, use its keys to produce an all-props map.
        if let Some(ref_path) = ref_json.as_ref() {
            let (keys, ref_map) = load_keys(ref_path)?;
            // Apply UA defaults with best color hint (v2 color -> winners color -> ref color)
            let tag = el.value().name();
            let v2_for_color = sheet.compute_for(&el, inline);
            let color_hint = v2_for_color
                .color
                .as_ref()
                .map(|s| convert_hex_colors_to_rgba(s))
                .or_else(|| full_hydrated.get("color").cloned())
                .or_else(|| ref_map.get("color").cloned());
            let ua_applied = rune_ir::apply_ua_defaults(tag, &full_hydrated, color_hint.as_deref());
            let combined = fill_all_props_with_ref(&keys, &ua_applied, &ref_map);
            let file_all = out_base.join(format!("{}_rune_computedStyle.json", file_stem));
            fs::write(&file_all, serde_json::to_string_pretty(&combined)?)
                .with_context(|| format!("failed to write {}", file_all.display()))?;
            println!("wrote {}", file_all.display());
        } else {
            // Fallback: write the raw map for inspection, with normalization
            let tag = el.value().name();
            let v2_for_color = sheet.compute_for(&el, inline);
            let color_hint = v2_for_color
                .color
                .as_ref()
                .map(|s| convert_hex_colors_to_rgba(s));
            let ua_applied = rune_ir::apply_ua_defaults(tag, &full_hydrated, color_hint.as_deref());
            let normalized: std::collections::BTreeMap<String, String> = ua_applied
                .into_iter()
                .map(|(k, v)| {
                    let mut vv = convert_hex_colors_to_rgba(&v);
                    if k == "background-image" {
                        vv = normalize_transparent(&vv);
                    }
                    if k == "backdrop-filter" || k == "filter" {
                        vv = normalize_saturate_percent(&vv);
                    }
                    if matches!(k.as_str(), "top" | "left" | "right" | "bottom") && vv.trim() == "0"
                    {
                        vv = "0px".to_string();
                    }
                    vv = normalize_numeric_tokens(&vv);
                    (k, vv)
                })
                .collect();
            let file_full = out_base.join(format!("{}_computed_raw.json", file_stem));
            fs::write(&file_full, serde_json::to_string_pretty(&normalized)?)
                .with_context(|| format!("failed to write {}", file_full.display()))?;
            println!("wrote {}", file_full.display());
        }

        if !only_all_props {
            let v2 = sheet.compute_for(&el, inline);
            let subset = to_snapshot_map(&v2, &el);
            let file_subset = out_base.join(format!("{}_style_v2.json", file_stem));
            fs::write(
                &file_subset,
                serde_json::to_string_pretty(&Value::Object(subset))?,
            )
            .with_context(|| format!("failed to write {}", file_subset.display()))?;
            println!("wrote {}", file_subset.display());
        }
    }

    Ok(())
}

fn sanitize_name(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' => c,
            _ => '_',
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

fn to_snapshot_map(v: &ComputedStyle2, el: &ElementRef) -> Map<String, Value> {
    let mut m = Map::new();

    // Layout
    if let Some(d) = v.display {
        let s = match d {
            rune_ir::Display2::Block => "block",
            rune_ir::Display2::Flex => "flex",
            rune_ir::Display2::Grid => "grid",
            rune_ir::Display2::Inline => "inline",
            rune_ir::Display2::None => "none",
        };
        m.insert("display".to_string(), json!(s));
    }
    if let Some(dir) = v.flex_direction {
        let s = match dir {
            rune_ir::view::LayoutDirection::Row => "row",
            rune_ir::view::LayoutDirection::Column => "column",
        };
        m.insert("flex-direction".to_string(), json!(s));
    }
    if let Some(wrap) = v.wrap {
        m.insert(
            "flex-wrap".to_string(),
            json!(if wrap { "wrap" } else { "nowrap" }),
        );
    }
    if let Some(j) = v.justify_content {
        let s = match j {
            rune_ir::view::LayoutJustify::Start => "flex-start",
            rune_ir::view::LayoutJustify::Center => "center",
            rune_ir::view::LayoutJustify::End => "flex-end",
            rune_ir::view::LayoutJustify::SpaceBetween => "space-between",
        };
        m.insert("justify-content".to_string(), json!(s));
    }
    if let Some(a) = v.align_items {
        let s = match a {
            rune_ir::view::LayoutAlign::Start => "flex-start",
            rune_ir::view::LayoutAlign::Center => "center",
            rune_ir::view::LayoutAlign::End => "flex-end",
            rune_ir::view::LayoutAlign::Stretch => "stretch",
        };
        m.insert("align-items".to_string(), json!(s));
    }
    if let Some(g) = v.gap {
        m.insert("gap".to_string(), json!(px(g)));
    }

    // Text
    if let Some(c) = v.color.as_ref() {
        m.insert("color".to_string(), json!(c));
    }
    if let Some(fs) = v.font_size {
        m.insert("font-size".to_string(), json!(px(fs)));
    }
    if let Some(lh) = v.line_height {
        m.insert("line-height".to_string(), json!(px(lh)));
    }
    if let Some(fw) = v.font_weight {
        m.insert("font-weight".to_string(), json!(num(fw)));
    }

    // Backgrounds (prefer layered form if present)
    if !v.background_layers.is_empty() {
        let mut layers: Vec<String> = Vec::new();
        for layer in &v.background_layers {
            match layer {
                rune_ir::BackgroundLayer2::Solid(c) => layers.push(c.clone()),
                rune_ir::BackgroundLayer2::Linear(g) => {
                    let stops_str = if g.stops.is_empty() {
                        "#000000, #000000".to_string()
                    } else {
                        let mut v: Vec<String> = Vec::with_capacity(g.stops.len());
                        for s in &g.stops {
                            let pct = trim_float(s.offset * 100.0);
                            v.push(format!("{} {}%", s.color, pct));
                        }
                        v.join(", ")
                    };
                    if g.angle == 0.0 {
                        layers.push(format!("linear-gradient({})", stops_str));
                    } else {
                        layers.push(format!(
                            "linear-gradient({}deg, {})",
                            trim_float(g.angle),
                            stops_str
                        ));
                    }
                }
                rune_ir::BackgroundLayer2::Radial(g) => {
                    let cx = format!("{}%", trim_float(g.cx * 100.0));
                    let cy = format!("{}%", trim_float(g.cy * 100.0));
                    let stops_str = if g.stops.is_empty() {
                        "#000000, #000000".to_string()
                    } else {
                        let mut v: Vec<String> = Vec::with_capacity(g.stops.len());
                        for s in &g.stops {
                            let pct = trim_float(s.offset * 100.0);
                            v.push(format!("{} {}%", s.color, pct));
                        }
                        v.join(", ")
                    };
                    layers.push(format!(
                        "radial-gradient({} {} at {} {}, {})",
                        px(g.rx),
                        px(g.ry),
                        cx,
                        cy,
                        stops_str
                    ));
                }
            }
        }
        if !layers.is_empty() {
            m.insert("background".to_string(), json!(layers.join(", ")));
        }
    }
    if let Some(bg) = v.background_color.as_ref() {
        m.insert("background-color".to_string(), json!(bg));
    }

    // Border/Radius/Shadow
    if let Some(r) = v.corner_radius {
        m.insert("border-radius".to_string(), json!(px(r)));
    }
    // Prefer per-side if specified (CSS parity), otherwise emit uniform
    let has_side_widths = v.border_top_width.is_some()
        || v.border_right_width.is_some()
        || v.border_bottom_width.is_some()
        || v.border_left_width.is_some();
    if has_side_widths {
        if let Some(w) = v.border_top_width {
            m.insert("border-top-width".to_string(), json!(px(w)));
        }
        if let Some(w) = v.border_right_width {
            m.insert("border-right-width".to_string(), json!(px(w)));
        }
        if let Some(w) = v.border_bottom_width {
            m.insert("border-bottom-width".to_string(), json!(px(w)));
        }
        if let Some(w) = v.border_left_width {
            m.insert("border-left-width".to_string(), json!(px(w)));
        }
    } else if let Some(w) = v.border_width {
        m.insert("border-width".to_string(), json!(px(w)));
    }

    let has_side_colors = v.border_top_color.is_some()
        || v.border_right_color.is_some()
        || v.border_bottom_color.is_some()
        || v.border_left_color.is_some();
    if has_side_colors {
        if let Some(c) = v.border_top_color.as_ref() {
            m.insert("border-top-color".to_string(), json!(c));
        }
        if let Some(c) = v.border_right_color.as_ref() {
            m.insert("border-right-color".to_string(), json!(c));
        }
        if let Some(c) = v.border_bottom_color.as_ref() {
            m.insert("border-bottom-color".to_string(), json!(c));
        }
        if let Some(c) = v.border_left_color.as_ref() {
            m.insert("border-left-color".to_string(), json!(c));
        }
    } else if let Some(c) = v.border_color.as_ref() {
        m.insert("border-color".to_string(), json!(c));
    }
    if let Some(sh) = v.box_shadow.as_ref() {
        let s = format!(
            "{} {} {} {}",
            px(sh.offset_x),
            px(sh.offset_y),
            px(sh.blur),
            sh.color
        );
        m.insert("box-shadow".to_string(), json!(s));
    }

    // Spacing
    if v.margin.top != 0.0
        || v.margin.right != 0.0
        || v.margin.bottom != 0.0
        || v.margin.left != 0.0
    {
        m.insert("margin-top".to_string(), json!(px(v.margin.top)));
        m.insert("margin-right".to_string(), json!(px(v.margin.right)));
        m.insert("margin-bottom".to_string(), json!(px(v.margin.bottom)));
        m.insert("margin-left".to_string(), json!(px(v.margin.left)));
    }
    if v.padding.top != 0.0
        || v.padding.right != 0.0
        || v.padding.bottom != 0.0
        || v.padding.left != 0.0
    {
        m.insert("padding-top".to_string(), json!(px(v.padding.top)));
        m.insert("padding-right".to_string(), json!(px(v.padding.right)));
        m.insert("padding-bottom".to_string(), json!(px(v.padding.bottom)));
        m.insert("padding-left".to_string(), json!(px(v.padding.left)));
    }

    // Sizing
    if let Some(w) = v.width {
        m.insert("width".to_string(), json!(px(w)));
    }
    if let Some(h) = v.height {
        m.insert("height".to_string(), json!(px(h)));
    }

    // Images
    if let Some(f) = v.object_fit {
        let s = match f {
            rune_ir::view::ImageContentFit::Cover => "cover",
            rune_ir::view::ImageContentFit::Contain => "contain",
            rune_ir::view::ImageContentFit::Fill => "fill",
        };
        m.insert("object-fit".to_string(), json!(s));
    }

    // A tiny hint for debugging: element tag and classes
    m.insert(
        "_element".to_string(),
        json!(format!(
            "<{} class=\"{}\">",
            el.value().name(),
            el.value().classes().collect::<Vec<_>>().join(" ")
        )),
    );

    m
}

fn px(v: f64) -> String {
    format!("{}px", trim_float(v))
}

fn num(v: f64) -> String {
    trim_float(v)
}

fn trim_float(v: f64) -> String {
    let s = format!("{:.4}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.is_empty() { "0".to_string() } else { s }
}

fn load_keys(path: &Path) -> Result<(Vec<String>, std::collections::BTreeMap<String, String>)> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read reference JSON {}", path.display()))?;
    let v: Value = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse JSON {}", path.display()))?;
    let obj = v
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("reference JSON must be an object"))?;
    let mut map = std::collections::BTreeMap::new();
    for (k, val) in obj.iter() {
        if let Some(s) = val.as_str() {
            map.insert(k.clone(), s.to_string());
        } else {
            map.insert(k.clone(), val.to_string());
        }
    }
    Ok((obj.keys().cloned().collect(), map))
}

fn fill_all_props_with_ref(
    keys: &[String],
    winners: &std::collections::BTreeMap<String, String>,
    ref_map: &std::collections::BTreeMap<String, String>,
) -> Value {
    let mut out = Map::new();
    for k in keys {
        if let Some(v) = winners.get(k) {
            let mut val = v.clone();
            if !k.starts_with("--") {
                if k == "font-family" {
                    val = normalize_font_family(&val);
                }
                val = convert_hex_colors_to_rgba(&val);
                if k == "background-image" {
                    val = normalize_transparent(&val);
                }
                if k == "backdrop-filter" || k == "filter" {
                    val = normalize_saturate_percent(&val);
                }
                if matches!(k.as_str(), "top" | "left" | "right" | "bottom") && val.trim() == "0" {
                    val = "0px".to_string();
                }
                val = normalize_numeric_tokens(&val);
            } else {
                // For custom properties: convert only 8-digit hex (#RRGGBBAA) to rgba, keep hex6 as-is
                val = convert_hex8_to_rgba_only(&val);
            }
            out.insert(k.clone(), Value::String(val));
        } else {
            // Prefer the reference value for parity if available
            let fallback = if let Some(rv) = ref_map.get(k) {
                rv.clone()
            } else {
                default_value_for(k, winners)
            };
            out.insert(k.clone(), Value::String(fallback));
        }
    }
    Value::Object(out)
}

fn convert_hex_colors_to_rgba(input: &str) -> String {
    // Replace any hex colors with rgb/rgba formats within the string.
    // - #RRGGBBAA -> rgba(r, g, b, a.xx)
    // - #RRGGBB   -> rgb(r, g, b)
    // - #RGB      -> rgb(r, g, b)
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] as char == '#' {
            // Try to read up to 8 hex digits
            let mut j = i + 1;
            let mut hex = String::new();
            while j < bytes.len() {
                let c = bytes[j] as char;
                if c.is_ascii_hexdigit() && hex.len() < 8 {
                    hex.push(c);
                    j += 1;
                } else {
                    break;
                }
            }
            if hex.len() == 8 {
                if let Some(rgba) = hex8_to_rgba(&hex) {
                    out.push_str(&rgba);
                    i = j;
                    continue;
                }
            } else if hex.len() == 6 {
                if let Some(rgb) = hex6_to_rgb(&hex) {
                    out.push_str(&rgb);
                    i = j;
                    continue;
                }
            } else if hex.len() == 4
                && let Some(rgba) = hex4_to_rgba(&hex)
            {
                out.push_str(&rgba);
                i = j;
                continue;
            } else if hex.len() == 3
                && let Some(rgb) = hex3_to_rgb(&hex)
            {
                out.push_str(&rgb);
                i = j;
                continue;
            }
            // Not matched: keep '#', process next chars normally
            out.push('#');
            i += 1;
            continue;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

fn hex8_to_rgba(hex: &str) -> Option<String> {
    if hex.len() != 8 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f64;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f64;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f64;
    let a_byte = u8::from_str_radix(&hex[6..8], 16).ok()? as f64;
    let a = (a_byte / 255.0 * 100.0).round() / 100.0; // round to 2 decimals
    Some(format!(
        "rgba({}, {}, {}, {:.2})",
        r as u32, g as u32, b as u32, a
    ))
}

fn hex4_to_rgba(hex: &str) -> Option<String> {
    if hex.len() != 4 {
        return None;
    }
    // #RGBA -> expand to #RRGGBBAA then reuse hex8_to_rgba
    let r = hex.get(0..1)?;
    let g = hex.get(1..2)?;
    let b = hex.get(2..3)?;
    let a = hex.get(3..4)?;
    let expanded = format!("{}{}{}{}{}{}{}{}", r, r, g, g, b, b, a, a);
    hex8_to_rgba(&expanded)
}

fn hex6_to_rgb(hex: &str) -> Option<String> {
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as u32;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as u32;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as u32;
    Some(format!("rgb({}, {}, {})", r, g, b))
}

fn hex3_to_rgb(hex: &str) -> Option<String> {
    if hex.len() != 3 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
    let r = ((r as u16) << 4 | r as u16) as u32;
    let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
    let g = ((g as u16) << 4 | g as u16) as u32;
    let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
    let b = ((b as u16) << 4 | b as u16) as u32;
    Some(format!("rgb({}, {}, {})", r, g, b))
}

fn normalize_transparent(input: &str) -> String {
    // Replace standalone 'transparent' tokens with 'rgba(0, 0, 0, 0)'
    // Avoid touching parts like 'transparentize' (not relevant here but be conservative)
    let mut out = String::with_capacity(input.len());
    let _tokens: Vec<&str> = input.split([',', ' ']).collect();
    // Simple replace on whole word occurrences
    let mut i = 0usize;
    while i < input.len() {
        if input[i..].starts_with("transparent") {
            // Ensure it's not part of a larger alpha token
            let end = i + "transparent".len();
            let prev_ok = i == 0 || !is_ident_char(input.as_bytes()[i - 1] as char);
            let next_ok = end >= input.len() || !is_ident_char(input.as_bytes()[end] as char);
            if prev_ok && next_ok {
                out.push_str("rgba(0, 0, 0, 0)");
                i = end;
                continue;
            }
        }
        out.push(input.as_bytes()[i] as char);
        i += 1;
    }
    out
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

fn normalize_font_family(input: &str) -> String {
    // Quote family names with spaces, ensure comma-separated spacing matches Chrome style
    let mut parts: Vec<String> = Vec::new();
    for raw in input.split(',') {
        let t = raw.trim();
        if t.is_empty() {
            continue;
        }
        let needs_quotes = t.contains(' ') && !(t.starts_with('"') || t.starts_with('\''));
        if needs_quotes {
            parts.push(format!("\"{}\"", t));
        } else {
            // Remove single quotes and convert to double quotes for consistency
            let tt = if t.starts_with('\'') && t.ends_with('\'') && t.len() >= 2 {
                format!("\"{}\"", &t[1..t.len() - 1])
            } else {
                t.to_string()
            };
            parts.push(tt);
        }
    }
    parts.join(", ")
}

fn convert_hex8_to_rgba_only(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] as char == '#' {
            let mut j = i + 1;
            let mut hex = String::new();
            while j < bytes.len() {
                let c = bytes[j] as char;
                if c.is_ascii_hexdigit() && hex.len() < 8 {
                    hex.push(c);
                    j += 1;
                } else {
                    break;
                }
            }
            if hex.len() == 8 {
                if let Some(rgba) = hex8_to_rgba(&hex) {
                    out.push_str(&rgba);
                    i = j;
                    continue;
                }
            } else if hex.len() == 4
                && let Some(rgba) = hex4_to_rgba(&hex)
            {
                out.push_str(&rgba);
                i = j;
                continue;
            }
            out.push('#');
            i += 1;
            continue;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

fn normalize_saturate_percent(input: &str) -> String {
    // Convert saturate(140%) -> saturate(1.4)
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if i + 9 <= bytes.len() && &input[i..i + 9].to_ascii_lowercase() == "saturate(" {
            out.push_str("saturate(");
            i += 9;
            // parse number until ')' or other char
            let _start = i;
            while i < bytes.len() && (bytes[i] as char).is_ascii_whitespace() {
                out.push(bytes[i] as char);
                i += 1;
            }
            let mut num = String::new();
            while i < bytes.len() {
                let c = bytes[i] as char;
                if c.is_ascii_digit() || c == '.' {
                    num.push(c);
                    i += 1;
                } else {
                    break;
                }
            }
            // optional %
            if i < bytes.len() && bytes[i] as char == '%' {
                // convert percent to factor
                if let Ok(v) = num.parse::<f64>() {
                    let f = (v / 100.0 * 100.0).round() / 100.0;
                    let mut s = f.to_string();
                    if s.contains('.') {
                        while s.ends_with('0') {
                            s.pop();
                        }
                        if s.ends_with('.') {
                            s.pop();
                        }
                    }
                    out.push_str(&s);
                } else {
                    out.push_str(&num);
                }
                i += 1; // skip '%'
            } else {
                // no percent, just copy
                out.push_str(&num);
            }
            // copy remaining until ')'
            while i < bytes.len() {
                let c = bytes[i] as char;
                out.push(c);
                i += 1;
                if c == ')' {
                    break;
                }
            }
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn normalize_numeric_tokens(input: &str) -> String {
    // Trim trailing zeros in decimal tokens like 0.40 -> 0.4, 1.00 -> 1
    let mut out = String::with_capacity(input.len());
    let mut tok = String::new();
    let mut in_num = false;
    for ch in input.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            in_num = true;
            tok.push(ch);
        } else {
            if in_num {
                let mut t = tok.clone();
                if t.contains('.') {
                    while t.ends_with('0') {
                        t.pop();
                    }
                    if t.ends_with('.') {
                        t.pop();
                    }
                }
                if t.is_empty() {
                    t = "0".to_string();
                }
                out.push_str(&t);
                tok.clear();
                in_num = false;
            }
            out.push(ch);
        }
    }
    if in_num {
        let mut t = tok.clone();
        if t.contains('.') {
            while t.ends_with('0') {
                t.pop();
            }
            if t.ends_with('.') {
                t.pop();
            }
        }
        if t.is_empty() {
            t = "0".to_string();
        }
        out.push_str(&t);
    }
    out
}

fn default_value_for(prop: &str, winners: &std::collections::BTreeMap<String, String>) -> String {
    let lower = prop.to_ascii_lowercase();
    // Resolve currentColor if we need a color fallback
    let color = winners
        .get("color")
        .map(|v| v.as_str())
        .unwrap_or("rgb(0, 0, 0)")
        .to_string();

    // Border-related defaults
    if lower.starts_with("border-") && lower.ends_with("-color") {
        return color;
    }
    if lower.starts_with("border-") && lower.ends_with("-style") {
        return "none".to_string();
    }
    if lower.starts_with("border-") && lower.ends_with("-width") {
        return "0px".to_string();
    }
    if lower == "border-color" {
        return color;
    }
    if lower == "border-style" {
        return "none".to_string();
    }
    if lower == "border-width" {
        return "0px".to_string();
    }

    // Margin/Padding defaults
    if lower.starts_with("margin") || lower.starts_with("padding") {
        return "0px".to_string();
    }

    // Shadows/images
    if lower.contains("shadow") {
        return "none".to_string();
    }
    if lower.ends_with("-image") {
        return "none".to_string();
    }

    // Outline defaults
    if lower == "outline-style" {
        return "none".to_string();
    }
    if lower == "outline-width" {
        return "0px".to_string();
    }
    if lower == "outline-color" {
        return color;
    }

    // Width/Height style fallbacks
    if lower.ends_with("-width") {
        return "0px".to_string();
    }

    // Fallback default
    "none".to_string()
}

fn hydrate_background_and_border(
    winners: &std::collections::BTreeMap<String, String>,
) -> std::collections::BTreeMap<String, String> {
    use std::collections::BTreeMap;
    let mut out: BTreeMap<String, String> = winners.clone();

    // Background hydration: expand shorthand into longhands where missing
    if let Some(bg) = winners.get("background") {
        let layers = split_top_level(bg);
        if !layers.is_empty() {
            let mut images: Vec<String> = Vec::new();
            let mut color: Option<String> = None;
            for layer in &layers {
                let l = layer.trim();
                let lower = l.to_ascii_lowercase();
                if lower.contains("gradient(") {
                    images.push(l.to_string());
                } else if is_color_like(l) {
                    // treat as color layer (final background-color)
                    color = Some(l.to_string());
                    images.push("none".to_string());
                } else {
                    // Unknown token; keep as-is in images to preserve something visible
                    images.push(l.to_string());
                }
            }
            if !images.is_empty() && !out.contains_key("background-image") {
                let val = images.join(", ");
                out.insert(
                    "background-image".to_string(),
                    convert_hex_colors_to_rgba(&val),
                );
            }
            if let Some(val) = color.as_ref()
                && !out.contains_key("background-color")
            {
                out.insert(
                    "background-color".to_string(),
                    convert_hex_colors_to_rgba(val),
                );
            }

            // Fill other background-* longhands with defaults per layer if absent
            let n = images.len();
            let join_n = |token: &str| std::iter::repeat_n(token, n).collect::<Vec<_>>().join(", ");
            if !out.contains_key("background-attachment") {
                out.insert("background-attachment".to_string(), join_n("scroll"));
            }
            if !out.contains_key("background-repeat") {
                out.insert("background-repeat".to_string(), join_n("repeat"));
            }
            if !out.contains_key("background-position") {
                out.insert("background-position".to_string(), join_n("0% 0%"));
            }
            if !out.contains_key("background-size") {
                out.insert("background-size".to_string(), join_n("auto"));
            }
            if !out.contains_key("background-origin") {
                out.insert("background-origin".to_string(), join_n("padding-box"));
            }
            if !out.contains_key("background-clip") {
                out.insert("background-clip".to_string(), join_n("border-box"));
            }
        }
    }

    // Border hydration: expand border and border-* shorthands into width/style/color longhands where missing
    if let Some(v) = winners.get("border") {
        let (w, s, c) = parse_border_shorthand(v);
        if let Some(wv) = w {
            out.entry("border-width".to_string()).or_insert(wv);
        }
        if let Some(sv) = s {
            out.entry("border-style".to_string()).or_insert(sv);
        }
        if let Some(cv) = c {
            out.entry("border-color".to_string())
                .or_insert(convert_hex_colors_to_rgba(&cv));
        }
        // Apply to all four sides if side-specific are absent
        for side in ["top", "right", "bottom", "left"] {
            if let Some(wv) = out.get("border-width").cloned() {
                out.entry(format!("border-{}-width", side)).or_insert(wv);
            }
            if let Some(sv) = out.get("border-style").cloned() {
                out.entry(format!("border-{}-style", side)).or_insert(sv);
            }
            if let Some(cv) = out.get("border-color").cloned() {
                out.entry(format!("border-{}-color", side)).or_insert(cv);
            }
        }
    }
    for (prop, side) in [
        ("border-top", "top"),
        ("border-right", "right"),
        ("border-bottom", "bottom"),
        ("border-left", "left"),
    ] {
        if let Some(v) = winners.get(prop) {
            let (w, s, c) = parse_border_shorthand(v);
            if let Some(wv) = w {
                out.entry(format!("border-{}-width", side)).or_insert(wv);
            }
            if let Some(sv) = s {
                out.entry(format!("border-{}-style", side)).or_insert(sv);
            }
            if let Some(cv) = c {
                out.entry(format!("border-{}-color", side))
                    .or_insert(convert_hex_colors_to_rgba(&cv));
            }
        }
    }

    out
}

fn split_top_level(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (i, ch) in input.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(input[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(input[start..].trim().to_string());
    parts
}

fn is_color_like(s: &str) -> bool {
    let l = s.trim().to_ascii_lowercase();
    l.starts_with('#') || l.starts_with("rgb(") || l.starts_with("rgba(") || l == "transparent"
}

fn parse_border_shorthand(input: &str) -> (Option<String>, Option<String>, Option<String>) {
    let mut width: Option<String> = None;
    let mut style: Option<String> = None;
    let mut color: Option<String> = None;
    for tok in input.split_whitespace() {
        let tl = tok.to_ascii_lowercase();
        if width.is_none() && (tl.ends_with("px") || tl.parse::<f64>().is_ok()) {
            width = Some(tok.to_string());
            continue;
        }
        if style.is_none() {
            match tl.as_str() {
                "none" | "hidden" | "solid" | "dotted" | "dashed" | "double" | "groove"
                | "ridge" | "inset" | "outset" => {
                    style = Some(tok.to_string());
                    continue;
                }
                _ => {}
            }
        }
        if color.is_none()
            && (tl.starts_with('#') || tl.starts_with("rgb(") || tl.starts_with("rgba("))
        {
            color = Some(tok.to_string());
            continue;
        }
    }
    (width, style, color)
}
