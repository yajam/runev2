use crate::css::types::{
    BackgroundLayer2, ComputedStyle2, Display2, LinearGradient2, RadialGradient2,
};
use csscolorparser::Color as CssColor;
use std::str::FromStr;

pub fn is_whitelisted_property(name: &str) -> bool {
    matches!(
        name,
        "display"
            | "flex-direction"
            | "flex-wrap"
            | "justify-content"
            | "align-items"
            | "text-align"
            | "gap"
            | "column-gap"
            | "row-gap"
            | "grid-template-columns"
            | "grid-template-rows"
            | "grid-auto-flow"
            | "grid-column"
            | "grid-row"
            | "color"
            | "font-family"
            | "font-size"
            | "line-height"
            | "font-weight"
            | "background-color"
            | "background"
            | "border"            // shorthand: width style color
            | "border-top"        // per-side shorthand
            | "border-right"
            | "border-bottom"
            | "border-left"
            | "border-top-width"
            | "border-right-width"
            | "border-bottom-width"
            | "border-left-width"
            | "border-top-color"
            | "border-right-color"
            | "border-bottom-color"
            | "border-left-color"
            | "border-width"
            | "border-color"
            | "border-radius"
            | "box-shadow"
            | "margin"
            | "margin-top"
            | "margin-right"
            | "margin-bottom"
            | "margin-left"
            | "padding"
            | "padding-top"
            | "padding-right"
            | "padding-bottom"
            | "padding-left"
            | "width"
            | "height"
            | "min-width"
            | "min-height"
            | "max-width"
            | "max-height"
            | "object-fit"
    )
}

