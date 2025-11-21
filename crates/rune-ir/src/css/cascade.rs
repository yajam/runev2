use std::collections::{HashMap, HashSet};
use std::fs;
use std::time::{Duration, Instant};

use scraper::{ElementRef, Html, Selector};
use url::Url;

use crate::css::diagnostics::diagnostics_enabled;
use crate::css::properties::canonical::{apply_whitelisted_property, is_whitelisted_property};
use crate::css::servo_dom::DomElement;
#[cfg(feature = "servo_selectors")]
use crate::css::servo_selectors::{
    ServoSelectorList, matches_subset, parse_list as parse_selector_list, specificity_of,
};
#[cfg(not(feature = "servo_selectors"))]
#[derive(Clone, Debug)]
struct ServoSelectorList(String);
#[cfg(not(feature = "servo_selectors"))]
fn parse_selector_list(input: &str) -> Option<ServoSelectorList> {
    Some(ServoSelectorList(input.to_string()))
}
#[cfg(not(feature = "servo_selectors"))]
fn matches_subset(_list: &ServoSelectorList, _el: &DomElement) -> bool {
    false
}
#[cfg(not(feature = "servo_selectors"))]
fn specificity_of(_list: &ServoSelectorList) -> u32 {
    0
}
use crate::css::types::{ComputedStyle2, budgets_from_env};
use crate::html::HtmlOptions;

#[derive(Debug, Clone)]
struct CssDeclarationV2 {
    name: String,
    value: String,
}

#[derive(Debug, Clone)]
struct CssRuleV2 {
    selector: ServoSelectorList,
    declarations: Vec<CssDeclarationV2>,
    specificity: u32,
    order: usize,
}

#[derive(Default)]
struct RuleIndex {
    by_id: HashMap<String, Vec<usize>>,    // id value → rule indices
    by_class: HashMap<String, Vec<usize>>, // class → rule indices
    by_tag: HashMap<String, Vec<usize>>,   // tag → rule indices
    universal: Vec<usize>,                 // rules with no key on rightmost
}

impl RuleIndex {
    fn add_keys(
        &mut self,
        idx: usize,
        id: Option<String>,
        classes: Vec<String>,
        tag: Option<String>,
    ) {
        let mut keyed = false;
        if let Some(idv) = id {
            self.by_id.entry(idv).or_default().push(idx);
            keyed = true;
        }
        for c in classes {
            self.by_class.entry(c).or_default().push(idx);
            keyed = true;
        }
        if let Some(t) = tag {
            self.by_tag.entry(t).or_default().push(idx);
            keyed = true;
        }
        if !keyed {
            self.universal.push(idx);
        }
    }

    fn candidates_for(&self, el: &ElementRef) -> Vec<usize> {
        let mut set: HashSet<usize> = HashSet::new();
        if let Some(id) = el.value().attr("id") {
            if let Some(v) = self.by_id.get(id) {
                set.extend(v);
            }
        }
        for class in el.value().classes() {
            if let Some(v) = self.by_class.get(class) {
                set.extend(v);
            }
        }
        let tag = el.value().name().to_ascii_lowercase();
        if let Some(v) = self.by_tag.get(&tag) {
            set.extend(v);
        }
        set.extend(self.universal.iter().copied());
        set.into_iter().collect()
    }
}

pub struct StyleSheet {
    rules: Vec<CssRuleV2>,
    index: RuleIndex,
    root_vars: HashMap<String, String>,
}

impl StyleSheet {
    pub fn empty() -> Self {
        Self {
            rules: Vec::new(),
            index: RuleIndex::default(),
            root_vars: HashMap::new(),
        }
    }