pub fn apply_whitelisted_property(out: &mut ComputedStyle2, name: &str, raw: &str) {
    let value = raw.trim();
    match name {
        "display" => {
            out.display = match value {
                "none" => Some(Display2::None),
                "flex" | "inline-flex" => Some(Display2::Flex),
                "grid" | "inline-grid" => Some(Display2::Grid),
                "inline" => Some(Display2::Inline),
                _ => Some(Display2::Block),
            };
        }
        "flex-direction" => {
            out.flex_direction = match value {
                "row" => Some(crate::view::LayoutDirection::Row),
                "column" => Some(crate::view::LayoutDirection::Column),
                _ => None,
            };
        }
        "flex-wrap" => {
            out.wrap = match value.to_ascii_lowercase().as_str() {
                "wrap" | "wrap-reverse" => Some(true),
                "nowrap" => Some(false),
                _ => None,
            };
        }
        "justify-content" => {
            out.justify_content = match value {
                "flex-start" | "start" => Some(crate::view::LayoutJustify::Start),
                "center" => Some(crate::view::LayoutJustify::Center),
                "flex-end" | "end" => Some(crate::view::LayoutJustify::End),
                "space-between" => Some(crate::view::LayoutJustify::SpaceBetween),
                "space-around" | "space-evenly" => Some(crate::view::LayoutJustify::SpaceBetween),
                _ => None,
            }
        }
        "align-items" => {
            out.align_items = match value {
                "flex-start" | "start" => Some(crate::view::LayoutAlign::Start),
                "center" => Some(crate::view::LayoutAlign::Center),
                "flex-end" | "end" => Some(crate::view::LayoutAlign::End),
                "stretch" => Some(crate::view::LayoutAlign::Stretch),
                _ => None,
            }
        }
        "text-align" => {
            out.text_align = Some(match value.to_ascii_lowercase().as_str() {
                "center" => crate::css::types::TextAlign2::Center,
                "right" | "end" => crate::css::types::TextAlign2::End,
                _ => crate::css::types::TextAlign2::Start,
            });
        }
        "gap" => out.gap = parse_length(value),
        "column-gap" => out.column_gap = parse_length(value),
        "row-gap" => out.row_gap = parse_length(value),
        "grid-template-columns" => {
            if let Some(t) = parse_grid_template(value) {
                out.grid_template_columns = Some(t)
            }
        }
        "grid-template-rows" => {
            if let Some(t) = parse_grid_template(value) {
                out.grid_template_rows = Some(t)
            }
        }
        "grid-auto-flow" => {
            out.grid_auto_flow = parse_grid_auto_flow(value);
        }
        "grid-column" => {
            if let Some(span) = parse_grid_span(value) {
                out.grid_column_span = Some(span)
            }
        }
        "grid-row" => {
            if let Some(span) = parse_grid_span(value) {
                out.grid_row_span = Some(span)
            }
        }
        "color" => out.color = canonical_color(value),
        "font-family" => out.font_family = Some(value.to_string()),
        "font-size" => out.font_size = parse_length(value),
        "line-height" => out.line_height = parse_length(value),
        "font-weight" => out.font_weight = parse_font_weight(value),
        "background-color" => out.background_color = canonical_color(value),
        "background" => apply_background(out, value),
        "border-radius" => out.corner_radius = parse_border_radius(value),
        // Shorthands
        "border" => apply_border(out, value),
        "border-top" => apply_border_side(out, value, "top"),
        "border-right" => apply_border_side(out, value, "right"),
        "border-bottom" => apply_border_side(out, value, "bottom"),
        "border-left" => apply_border_side(out, value, "left"),
        // Longhands: widths
        "border-width" => {
            if let Some(v) = parse_edge_values(value) {
                out.border_top_width = Some(v[0]);
                out.border_right_width = Some(v[1]);
                out.border_bottom_width = Some(v[2]);
                out.border_left_width = Some(v[3]);
                if v[0] == v[1] && v[1] == v[2] && v[2] == v[3] {
                    out.border_width = Some(v[0]);
                }
            } else if let Some(w) = parse_length(value) {
                out.border_width = Some(w);
                out.border_top_width = Some(w);
                out.border_right_width = Some(w);
                out.border_bottom_width = Some(w);
                out.border_left_width = Some(w);
            }
        }
        "border-top-width" => out.border_top_width = parse_length(value),
        "border-right-width" => out.border_right_width = parse_length(value),
        "border-bottom-width" => out.border_bottom_width = parse_length(value),
        "border-left-width" => out.border_left_width = parse_length(value),
        // Longhands: colors
        "border-color" => {
            if let Some(v) = parse_edge_colors(value) {
                out.border_top_color = v[0].clone();
                out.border_right_color = v[1].clone();
                out.border_bottom_color = v[2].clone();
                out.border_left_color = v[3].clone();
                if v[0] == v[1] && v[1] == v[2] && v[2] == v[3] {
                    out.border_color = v[0].clone();
                }
            } else if let Some(c) = canonical_color(value) {
                out.border_color = Some(c.clone());
                out.border_top_color = Some(c.clone());
                out.border_right_color = Some(c.clone());
                out.border_bottom_color = Some(c.clone());
                out.border_left_color = Some(c);
            }
        }
        "border-top-color" => out.border_top_color = canonical_color(value),
        "border-right-color" => out.border_right_color = canonical_color(value),
        "border-bottom-color" => out.border_bottom_color = canonical_color(value),
        "border-left-color" => out.border_left_color = canonical_color(value),
        "box-shadow" => out.box_shadow = parse_box_shadow(value),
        "margin" => {
            if let Some((vals, autos)) = parse_edge_values_auto(value) {
                out.margin.apply(vals);
                out.margin_left_auto = autos[3];
                out.margin_right_auto = autos[1];
            }
        }
        "margin-top" => {
            if value.eq_ignore_ascii_case("auto") {
                // top/bottom auto not used for centering; treat as 0 length
                out.margin.top = 0.0;
            } else if let Some(v) = parse_length(value) {
                out.margin.top = v
            }
        }
        "margin-right" => {
            if value.eq_ignore_ascii_case("auto") {
                out.margin_right_auto = true;
                out.margin.right = 0.0;
            } else if let Some(v) = parse_length(value) {
                out.margin.right = v
            }
        }
        "margin-bottom" => {
            if value.eq_ignore_ascii_case("auto") {
                out.margin.bottom = 0.0;
            } else if let Some(v) = parse_length(value) {
                out.margin.bottom = v
            }
        }
        "margin-left" => {
            if value.eq_ignore_ascii_case("auto") {
                out.margin_left_auto = true;
                out.margin.left = 0.0;
            } else if let Some(v) = parse_length(value) {
                out.margin.left = v
            }
        }
        "padding" => {
            if let Some(v) = parse_edge_values(value) {
                out.padding.apply(v)
            }
        }
        "padding-top" => {
            if let Some(v) = parse_length(value) {
                out.padding.top = v
            }
        }
        "padding-right" => {
            if let Some(v) = parse_length(value) {
                out.padding.right = v
            }
        }
        "padding-bottom" => {
            if let Some(v) = parse_length(value) {
                out.padding.bottom = v
            }
        }
        "padding-left" => {
            if let Some(v) = parse_length(value) {
                out.padding.left = v
            }
        }
        "width" => out.width = parse_length(value),
        "height" => out.height = parse_length(value),
        "min-width" => out.min_width = parse_length(value),
        "min-height" => out.min_height = parse_length(value),
        "max-width" => out.max_width = parse_length(value),
        "max-height" => out.max_height = parse_length(value),
        "object-fit" => {
            out.object_fit = match value {
                "cover" => Some(crate::view::ImageContentFit::Cover),
                "contain" => Some(crate::view::ImageContentFit::Contain),
                "fill" => Some(crate::view::ImageContentFit::Fill),
                _ => None,
            }
        }
        _ => {}
    }
}

pub fn parse_edge_values(input: &str) -> Option<[f64; 4]> {
    let parts: Vec<f64> = input.split_whitespace().filter_map(parse_length).collect();
    match parts.as_slice() {
        [] => None,
        [single] => Some([*single, *single, *single, *single]),
        [vertical, horizontal] => Some([*vertical, *horizontal, *vertical, *horizontal]),
        [top, horizontal, bottom] => Some([*top, *horizontal, *bottom, *horizontal]),
        [top, right, bottom, left, ..] => Some([*top, *right, *bottom, *left]),
    }
}

#[allow(unused_assignments, unused_mut)]
fn parse_edge_values_auto(value: &str) -> Option<([f64; 4], [bool; 4])> {
    let tokens: Vec<&str> = value.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }
    let mut lens = [0.0f64; 4];
    let mut autos = [false; 4];
    let mut parse_tok = |tok: &str| -> (f64, bool) {
        if tok.eq_ignore_ascii_case("auto") {
            (0.0, true)
        } else {
            (parse_length(tok).unwrap_or(0.0), false)
        }
    };
    match tokens.len() {
        1 => {
            let (v, a) = parse_tok(tokens[0]);
            lens = [v, v, v, v];
            autos = [a, a, a, a];
        }
        2 => {
            let (v, a) = parse_tok(tokens[0]);
            let (h, ah) = parse_tok(tokens[1]);
            lens = [v, h, v, h];
            autos = [a, ah, a, ah];
        }
        3 => {
            let (t, at) = parse_tok(tokens[0]);
            let (h, ah) = parse_tok(tokens[1]);
            let (b, ab) = parse_tok(tokens[2]);
            lens = [t, h, b, h];
            autos = [at, ah, ab, ah];
        }
        _ => {
            let (t, at) = parse_tok(tokens[0]);
            let (r, ar) = parse_tok(tokens[1]);
            let (b, ab) = parse_tok(tokens[2]);
            let (l, al) = parse_tok(tokens[3]);
            lens = [t, r, b, l];
            autos = [at, ar, ab, al];
        }
    }
    Some((lens, autos))
}

pub fn parse_length(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(stripped) = trimmed.strip_suffix("px") {
        return stripped.trim().parse().ok();
    }
    trimmed.parse().ok()
}

pub fn parse_font_weight(value: &str) -> Option<f64> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "normal" => Some(400.0),
        "bold" => Some(700.0),
        "lighter" => Some(300.0),
        "bolder" => Some(700.0),
        other => other.parse::<f64>().ok(),
    }
}

pub fn canonical_color(raw: &str) -> Option<String> {
    let v = raw.trim();
    // Treat tokens that mean "no explicit color" as None
    if v.eq_ignore_ascii_case("none")
        || v.eq_ignore_ascii_case("inherit")
        || v.eq_ignore_ascii_case("currentcolor")
    {
        return None;
    }
    if v.starts_with("var(") {
        // Preserve var(...) so higher layer can resolve using :root variables.
        return Some(v.to_string());
    }
    if let Ok(c) = CssColor::from_str(v) {
        let r = (c.r * 255.0).round().clamp(0.0, 255.0) as u8;
        let g = (c.g * 255.0).round().clamp(0.0, 255.0) as u8;
        let b = (c.b * 255.0).round().clamp(0.0, 255.0) as u8;
        let a = c.a;
        if (a - 1.0).abs() < 1e-6 {
            Some(format!("#{:02x}{:02x}{:02x}", r, g, b))
        } else {
            Some(format!("rgba({},{},{},{:.2})", r, g, b, a))
        }
    } else {
        None
    }
}