    pub fn from_sources(sources: &[String]) -> Self {
        let mut sheet = StyleSheet::empty();
        let budgets = budgets_from_env().clone();
        let start = Instant::now();
        let mut order = 0usize;
        for css in sources {
            if sheet.rules.len() >= budgets.max_rules {
                break;
            }
            for (selectors, body) in split_rules(css) {
                if sheet.rules.len() >= budgets.max_rules {
                    break;
                }
                let list = match parse_selector_list(&selectors) {
                    Some(l) => l,
                    None => continue,
                };
                let decls = parse_declarations(&body);
                if decls.is_empty() {
                    continue;
                }
                if start.elapsed() > Duration::from_millis(budgets.timeout_ms) {
                    break;
                }
                order += 1;
                let idx = sheet.rules.len();
                let spec = specificity_of(&list);
                let (id_key, class_keys, tag_key) = rightmost_keys(&selectors);
                let rule = CssRuleV2 {
                    selector: list,
                    declarations: decls.clone(),
                    specificity: spec,
                    order,
                };
                if selectors.contains(":root") {
                    for d in &decls {
                        if let Some(name) = d.name.strip_prefix("--") {
                            sheet
                                .root_vars
                                .insert(format!("--{}", name), d.value.clone());
                        }
                    }
                }
                sheet.index.add_keys(idx, id_key, class_keys, tag_key);
                sheet.rules.push(rule);
            }
        }
        sheet
    }

    pub fn compute_for(&self, el: &ElementRef, inline: Option<&str>) -> ComputedStyle2 {
        let budgets = budgets_from_env().clone();
        let start = Instant::now();
        let mut out = ComputedStyle2::default();
        let mut winners: HashMap<String, (u32, usize, String)> = HashMap::new();
        let mut matched_rules = 0usize;
        let dom_el = DomElement::new(el.clone());

        // Gather candidates via index
        let mut candidates = self.index.candidates_for(el);
        // Soft cap
        if candidates.len() > budgets.max_rules {
            candidates.truncate(budgets.max_rules);
        }

        // Verify full selector chain matches, then apply
        for idx in candidates {
            if start.elapsed() > Duration::from_millis(budgets.timeout_ms) {
                break;
            }
            let rule = &self.rules[idx];
            if !matches_subset(&rule.selector, &dom_el) {
                continue;
            }
            matched_rules += 1;
            // apply declarations with specificity ordering
            for decl in &rule.declarations {
                let key = decl.name.clone();
                let prio = (rule.specificity, rule.order);
                let should_set = match winners.get(&key) {
                    Some(existing) => prio_gt2(&prio, &(existing.0, existing.1)),
                    None => true,
                };
                if should_set {
                    let resolved = resolve_vars(&decl.value, &self.root_vars);
                    winners.insert(key, (rule.specificity, rule.order, resolved));
                }
            }
        }

        // Apply winning properties through whitelist
        for (name, (_s, _o, value)) in winners.into_iter() {
            if is_whitelisted_property(&name) {
                apply_whitelisted_property(&mut out, &name, &value);
            }
        }

        // Inline overrides last
        if let Some(inline_raw) = inline {
            for (name, value) in parse_inline_declarations(inline_raw) {
                if is_whitelisted_property(&name) {
                    apply_whitelisted_property(&mut out, &name, &value);
                }
            }
        }

        if diagnostics_enabled("css") {
            tracing::info!(matched_rules, properties = %format_applied(&out), "diagnostics: cssv2 cascade applied");
        }
        out
    }