fn apply_background(out: &mut ComputedStyle2, value: &str) {
    let v = value.trim();
    // Split top-level layers by commas (ignoring commas inside parentheses)
    let mut layers: Vec<&str> = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (i, ch) in v.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                layers.push(v[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    layers.push(v[start..].trim());
    // Build ordered background_layers (first = topmost layer)
    out.background_layers.clear();
    for layer in &layers {
        if let Some(rad) = parse_radial_gradient(layer) {
            out.background_layers.push(BackgroundLayer2::Radial(rad));
            continue;
        }
        if let Some(lin) = parse_linear_gradient(layer) {
            out.background_layers.push(BackgroundLayer2::Linear(lin));
            continue;
        }
        if layer.starts_with("var(") {
            out.background_layers
                .push(BackgroundLayer2::Solid(layer.to_string()));
            continue;
        }
        if let Some(c) = canonical_color(layer) {
            out.background_layers.push(BackgroundLayer2::Solid(c));
            continue;
        }
    }
    // Also populate fallback singles to keep older code paths working
    // Prefer the first gradient (topmost) if present; else last solid color.
    for l in &out.background_layers {
        match l {
            BackgroundLayer2::Radial(rad) => {
                out.background_radial = Some(rad.clone());
                out.background_gradient = None;
                out.background_color = None;
                break;
            }
            BackgroundLayer2::Linear(lin) => {
                if out.background_radial.is_none() {
                    out.background_gradient = Some(lin.clone());
                    out.background_color = None;
                    // keep scanning in case a radial appears later (unlikely in valid CSS ordering)
                }
            }
            _ => {}
        }
    }
    if out.background_radial.is_none() && out.background_gradient.is_none() {
        if let Some(last) = out.background_layers.iter().rev().find_map(|l| match l {
            BackgroundLayer2::Solid(c) => Some(c.clone()),
            _ => None,
        }) {
            out.background_color = Some(last);
        }
    }
}

fn parse_grid_auto_flow(input: &str) -> Option<crate::css::types::GridAutoFlow2> {
    let mut _row = false;
    let mut col = false;
    let mut dense = false;
    for token in input.split_whitespace() {
        let t = token.trim().to_ascii_lowercase();
        match t.as_str() {
            "row" => _row = true,
            "column" => col = true,
            "dense" => dense = true,
            _ => {}
        }
    }
    let base = if col {
        crate::css::types::GridAutoFlow2::Column
    } else {
        crate::css::types::GridAutoFlow2::Row
    };
    Some(match (base, dense) {
        (crate::css::types::GridAutoFlow2::Row, false) => crate::css::types::GridAutoFlow2::Row,
        (crate::css::types::GridAutoFlow2::Row, true) => crate::css::types::GridAutoFlow2::RowDense,
        (crate::css::types::GridAutoFlow2::Column, false) => {
            crate::css::types::GridAutoFlow2::Column
        }
        (crate::css::types::GridAutoFlow2::Column, true) => {
            crate::css::types::GridAutoFlow2::ColumnDense
        }
        _ => crate::css::types::GridAutoFlow2::Row,
    })
}

fn parse_grid_span(input: &str) -> Option<u16> {
    let s = input.trim().to_ascii_lowercase();
    // Accept forms: "span N" or just a number as shorthand for start/end (we treat as span)
    if let Some(rest) = s.strip_prefix("span") {
        let n = rest.trim().split_whitespace().next()?;
        if let Ok(v) = n.parse::<u16>() {
            return Some(v.max(1));
        }
    }
    // Fallback: if a single number, interpret as span 1 (ignore); if two numbers separated by '/', ignore
    if let Ok(v) = s.parse::<u16>() {
        return Some(v.max(1));
    }
    None
}

fn parse_grid_template(value: &str) -> Option<crate::css::types::GridTemplate2> {
    use crate::css::types::{GridTemplate2, GridTrackSize2};
    let mut tracks: Vec<GridTrackSize2> = Vec::new();
    let v = value.trim();
    if v.eq_ignore_ascii_case("none") || v.is_empty() {
        return Some(GridTemplate2 { tracks });
    }
    // Handle repeat(N, 1fr)
    if let Some(inner) = v
        .strip_prefix("repeat(")
        .and_then(|rest| rest.strip_suffix(")"))
    {
        let mut parts = inner.split(',').map(|p| p.trim());
        let count = parts.next()?.parse::<u16>().ok()? as usize;
        let track = parts.next()?.to_ascii_lowercase();
        let sizing = if track.ends_with("fr") {
            let num = track.trim_end_matches("fr").trim();
            let val = if num.is_empty() {
                1.0
            } else {
                num.parse::<f64>().ok()?
            };
            GridTrackSize2::Fr(val)
        } else if track.ends_with("px") {
            let num = track.trim_end_matches("px").trim();
            GridTrackSize2::Px(num.parse::<f64>().ok()?)
        } else if track == "auto" {
            GridTrackSize2::Auto
        } else {
            return None;
        };
        for _ in 0..count {
            tracks.push(sizing.clone());
        }
        return Some(GridTemplate2 { tracks });
    }
    // Otherwise parse a space-separated list of tracks
    for token in v.split_whitespace() {
        let t = token.to_ascii_lowercase();
        if t.ends_with("fr") {
            let num = t.trim_end_matches("fr").trim();
            let val = if num.is_empty() {
                1.0
            } else {
                num.parse::<f64>().ok()?
            };
            tracks.push(GridTrackSize2::Fr(val));
        } else if t.ends_with("px") {
            let num = t.trim_end_matches("px").trim();
            tracks.push(GridTrackSize2::Px(num.parse::<f64>().ok()?));
        } else if t == "auto" {
            tracks.push(GridTrackSize2::Auto);
        } else {
            // unsupported (minmax, percentages) → skip
        }
    }
    Some(GridTemplate2 { tracks })
}

fn parse_linear_gradient(input: &str) -> Option<LinearGradient2> {
    let s = input.trim();
    let lower = s.to_ascii_lowercase();
    if !lower.starts_with("linear-gradient(") || !s.ends_with(')') {
        return None;
    }
    let inner = &s[s.find('(')? + 1..s.rfind(')')?];

    // Split top-level by commas, respecting nested parentheses
    let mut parts: Vec<&str> = Vec::new();
    let mut depth = 0i32;
    let mut start_ix = 0usize;
    for (i, ch) in inner.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(inner[start_ix..i].trim());
                start_ix = i + 1;
            }
            _ => {}
        }
    }
    parts.push(inner[start_ix..].trim());
    if parts.len() < 2 {
        return None;
    }

    // Angle can be the first item
    let mut angle: f64 = 0.0;
    let first = parts[0];
    if let Some(num) = first.strip_suffix("deg").map(|v| v.trim()) {
        if let Ok(a) = num.parse::<f64>() {
            angle = a;
            parts.remove(0);
        }
    } else if first.to_ascii_lowercase().starts_with("to ") {
        // Minimal support for direction keywords frequently used by authors
        // Map CSS directions to degrees (0deg = pointing right in our convention)
        let dir = first[3..].trim().to_ascii_lowercase();
        angle = match dir.as_str() {
            "top" => 270.0,
            "bottom" => 90.0,
            "left" => 180.0,
            "right" => 0.0,
            "top right" | "right top" => 315.0,
            "top left" | "left top" => 225.0,
            "bottom right" | "right bottom" => 45.0,
            "bottom left" | "left bottom" => 135.0,
            _ => 0.0,
        };
        parts.remove(0);
    }
    if parts.len() < 2 {
        return None;
    }

    // Helper: try to parse a color stop candidate from a chunk
    fn parse_color_stop_candidate(chunk: &str) -> Option<(String, Option<f64>)> {
        let t = chunk.trim();
        if t.is_empty() {
            return None;
        }
        if let Some(color) = canonical_color(t) {
            return Some((color, None));
        }
        if let Some((color_part, pos_part)) = t.rsplit_once(char::is_whitespace) {
            let color_str = color_part.trim();
            let pos_str = pos_part.trim();
            if let Some(color) = canonical_color(color_str) {
                if let Some(pct) = pos_str
                    .strip_suffix('%')
                    .and_then(|v| v.trim().parse::<f64>().ok())
                {
                    return Some((color, Some(pct / 100.0)));
                }
                if let Ok(v) = pos_str.parse::<f64>() {
                    return Some((color, Some(v)));
                }
                return Some((color, None));
            }
        }
        None
    }

    // Parse color stops
    let mut raw_stops: Vec<(String, Option<f64>)> = Vec::new();
    for p in parts {
        if let Some((c, off)) = parse_color_stop_candidate(p) {
            raw_stops.push((c, off));
        }
    }
    if raw_stops.is_empty() {
        return None;
    }

    // Resolve offsets: first=0, last=1 by default, interpolate interior
    let n = raw_stops.len();
    let mut offs: Vec<Option<f64>> = raw_stops.iter().map(|(_, o)| *o).collect();
    if offs[0].is_none() {
        offs[0] = Some(0.0);
    }
    if offs[n - 1].is_none() {
        offs[n - 1] = Some(1.0);
    }
    let mut i = 0usize;
    while i + 1 < n {
        if let Some(left) = offs[i] {
            let mut j = i + 1;
            while j < n && offs[j].is_none() {
                j += 1;
            }
            if j < n {
                let right = offs[j].unwrap();
                let gaps = j - i - 1;
                if gaps > 0 {
                    let step = (right - left) / (gaps as f64 + 1.0);
                    for k in 1..=gaps {
                        offs[i + k] = Some(left + step * k as f64);
                    }
                }
                i = j;
            } else {
                break;
            }
        } else {
            i += 1;
        }
    }
    if offs.iter().any(|o| o.is_none()) {
        for (idx, o) in offs.iter_mut().enumerate() {
            if o.is_none() {
                *o = Some(idx as f64 / (n - 1) as f64);
            }
        }
    }

    let stops = raw_stops
        .into_iter()
        .zip(offs.into_iter())
        .map(|((color, _), off)| crate::css::types::ColorStop {
            color,
            offset: off.unwrap_or(0.0),
        })
        .collect::<Vec<_>>();

    Some(LinearGradient2 { angle, stops })
}