    /// Compute the raw property winner map for an element, applying selector cascade and
    /// inline overrides, with var() resolved from :root. Does not filter by whitelist.
    pub fn compute_raw_for(
        &self,
        el: &ElementRef,
        inline: Option<&str>,
    ) -> std::collections::BTreeMap<String, String> {
        use std::collections::BTreeMap;
        let budgets = budgets_from_env().clone();
        let start = Instant::now();
        let mut winners: HashMap<String, (u32, usize, String)> = HashMap::new();
        let dom_el = DomElement::new(el.clone());

        let mut candidates = self.index.candidates_for(el);
        if candidates.len() > budgets.max_rules {
            candidates.truncate(budgets.max_rules);
        }

        for idx in candidates {
            if start.elapsed() > Duration::from_millis(budgets.timeout_ms) {
                break;
            }
            let rule = &self.rules[idx];
            if !matches_subset(&rule.selector, &dom_el) {
                continue;
            }
            for decl in &rule.declarations {
                let key = decl.name.clone();
                let prio = (rule.specificity, rule.order);
                let should_set = match winners.get(&key) {
                    Some(existing) => prio_gt2(&prio, &(existing.0, existing.1)),
                    None => true,
                };
                if should_set {
                    let resolved = resolve_vars(&decl.value, &self.root_vars);
                    winners.insert(key, (rule.specificity, rule.order, resolved));
                }
            }
        }

        // Inline overrides last (raw)
        if let Some(inline_raw) = inline {
            for (name, value) in parse_inline_declarations(inline_raw) {
                winners.insert(name, (u32::MAX, usize::MAX, value));
            }
        }

        // Inherit custom properties from :root for parity/debugging
        for (name, val) in &self.root_vars {
            if !winners.contains_key(name) {
                winners.insert(name.clone(), (0, 0, val.clone()));
            }
        }

        // Convert to a stable, sorted map of name -> value
        let mut out: BTreeMap<String, String> = BTreeMap::new();
        for (name, (_s, _o, v)) in winners {
            out.insert(name, v);
        }
        out
    }
}

fn prio_gt2(a: &(u32, usize), b: &(u32, usize)) -> bool {
    if a.0 != b.0 {
        return a.0 > b.0;
    }
    a.1 > b.1
}

// Minimal CSS block splitter: yields (selectors, body)
fn split_rules(css: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let commented = strip_css_comments(css);
    let source = strip_media_blocks(&commented);
    for raw in source.split('}') {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((sel, body)) = trimmed.split_once('{') {
            let sel = sel.trim();
            let body = body.trim();
            if !sel.is_empty() && !body.is_empty() {
                out.push((sel.to_string(), body.to_string()));
            }
        }
    }
    out
}

fn strip_css_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_comment = false;
    while let Some(ch) = chars.next() {
        if in_comment {
            if ch == '*' {
                if let Some('/') = chars.peek().copied() {
                    chars.next();
                    in_comment = false;
                }
            }
        } else {
            if ch == '/' {
                if let Some('*') = chars.peek().copied() {
                    chars.next();
                    in_comment = true;
                    continue;
                }
            }
            out.push(ch);
        }
    }
    out
}