pub fn parse_radial_gradient(input: &str) -> Option<RadialGradient2> {
    let s = input.trim();
    let lower = s.to_ascii_lowercase();
    if !lower.starts_with("radial-gradient(") || !s.ends_with(')') {
        return None;
    }
    let inner = &s[s.find('(')? + 1..s.rfind(')')?];

    // Split top-level by commas, respecting nested parentheses
    let mut parts: Vec<&str> = Vec::new();
    let mut depth = 0i32;
    let mut start_ix = 0usize;
    for (i, ch) in inner.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(inner[start_ix..i].trim());
                start_ix = i + 1;
            }
            _ => {}
        }
    }
    parts.push(inner[start_ix..].trim());
    if parts.is_empty() {
        return None;
    }

    // Helper: try to parse a color stop candidate from a chunk
    fn parse_color_stop_candidate(chunk: &str) -> Option<(String, Option<f64>)> {
        let t = chunk.trim();
        if t.is_empty() {
            return None;
        }
        // Try as a whole color first
        if let Some(color) = canonical_color(t) {
            return Some((color, None));
        }
        // Otherwise split by last whitespace, if suffix looks like a position
        if let Some((color_part, pos_part)) = t.rsplit_once(char::is_whitespace) {
            let color_str = color_part.trim();
            let pos_str = pos_part.trim();
            if let Some(color) = canonical_color(color_str) {
                if let Some(pct) = pos_str
                    .strip_suffix('%')
                    .and_then(|v| v.trim().parse::<f64>().ok())
                {
                    return Some((color, Some(pct / 100.0)));
                }
                if let Ok(v) = pos_str.parse::<f64>() {
                    return Some((color, Some(v)));
                }
                // Not a position → still accept the color
                return Some((color, None));
            }
        }
        None
    }

    let first_is_color = parse_color_stop_candidate(parts[0]).is_some();

    let mut cx = 0.5f64;
    let mut cy = 0.5f64;
    let mut rx = 1.0f64;
    let mut ry = 1.0f64;

    let color_parts: &[&str];
    if first_is_color {
        color_parts = &parts[..];
    } else {
        // Parse preamble (shape/size/position)
        let pre = parts[0].trim();
        let mut before_at = pre;
        let mut pos_str = "";
        if let Some(idx) = pre.to_ascii_lowercase().find(" at ") {
            before_at = pre[..idx].trim();
            pos_str = pre[idx + 4..].trim();
        }
        // Position: keywords or percentages
        if !pos_str.is_empty() {
            let mut it = pos_str.split_whitespace();
            if let Some(x) = it.next() {
                cx = match x.to_ascii_lowercase().as_str() {
                    "left" => 0.0,
                    "center" => 0.5,
                    "right" => 1.0,
                    _ => x
                        .trim_end_matches('%')
                        .parse::<f64>()
                        .ok()
                        .map(|p| p / 100.0)
                        .unwrap_or(cx),
                };
            }
            if let Some(y) = it.next() {
                cy = match y.to_ascii_lowercase().as_str() {
                    "top" => 0.0,
                    "center" => 0.5,
                    "bottom" => 1.0,
                    _ => y
                        .trim_end_matches('%')
                        .parse::<f64>()
                        .ok()
                        .map(|p| p / 100.0)
                        .unwrap_or(cy),
                };
            }
        }
        // Size/shape
        if !before_at.is_empty() {
            let mut toks = before_at.split_whitespace().collect::<Vec<_>>();
            let mut circle = false;
            if let Some(i) = toks.iter().position(|t| {
                t.eq_ignore_ascii_case(&"circle") || t.eq_ignore_ascii_case(&"ellipse")
            }) {
                circle = toks[i].eq_ignore_ascii_case("circle");
                toks.remove(i);
            }
            if !toks.is_empty() {
                let size_str = toks.join(" ");
                match size_str.to_ascii_lowercase().as_str() {
                    "closest-side" | "farthest-side" | "closest-corner" | "farthest-corner" => {
                        rx = 1.0;
                        ry = 1.0;
                    }
                    _ => {
                        let dims = size_str.split_whitespace().collect::<Vec<_>>();
                        if let Some(a) = dims.get(0).and_then(|v| parse_length(v)) {
                            rx = a;
                        }
                        if let Some(b) = dims.get(1).and_then(|v| parse_length(v)) {
                            ry = b;
                        } else {
                            ry = rx;
                        }
                    }
                }
            }
            if circle {
                ry = rx;
            }
        }
        color_parts = &parts[1..];
        if color_parts.is_empty() {
            return None;
        }
    }

    // Parse color stops
    let mut raw_stops: Vec<(String, Option<f64>)> = Vec::new();
    for p in color_parts.iter().copied() {
        if let Some((c, off)) = parse_color_stop_candidate(p) {
            raw_stops.push((c, off));
        }
    }
    if raw_stops.is_empty() {
        return None;
    }

    // Resolve offsets: first=0 if missing, last=1 if missing, interpolate interior
    let n = raw_stops.len();
    let mut offs: Vec<Option<f64>> = raw_stops.iter().map(|(_, o)| *o).collect();
    if offs[0].is_none() {
        offs[0] = Some(0.0);
    }
    if offs[n - 1].is_none() {
        offs[n - 1] = Some(1.0);
    }
    let mut i = 0usize;
    while i + 1 < n {
        if let Some(left) = offs[i] {
            let mut j = i + 1;
            while j < n && offs[j].is_none() {
                j += 1;
            }
            if j < n {
                let right = offs[j].unwrap();
                let gaps = j - i - 1;
                if gaps > 0 {
                    let step = (right - left) / (gaps as f64 + 1.0);
                    for k in 1..=gaps {
                        offs[i + k] = Some(left + step * k as f64);
                    }
                }
                i = j;
            } else {
                break;
            }
        } else {
            i += 1;
        }
    }
    if offs.iter().any(|o| o.is_none()) {
        for (idx, o) in offs.iter_mut().enumerate() {
            if o.is_none() {
                *o = Some(idx as f64 / (n - 1) as f64);
            }
        }
    }

    let stops_vec: Vec<crate::css::types::ColorStop> = raw_stops
        .into_iter()
        .zip(offs.into_iter())
        .map(|((color, _), off)| crate::css::types::ColorStop {
            color,
            offset: off.unwrap_or(0.0),
        })
        .collect();

    Some(RadialGradient2 {
        cx,
        cy,
        rx,
        ry,
        stops: stops_vec,
    })
}