// Remove all @media blocks completely (we don't evaluate media queries yet).
// This is a conservative single-pass parser that skips balanced braces following
// an @media prelude. Nested blocks are handled by depth tracking.
fn strip_media_blocks(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = String::with_capacity(source.len());
    let mut i = 0usize;
    let n = bytes.len();
    while i < n {
        if bytes[i] == b'@' {
            // Read identifier after '@'
            let mut j = i + 1;
            while j < n && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            let start_ident = j;
            while j < n && bytes[j].is_ascii_alphabetic() {
                j += 1;
            }
            let ident = &source[start_ident..j].to_ascii_lowercase();
            if ident == "media" {
                // Find first '{' starting the block
                while j < n && bytes[j] != b'{' {
                    j += 1;
                }
                if j >= n {
                    break;
                }
                // Skip balanced block
                let mut depth = 1i32;
                j += 1; // skip '{'
                while j < n && depth > 0 {
                    match bytes[j] {
                        b'{' => depth += 1,
                        b'}' => depth -= 1,
                        _ => {}
                    }
                    j += 1;
                }
                // Skip the entire @media block by advancing i
                i = j;
                continue;
            }
        }
        // Normal character: copy and advance
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn parse_declarations(source: &str) -> Vec<CssDeclarationV2> {
    parse_declarations_lightningcss(source).unwrap_or_else(|| parse_declarations_simple(source))
}

#[cfg(feature = "cssv2")]
fn parse_declarations_lightningcss(source: &str) -> Option<Vec<CssDeclarationV2>> {
    use lightningcss::printer::PrinterOptions;
    use lightningcss::stylesheet::{ParserOptions, StyleSheet as LStyleSheet};
    // Wrap the declarations into a single dummy rule so lightningcss parses the declaration list.
    let css = format!("rune-decls{{{}}}", source);
    let sheet = LStyleSheet::parse(&css, ParserOptions::default()).ok()?;
    // Re-serialize to canonical CSS then extract content between braces.
    let code = sheet.to_css(PrinterOptions::default()).ok()?.code;
    let inner = code.split_once('{')?.1;
    let inner = inner.rsplit_once('}')?.0;
    Some(parse_declarations_simple(inner))
}

#[cfg(not(feature = "cssv2"))]
fn parse_declarations_lightningcss(_source: &str) -> Option<Vec<CssDeclarationV2>> {
    None
}

fn parse_declarations_simple(source: &str) -> Vec<CssDeclarationV2> {
    source
        .split(';')
        .filter_map(|decl| {
            let (name, value) = decl.split_once(':')?;
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim();
            if name.is_empty() || value.is_empty() {
                return None;
            }
            Some(CssDeclarationV2 {
                name,
                value: value.to_string(),
            })
        })
        .collect()
}

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

fn format_applied(v: &ComputedStyle2) -> String {
    // Tiny summary of applied fields to aid debugging.
    let mut parts = Vec::new();
    if v.display.is_some() {
        parts.push("display");
    }
    if v.flex_direction.is_some() {
        parts.push("flex-direction");
    }
    if v.justify_content.is_some() {
        parts.push("justify-content");
    }
    if v.align_items.is_some() {
        parts.push("align-items");
    }
    if v.gap.is_some() {
        parts.push("gap");
    }
    if v.color.is_some() {
        parts.push("color");
    }
    if v.background_color.is_some() {
        parts.push("background-color");
    }
    if v.background_gradient.is_some() {
        parts.push("background-gradient");
    }
    if v.background_radial.is_some() {
        parts.push("background-radial");
    }
    if !v.background_layers.is_empty() {
        parts.push("background-layers");
    }
    if v.margin.top != 0.0
        || v.margin.right != 0.0
        || v.margin.bottom != 0.0
        || v.margin.left != 0.0
    {
        parts.push("margin");
    }
    if v.padding.top != 0.0
        || v.padding.right != 0.0
        || v.padding.bottom != 0.0
        || v.padding.left != 0.0
    {
        parts.push("padding");
    }
    if v.width.is_some() {
        parts.push("width");
    }
    if v.height.is_some() {
        parts.push("height");
    }
    if v.object_fit.is_some() {
        parts.push("object-fit");
    }
    parts.join(",")
}

/// Build a stylesheet from <style> blocks and <link rel="stylesheet"> tags.
pub fn build_stylesheet_from_html(document: &Html, options: &HtmlOptions) -> StyleSheet {
    let mut sources: Vec<String> = Vec::new();

    // Inline <style> blocks
    if let Ok(sel) = Selector::parse("style") {
        for node in document.select(&sel) {
            let css = node.text().collect::<String>();
            if !css.trim().is_empty() {
                sources.push(css);
            }
        }
    }

    // Linked stylesheets
    if let Ok(sel) = Selector::parse("link") {
        for node in document.select(&sel) {
            let el = node.value();
            let rel = el.attr("rel").unwrap_or("").to_ascii_lowercase();
            if !rel.contains("stylesheet") {
                continue;
            }
            if let Some(href) = el.attr("href") {
                if let Some(css) = read_stylesheet_link(href, options) {
                    sources.push(css);
                }
            }
        }
    }

    StyleSheet::from_sources(&sources)
}

fn read_stylesheet_link(href: &str, options: &HtmlOptions) -> Option<String> {
    let trimmed = href.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Absolute URL
    if let Ok(url) = Url::parse(trimmed) {
        match url.scheme() {
            "file" => {
                if let Ok(path) = url.to_file_path() {
                    return fs::read_to_string(path).ok();
                }
            }
            // Skip remote http(s) for now; no HTTP client dependency here.
            _ => {
                if diagnostics_enabled("css") {
                    tracing::info!(href = %href, "diagnostics: skipped remote stylesheet link");
                }
                return None;
            }
        }
    }
    // Relative path resolution using base_path from options
    if let Some(base) = &options.base_path {
        let path = base.join(trimmed);
        if path.exists() {
            return fs::read_to_string(path).ok();
        }
    }
    // Last resort: try as-is
    fs::read_to_string(trimmed).ok()
}

fn rightmost_keys(selector_text: &str) -> (Option<String>, Vec<String>, Option<String>) {
    let segment = selector_text
        .rsplit(|c: char| c.is_whitespace() || c == '>' || c == '+' || c == '~')
        .next()
        .unwrap_or(selector_text)
        .trim();
    let mut id: Option<String> = None;
    let mut classes: Vec<String> = Vec::new();
    let mut tag: Option<String> = None;
    let mut chars = segment.chars().peekable();
    let mut buf = String::new();
    while let Some(ch) = chars.peek().copied() {
        match ch {
            '.' => {
                chars.next();
                buf.clear();
                while let Some(n) = chars.peek().copied() {
                    if n.is_alphanumeric() || n == '-' || n == '_' {
                        buf.push(n);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if !buf.is_empty() {
                    classes.push(buf.to_ascii_lowercase());
                }
            }
            '#' => {
                chars.next();
                buf.clear();
                while let Some(n) = chars.peek().copied() {
                    if n.is_alphanumeric() || n == '-' || n == '_' {
                        buf.push(n);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if !buf.is_empty() {
                    id = Some(buf.clone());
                }
            }
            ':' => {
                chars.next();
                while let Some(n) = chars.peek().copied() {
                    if n == '(' {
                        let _ = chars.next();
                        let mut d = 1;
                        while let Some(c2) = chars.next() {
                            if c2 == '(' {
                                d += 1;
                            } else if c2 == ')' {
                                d -= 1;
                                if d == 0 {
                                    break;
                                }
                            }
                        }
                        break;
                    } else if n.is_alphanumeric() || n == '-' || n == '_' {
                        let _ = chars.next();
                    } else {
                        break;
                    }
                }
            }
            '[' => {
                chars.next();
                while let Some(n) = chars.next() {
                    if n == ']' {
                        break;
                    }
                }
            }
            '*' => {
                chars.next();
            }
            _ => {
                if tag.is_none() && (ch.is_alphabetic() || ch == '_') {
                    buf.clear();
                    while let Some(n) = chars.peek().copied() {
                        if n.is_alphanumeric() || n == '-' || n == '_' {
                            buf.push(n);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    if !buf.is_empty() {
                        tag = Some(buf.to_ascii_lowercase());
                    }
                } else {
                    let _ = chars.next();
                }
            }
        }
    }
    (id, classes, tag)
}

fn resolve_vars(raw: &str, vars: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(raw.len());
    let bytes = raw.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if i + 4 <= bytes.len() && &raw[i..i + 4].to_ascii_lowercase() == "var(" {
            // Find matching ')' with nesting count
            let mut j = i + 4; // past 'var('
            let mut depth = 1i32;
            while j < bytes.len() {
                let ch = raw.as_bytes()[j] as char;
                if ch == '(' {
                    depth += 1;
                }
                if ch == ')' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                j += 1;
            }
            if j >= bytes.len() {
                break;
            }
            let inner = raw[i + 4..j].trim();
            // Parse name and optional fallback
            let (name_part, fallback_part) = if let Some(pos) = inner.find(',') {
                (inner[..pos].trim(), Some(inner[pos + 1..].trim()))
            } else {
                (inner, None)
            };
            let name = name_part;
            let value = if let Some(v) = vars.get(name) {
                v.clone()
            } else if let Some(fb) = fallback_part {
                resolve_vars(fb, vars)
            } else {
                String::new()
            };
            out.push_str(&value);
            i = j + 1;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    if i < bytes.len() {
        out.push_str(&raw[i..]);
    }
    out
}