fn parse_border_radius(input: &str) -> Option<f64> {
    // Take the first length, convert to px if provided without unit.
    let first = input.split('/').next().unwrap_or("").trim();
    let token = first.split_whitespace().next().unwrap_or("");
    super::canonical::parse_length(token)
}

/// Parse a border shorthand into (width, color). Ignores style keywords.
fn parse_border_tokens(value: &str) -> (Option<f64>, Option<String>) {
    // Tokenize by whitespace but keep function-like arguments intact, e.g.
    // "rgba(255, 255, 255, 0.06)" should remain a single token.
    fn split_preserving_parens(s: &str) -> Vec<String> {
        let mut toks: Vec<String> = Vec::new();
        let mut current = String::new();
        let mut depth: i32 = 0;
        for ch in s.chars() {
            match ch {
                '(' => {
                    depth += 1;
                    current.push(ch);
                }
                ')' => {
                    depth -= 1;
                    current.push(ch);
                }
                c if c.is_whitespace() && depth == 0 => {
                    if !current.trim().is_empty() {
                        toks.push(current.trim().to_string());
                        current.clear();
                    }
                }
                _ => current.push(ch),
            }
        }
        if !current.trim().is_empty() {
            toks.push(current.trim().to_string());
        }
        toks
    }
    let mut width: Option<f64> = None;
    let mut color: Option<String> = None;
    for token in split_preserving_parens(value) {
        if width.is_none() {
            if let Some(w) = parse_length(&token) {
                width = Some(w);
                continue;
            }
        }
        // skip style keyword like solid/dashed/etc.
        match token.to_ascii_lowercase().as_str() {
            "none" | "hidden" | "solid" | "dotted" | "dashed" | "double" | "groove" | "ridge"
            | "inset" | "outset" => {
                continue;
            }
            _ => {}
        }
        if color.is_none() {
            if let Some(c) = canonical_color(&token) {
                color = Some(c);
                continue;
            }
        }
    }
    (width, color)
}

fn apply_border(out: &mut ComputedStyle2, value: &str) {
    let (width, color) = parse_border_tokens(value);
    if width.is_some() {
        out.border_width = width;
        // hydrate per-side for equality
        if let Some(w) = out.border_width {
            out.border_top_width = Some(w);
            out.border_right_width = Some(w);
            out.border_bottom_width = Some(w);
            out.border_left_width = Some(w);
        }
    }
    if color.is_some() {
        out.border_color = color.clone();
        if let Some(c) = color {
            out.border_top_color = Some(c.clone());
            out.border_right_color = Some(c.clone());
            out.border_bottom_color = Some(c.clone());
            out.border_left_color = Some(c);
        }
    }
}

fn apply_border_side(out: &mut ComputedStyle2, value: &str, side: &str) {
    let (width, color) = parse_border_tokens(value);
    match side {
        "top" => {
            if width.is_some() {
                out.border_top_width = width;
            }
            if color.is_some() {
                out.border_top_color = color.clone();
            }
        }
        "right" => {
            if width.is_some() {
                out.border_right_width = width;
            }
            if color.is_some() {
                out.border_right_color = color.clone();
            }
        }
        "bottom" => {
            if width.is_some() {
                out.border_bottom_width = width;
            }
            if color.is_some() {
                out.border_bottom_color = color.clone();
            }
        }
        "left" => {
            if width.is_some() {
                out.border_left_width = width;
            }
            if color.is_some() {
                out.border_left_color = color.clone();
            }
        }
        _ => {}
    }
}

fn parse_edge_colors(value: &str) -> Option<[Option<String>; 4]> {
    // Split tokens preserving rgba() and var() contents
    let mut toks: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut depth: i32 = 0;
    for ch in value.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth -= 1;
                current.push(ch);
            }
            c if c.is_whitespace() && depth == 0 => {
                if !current.trim().is_empty() {
                    toks.push(current.trim().to_string());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        toks.push(current.trim().to_string());
    }
    if toks.is_empty() {
        return None;
    }
    let mut cols: Vec<Option<String>> = Vec::new();
    for t in toks {
        cols.push(canonical_color(&t));
    }
    let out = match cols.as_slice() {
        [] => return None,
        [a] => [a.clone(), a.clone(), a.clone(), a.clone()],
        [v, h] => [v.clone(), h.clone(), v.clone(), h.clone()],
        [t, h, b] => [t.clone(), h.clone(), b.clone(), h.clone()],
        [t, r, b, l, ..] => [t.clone(), r.clone(), b.clone(), l.clone()],
    };
    Some(out)
}

fn parse_box_shadow(input: &str) -> Option<crate::css::types::BoxShadow2> {
    let s = input.trim();
    if s.eq_ignore_ascii_case("none") {
        return None;
    }
    // Very simple: [offset-x] [offset-y] [blur]? [color]? order, px units optional
    let mut offset_x: Option<f64> = None;
    let mut offset_y: Option<f64> = None;
    let mut blur: f64 = 0.0;
    let mut color: Option<String> = None;
    for token in s.split_whitespace() {
        if offset_x.is_none() {
            if let Some(v) = parse_length(token) {
                offset_x = Some(v);
                continue;
            }
        }
        if offset_y.is_none() {
            if let Some(v) = parse_length(token) {
                offset_y = Some(v);
                continue;
            }
        }
        if blur == 0.0 {
            if let Some(v) = parse_length(token) {
                blur = v;
                continue;
            }
        }
        if color.is_none() {
            if let Some(c) = canonical_color(token) {
                color = Some(c);
                continue;
            }
        }
    }
    Some(crate::css::types::BoxShadow2 {
        offset_x: offset_x.unwrap_or(0.0),
        offset_y: offset_y.unwrap_or(0.0),
        blur,
        color: color.unwrap_or_else(|| "rgba(0,0,0,0.25)".to_string()),
    })
}
