#![allow(dead_code)]
#![allow(unused_imports)]

use std::{
    collections::HashMap,
    ops::Deref,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use ego_tree::NodeRef;
use scraper::{ElementRef, Html, Node, Selector};
use tracing::info;
use url::Url;

use std::sync::{Mutex, OnceLock};

use crate::{
    data::document::{
        ActionNodeData, DataDocument, DataNode, DataNodeKind, ImageNodeData, TextNodeData,
    },
    logic::{LogicEngine, LogicModuleDescriptor},
    package::RunePackage,
    view::{
        ButtonSpec, EdgeInsets, FlexContainerSpec, FlexLayout, GridAutoFlow, GridContainerSpec,
        GridItemPlacement, GridLayout, GridTrackSize, ImageContentFit, ImageSpec, InputBoxSpec,
        LayoutAlign, LayoutDirection, LayoutJustify, LinkSpec, ScrollBehavior, SpacerSpec,
        SurfaceStyle, TextAreaSpec, TextSpec, TextStyle, ViewBackground, ViewDocument, ViewNode,
        ViewNodeKind,
    },
};

#[derive(Debug, Clone)]
pub struct HtmlOptions {
    pub document_id: Option<String>,
    pub view_id: Option<String>,
    pub base_path: Option<PathBuf>,
    pub base_url: Option<Url>,
}

impl Default for HtmlOptions {
    fn default() -> Self {
        Self {
            document_id: None,
            view_id: None,
            base_path: None,
            base_url: None,
        }
    }
}

pub fn package_from_html(html: &str, options: HtmlOptions) -> Result<RunePackage> {
    let document = Html::parse_document(html);
    let translator = HtmlTranslator::new(document, options)?;
    translator.into_package()
}

pub fn package_from_file(path: &Path) -> Result<RunePackage> {
    let html = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read HTML file '{}'", path.display()))?;
    let mut options = HtmlOptions::default();
    options.base_path = path.parent().map(|parent| parent.to_path_buf());
    if let Ok(url) = Url::from_file_path(path) {
        options.base_url = Some(url);
    }
    options.document_id = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| sanitize_identifier(stem));
    package_from_html(&html, options)
}

struct HtmlTranslator {
    html: Html,
    options: HtmlOptions,
}

impl HtmlTranslator {
    fn new(html: Html, options: HtmlOptions) -> Result<Self> {
        Ok(Self { html, options })
    }

    fn into_package(self) -> Result<RunePackage> {
        let mut builder = TranslatorState::new(&self.options);
        let root_vars = collect_root_custom_properties(&self.html);
        builder.set_root_vars(root_vars);
        let title = extract_title(&self.html);
        let document_id = self
            .options
            .document_id
            .clone()
            .or_else(|| title.clone())
            .unwrap_or_else(|| "document".to_string());
        let document_id = sanitize_identifier(&document_id);
        let view_id = self
            .options
            .view_id
            .clone()
            .unwrap_or_else(|| format!("{}-view", document_id));

        // Build v2 stylesheet (style + link rel=stylesheet) and make it the source of truth.
        let sheet = crate::css::build_stylesheet_from_html(&self.html, &self.options);
        builder.set_stylesheet_v2(sheet);
        // Diagnostics: parse <style> blocks via lightningcss for visibility.
        if diagnostics_enabled("css") {
            if let Ok(selector) = Selector::parse("style") {
                let mut parsed_blocks = 0usize;
                for node in self.html.select(&selector) {
                    let css = node.text().collect::<String>();
                    if lightningcss::stylesheet::StyleSheet::parse(
                        &css,
                        lightningcss::stylesheet::ParserOptions::default(),
                    )
                    .is_ok()
                    {
                        parsed_blocks += 1;
                    }
                }
                info!(
                    blocks = parsed_blocks,
                    "diagnostics: cssv2 parsed <style> blocks"
                );
            }
        }

        let body = find_body(&self.html);
        if let Some(body) = body {
            builder.convert_root(body)?;
        } else {
            let root = self
                .html
                .tree
                .root()
                .children()
                .find_map(|child| {
                    if let Node::Element(_) = child.value() {
                        Some(child)
                    } else {
                        None
                    }
                })
                .context("HTML document missing body element")?;
            builder.convert_root(root)?;
        }

        let view_document = ViewDocument {
            view_id,
            root: builder.root_view_id.clone(),
            nodes: builder.view_nodes,
        };

        let data_document = DataDocument {
            document_id: document_id.clone(),
            nodes: builder.data_nodes,
            bindings: Vec::new(),
            channels: Vec::new(),
        };

        let base_path = self
            .options
            .base_path
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let mut package = RunePackage::from_documents(
            document_id.clone(),
            data_document,
            view_document,
            base_path,
        )
        .context("failed to assemble Rune package from HTML")?;
        // Propagate the HTML <title> into the manifest as a human-friendly page title.
        if let Some(t) = title.clone() {
            package.manifest.entrypoint.page_title = Some(t);
        }
        // Phase 5: extract <script> tags into JS logic modules (Boa/live HTML mode).
        let scripts = collect_script_entries(&self.html);
        if !scripts.is_empty() {
            let mut first_name: Option<String> = None;
            // Ensure we have a place to write inline scripts if needed.
            let logic_dir = package.base_path().join("logic");
            let _ = std::fs::create_dir_all(&logic_dir);

            for (idx, script) in scripts.iter().enumerate() {
                // Determine module path and write inline content if present.
                let module_path = match (&script.src, &script.inline_source) {
                    (Some(src), _) => {
                        // Resolve links against base options; allow absolute paths/URLs.
                        let resolver = ResourceResolver::new(&self.options);
                        let resolved = resolver.resolve_link(src);
                        // Skip remote HTTP(S) scripts for now (no fetch in translator).
                        if resolved.starts_with("http://") || resolved.starts_with("https://") {
                            if diagnostics_enabled("html") || diagnostics_enabled("js") {
                                info!(src = %src, resolved = %resolved, "diagnostics: skipped remote script");
                            }
                            continue;
                        }
                        resolved
                    }
                    (None, Some(code)) => {
                        // Write inline script to a stable location in the package base path.
                        // Name: <document_id>-inline-<n>.js
                        let file_name = format!("{}-inline-{:02}.js", document_id, idx + 1);
                        let file_path = logic_dir.join(&file_name);
                        // Ignore write errors silently? No: bubble up to aid debugging.
                        std::fs::write(&file_path, code.as_bytes()).with_context(|| {
                            format!("failed to write inline script to {}", file_path.display())
                        })?;
                        // Use path relative to base when possible; otherwise absolute.
                        match file_path.strip_prefix(package.base_path()) {
                            Ok(rel) => rel.to_string_lossy().to_string(),
                            Err(_) => file_path.to_string_lossy().to_string(),
                        }
                    }
                    _ => {
                        // No usable content; skip.
                        continue;
                    }
                };

                // Name module deterministically.
                let module_name = if scripts.len() == 1 {
                    format!("{}-logic", document_id)
                } else {
                    format!("{}-logic-{:02}", document_id, idx + 1)
                };
                if first_name.is_none() {
                    first_name = Some(module_name.clone());
                }

                package.logic_modules.insert(
                    module_name,
                    LogicModuleDescriptor {
                        module: module_path,
                        capabilities: Vec::new(),
                        engine: LogicEngine::Js,
                    },
                );
            }

            if let Some(entry) = first_name {
                package.manifest.entrypoint.logic = Some(entry);
            }
        }
        Ok(package)
    }
}

fn parse_diagnostics_env() -> &'static std::collections::HashSet<String> {
    static SET: OnceLock<std::collections::HashSet<String>> = OnceLock::new();
    SET.get_or_init(|| {
        let raw = std::env::var("RUNE_DIAGNOSTICS").unwrap_or_default();
        raw.split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    })
}

fn diagnostics_enabled(category: &str) -> bool {
    let set = parse_diagnostics_env();
    set.contains("all") || set.contains(&category.to_ascii_lowercase())
}

fn extract_title(document: &Html) -> Option<String> {
    let selector = Selector::parse("title").ok()?;
    document
        .select(&selector)
        .next()
        .map(|node| normalize_whitespace(&collect_text(&node)))
}

fn find_body(document: &Html) -> Option<NodeRef<'_, Node>> {
    let selector = Selector::parse("body").ok()?;
    document
        .select(&selector)
        .next()
        .map(|body| (*body).clone())
}

#[derive(Debug, Clone)]
struct ScriptEntry {
    src: Option<String>,
    inline_source: Option<String>,
}

fn collect_script_entries(document: &Html) -> Vec<ScriptEntry> {
    let mut entries = Vec::new();
    let Some(selector) = Selector::parse("script").ok() else {
        return entries;
    };
    for node in document.select(&selector) {
        let el = node.value();
        let ty = el.attr("type").map(|v| v.trim().to_ascii_lowercase());
        // Skip JSON script types like application/json, application/ld+json
        if matches!(ty.as_deref(), Some(t) if t.contains("json")) {
            continue;
        }
        if let Some(src) = el.attr("src").map(|v| v.trim().to_string()) {
            if !src.is_empty() {
                entries.push(ScriptEntry {
                    src: Some(src),
                    inline_source: None,
                });
                continue;
            }
        }
        // Inline content
        let raw = collect_raw_text(&node);
        if !raw.trim().is_empty() {
            entries.push(ScriptEntry {
                src: None,
                inline_source: Some(raw),
            });
        }
    }
    entries
}

struct TranslatorState {
    data_nodes: Vec<DataNode>,
    view_nodes: Vec<ViewNode>,
    root_view_id: String,
    id_generator: IdGenerator,
    resolver: ResourceResolver,
    root_vars: HashMap<String, String>,
    stylesheet_v2: Option<crate::css::StyleSheet>,
}

impl TranslatorState {
    fn new(options: &HtmlOptions) -> Self {
        Self {
            data_nodes: Vec::new(),
            view_nodes: Vec::new(),
            root_view_id: String::new(),
            id_generator: IdGenerator::new(),
            resolver: ResourceResolver::new(options),
            root_vars: HashMap::new(),
            stylesheet_v2: None,
        }
    }

    fn set_root_vars(&mut self, vars: HashMap<String, String>) {
        self.root_vars = vars;
    }

    fn set_stylesheet_v2(&mut self, sheet: crate::css::StyleSheet) {
        self.stylesheet_v2 = Some(sheet);
    }

    fn convert_root(&mut self, root: NodeRef<Node>) -> Result<()> {
        let element_ref =
            ElementRef::wrap(root.clone()).ok_or_else(|| anyhow!("root node is not an element"))?;
        let mut style = ComputedStyle::default();
        let mut taffy_hints = None;
        if let Some(sheet) = &self.stylesheet_v2 {
            let inline = element_ref.value().attr("style");
            let mut v2 = sheet.compute_for(&element_ref, inline);
            // Apply UA default display when unspecified to mirror Safari/WebKit behavior
            if v2.display.is_none() {
                v2.display = Some(crate::css::default_display_for_tag(
                    element_ref.value().name(),
                ));
            }
            // Bridge for visual properties still consumed elsewhere in this module
            crate::css::apply_cssv2_inline_to_style(&mut style, &v2);
            // New: derive Taffy hints directly from v2 style for layout
            taffy_hints = Some(crate::css::convert_style_to_taffy(&v2));
        }
        resolve_vars_in_style(&mut style, &self.root_vars);
        let root_id = self.id_generator.next_view_id();
        let mut container = empty_container_spec();
        if let Some(h) = &taffy_hints {
            container.layout.direction = h.direction;
            container.layout.wrap = h.wrap;
            container.layout.justify = h.justify;
            container.layout.align = h.align;
            container.layout.gap = h.gap;
            container.margin = h.margin.clone();
            container.margin_left_auto = h.margin_left_auto;
            container.margin_right_auto = h.margin_right_auto;
            container.padding = h.padding.clone();
            container.width = h.width;
            container.height = h.height;
            container.min_width = h.min_width;
            container.min_height = h.min_height;
            container.max_width = h.max_width;
            container.max_height = h.max_height;
        } else {
            container.layout.direction = LayoutDirection::Column;
            container.layout.gap = style.gap.unwrap_or(12.0);
            container.margin = to_edge_insets(&style.margin);
            container.padding = to_edge_insets(&style.padding);
            container.min_width = style.min_width;
            container.min_height = style.min_height;
            container.max_width = style.max_width;
            container.max_height = style.max_height;
        }
        // Prefer layered backgrounds when present
        if !style.backgrounds.is_empty() {
            container.backgrounds = style.backgrounds.clone();
        }
        container.background = if let Some((start, end, cx, cy, rx, ry)) = &style.background_radial
        {
            Some(ViewBackground::RadialGradient {
                cx: *cx,
                cy: *cy,
                rx: *rx,
                ry: *ry,
                stops: vec![(start.clone(), 0.0), (end.clone(), 1.0)],
            })
        } else if let Some((start, end, angle)) = &style.background_gradient {
            Some(ViewBackground::LinearGradient {
                angle: *angle,
                stops: vec![(start.clone(), 0.0), (end.clone(), 1.0)],
            })
        } else {
            style
                .background_color
                .clone()
                .map(|color| ViewBackground::Solid { color })
        };
        container.scroll = ScrollBehavior {
            horizontal: false,
            vertical: true,
        };

        let mut children = Vec::new();
        for child in root.children() {
            if let Some(id) = self.convert_node(child, Some(&style))? {
                children.push(id);
            }
        }
        container.children = children;
        let root_view = ViewNode {
            id: root_id.clone(),
            node_id: None,
            widget_id: None,
            kind: ViewNodeKind::FlexContainer(container),
        };
        self.root_view_id = root_id.clone();
        self.view_nodes.push(root_view);
        Ok(())
    }

    fn convert_node(
        &mut self,
        node: NodeRef<Node>,
        parent_style: Option<&ComputedStyle>,
    ) -> Result<Option<String>> {
        if let Some(element_ref) = ElementRef::wrap(node.clone()) {
            let tag = element_ref.value().name().to_ascii_lowercase();
            if matches!(tag.as_str(), "script" | "style" | "meta" | "link" | "head") {
                return Ok(None);
            }
            let mut style = ComputedStyle::default();
            let mut taffy_hints = None;
            let mut v2_style: Option<crate::css::ComputedStyle2> = None;
            if let Some(sheet) = &self.stylesheet_v2 {
                let inline = element_ref.value().attr("style");
                let mut v2 = sheet.compute_for(&element_ref, inline);
                if v2.display.is_none() {
                    v2.display = Some(crate::css::default_display_for_tag(
                        element_ref.value().name(),
                    ));
                }
                crate::css::apply_cssv2_inline_to_style(&mut style, &v2);
                taffy_hints = Some(crate::css::convert_style_to_taffy(&v2));
                v2_style = Some(v2);
            }
            resolve_vars_in_style(&mut style, &self.root_vars);
            // no debug prints
            // Inherit text properties from the closest ancestor when unspecified.
            // This mirrors standard CSS inheritance for color/font-family/text-align.
            if style.color.is_none() {
                if let Some(p) = parent_style {
                    if let Some(pc) = &p.color {
                        style.color = Some(pc.clone());
                    }
                }
            }
            if style.font_family.is_none() {
                if let Some(p) = parent_style {
                    if let Some(pf) = &p.font_family {
                        style.font_family = Some(pf.clone());
                    }
                }
            }
            if style.text_align.is_none() {
                if let Some(p) = parent_style {
                    if let Some(pa) = p.text_align {
                        style.text_align = Some(pa);
                    }
                }
            }
            if matches!(style.display, Display::None) {
                return Ok(None);
            }
            let classification = classify_element(&element_ref, &style);
            return match classification {
                ElementKind::Container => {
                    let is_grid = matches!(
                        v2_style.as_ref().and_then(|v| v.display),
                        Some(crate::css::Display2::Grid)
                    );
                    if is_grid {
                        // Build Grid container
                        let mut spec = GridContainerSpec {
                            layout: GridLayout::default(),
                            padding: to_edge_insets(&style.padding),
                            margin: to_edge_insets(&style.margin),
                            margin_left_auto: false,
                            margin_right_auto: false,
                            background: None,
                            backgrounds: Vec::new(),
                            corner_radius: style.corner_radius,
                            width: style.width,
                            height: style.height,
                            min_width: style.min_width,
                            min_height: style.min_height,
                            max_width: style.max_width,
                            max_height: style.max_height,
                            border_width: style.border_width,
                            border_color: style.border_color.clone(),
                            border_top_width: None,
                            border_right_width: None,
                            border_bottom_width: None,
                            border_left_width: None,
                            border_top_color: None,
                            border_right_color: None,
                            border_bottom_color: None,
                            border_left_color: None,
                            box_shadow: style.box_shadow.clone().map(|(x, y, blur, color)| {
                                crate::view::BoxShadowSpec {
                                    offset_x: x,
                                    offset_y: y,
                                    blur,
                                    color,
                                }
                            }),
                            children: Vec::new(),
                            placements: Vec::new(),
                        };
                        if let Some(v2) = &v2_style {
                            if !style.backgrounds.is_empty() {
                                spec.backgrounds = style.backgrounds.clone();
                            }
                            spec.margin_left_auto = v2.margin_left_auto;
                            spec.margin_right_auto = v2.margin_right_auto;
                            spec.background = if let Some((start, end, cx, cy, rx, ry)) =
                                &style.background_radial
                            {
                                Some(ViewBackground::RadialGradient {
                                    cx: *cx,
                                    cy: *cy,
                                    rx: *rx,
                                    ry: *ry,
                                    stops: vec![(start.clone(), 0.0), (end.clone(), 1.0)],
                                })
                            } else if let Some((start, end, angle)) = &style.background_gradient {
                                Some(ViewBackground::LinearGradient {
                                    angle: *angle,
                                    stops: vec![(start.clone(), 0.0), (end.clone(), 1.0)],
                                })
                            } else {
                                style
                                    .background_color
                                    .clone()
                                    .map(|color| ViewBackground::Solid { color })
                            };
                            if let Some(t) = &v2.grid_template_columns {
                                spec.layout.columns = t
                                    .tracks
                                    .iter()
                                    .map(|trk| match trk {
                                        crate::css::GridTrackSize2::Fr(v) => {
                                            GridTrackSize::Fr { value: *v }
                                        }
                                        crate::css::GridTrackSize2::Px(v) => {
                                            GridTrackSize::Px { value: *v }
                                        }
                                        crate::css::GridTrackSize2::Auto => GridTrackSize::Auto,
                                    })
                                    .collect();
                            }
                            if let Some(t) = &v2.grid_template_rows {
                                spec.layout.rows = t
                                    .tracks
                                    .iter()
                                    .map(|trk| match trk {
                                        crate::css::GridTrackSize2::Fr(v) => {
                                            GridTrackSize::Fr { value: *v }
                                        }
                                        crate::css::GridTrackSize2::Px(v) => {
                                            GridTrackSize::Px { value: *v }
                                        }
                                        crate::css::GridTrackSize2::Auto => GridTrackSize::Auto,
                                    })
                                    .collect();
                            }
                            spec.layout.column_gap = v2.column_gap.or(v2.gap).unwrap_or(0.0);
                            spec.layout.row_gap = v2.row_gap.or(v2.gap).unwrap_or(0.0);
                            // Align items within their grid areas if specified
                            spec.layout.align_items = v2.align_items;
                            spec.layout.auto_flow = match v2
                                .grid_auto_flow
                                .unwrap_or(crate::css::GridAutoFlow2::Row)
                            {
                                crate::css::GridAutoFlow2::Row => GridAutoFlow::Row,
                                crate::css::GridAutoFlow2::Column => GridAutoFlow::Column,
                                crate::css::GridAutoFlow2::RowDense => GridAutoFlow::RowDense,
                                crate::css::GridAutoFlow2::ColumnDense => GridAutoFlow::ColumnDense,
                            };
                        }
                        let mut children: Vec<String> = Vec::new();
                        let mut placements: Vec<GridItemPlacement> = Vec::new();
                        for child in node.children() {
                            let mut placement = GridItemPlacement {
                                col_span: 1,
                                row_span: 1,
                                col_start: None,
                                row_start: None,
                            };
                            if let (Some(sheet), Some(el)) =
                                (&self.stylesheet_v2, ElementRef::wrap(child.clone()))
                            {
                                let inline = el.value().attr("style");
                                let mut c2 = sheet.compute_for(&el, inline);
                                if c2.display.is_none() {
                                    c2.display = Some(crate::css::default_display_for_tag(
                                        el.value().name(),
                                    ));
                                }
                                if let Some(span) = c2.grid_column_span {
                                    placement.col_span = span.max(1);
                                }
                                if let Some(span) = c2.grid_row_span {
                                    placement.row_span = span.max(1);
                                }
                            }
                            if let Some(id) = self.convert_node(child, Some(&style))? {
                                children.push(id);
                                placements.push(placement);
                            }
                        }
                        if children.is_empty() {
                            return Ok(None);
                        }
                        spec.children = children;
                        spec.placements = placements;
                        let view_id = self.id_generator.next_view_id();
                        let view_node = ViewNode {
                            id: view_id.clone(),
                            node_id: None,
                            widget_id: None,
                            kind: ViewNodeKind::GridContainer(spec),
                        };
                        self.view_nodes.push(view_node);
                        Ok(Some(view_id))
                    } else {
                        // Flex container path (unchanged)
                        let mut container = empty_container_spec();
                        if let Some(h) = &taffy_hints {
                            container.layout.direction = h.direction;
                            container.layout.wrap = h.wrap;
                            container.layout.justify = h.justify;
                            container.layout.align = h.align;
                            container.layout.gap = h.gap;
                            container.padding = h.padding.clone();
                            container.margin = h.margin.clone();
                            container.margin_left_auto = h.margin_left_auto;
                            container.margin_right_auto = h.margin_right_auto;
                            container.width = h.width;
                            container.height = h.height;
                            container.min_width = h.min_width;
                            container.min_height = h.min_height;
                            container.max_width = h.max_width;
                            container.max_height = h.max_height;
                        } else {
                            container.layout = build_flex_layout(&style);
                            container.padding = to_edge_insets(&style.padding);
                            container.margin = to_edge_insets(&style.margin);
                            container.width = style.width;
                            container.height = style.height;
                            container.min_width = style.min_width;
                            container.min_height = style.min_height;
                            container.max_width = style.max_width;
                            container.max_height = style.max_height;
                        }
                        if !style.backgrounds.is_empty() {
                            container.backgrounds = style.backgrounds.clone();
                        }
                        container.background =
                            if let Some((start, end, cx, cy, rx, ry)) = &style.background_radial {
                                Some(ViewBackground::RadialGradient {
                                    cx: *cx,
                                    cy: *cy,
                                    rx: *rx,
                                    ry: *ry,
                                    stops: vec![(start.clone(), 0.0), (end.clone(), 1.0)],
                                })
                            } else if let Some((start, end, angle)) = &style.background_gradient {
                                Some(ViewBackground::LinearGradient {
                                    angle: *angle,
                                    stops: vec![(start.clone(), 0.0), (end.clone(), 1.0)],
                                })
                            } else {
                                style
                                    .background_color
                                    .clone()
                                    .map(|color| ViewBackground::Solid { color })
                            };
                        // Propagate rounded corners to the container visuals
                        container.corner_radius = style.corner_radius;
                        container.border_width = style.border_width;
                        container.border_color = style.border_color.clone();
                        if let Some(v2) = &v2_style {
                            container.border_top_width = v2.border_top_width;
                            container.border_right_width = v2.border_right_width;
                            container.border_bottom_width = v2.border_bottom_width;
                            container.border_left_width = v2.border_left_width;
                            container.border_top_color = v2.border_top_color.clone();
                            container.border_right_color = v2.border_right_color.clone();
                            container.border_bottom_color = v2.border_bottom_color.clone();
                            container.border_left_color = v2.border_left_color.clone();
                        }
                        if let Some((x, y, blur, color)) = style.box_shadow.clone() {
                            container.box_shadow = Some(crate::view::BoxShadowSpec {
                                offset_x: x,
                                offset_y: y,
                                blur,
                                color,
                            });
                        }
                        let mut children = Vec::new();
                        for child in node.children() {
                            if let Some(id) = self.convert_node(child, Some(&style))? {
                                children.push(id);
                            }
                        }
                        if children.is_empty() {
                            return Ok(None);
                        }
                        container.children = children;
                        let view_id = self.id_generator.next_view_id();
                        let view_node = ViewNode {
                            id: view_id.clone(),
                            node_id: None,
                            widget_id: None,
                            kind: ViewNodeKind::FlexContainer(container),
                        };
                        self.view_nodes.push(view_node);
                        Ok(Some(view_id))
                    }
                }
                ElementKind::Text(role) => {
                    // Preserve formatting for <pre> blocks: do not collapse whitespace/newlines.
                    // For all other text elements keep normalized whitespace.
                    let text_content = if tag.eq_ignore_ascii_case("pre") {
                        collect_raw_text(&node)
                    } else {
                        normalize_whitespace(&collect_text(&node))
                    };
                    if text_content.is_empty() {
                        return Ok(None);
                    }
                    let data_id = self.id_generator.next_data_id();
                    let widget_id = self.id_generator.next_widget_id();
                    let data_node = DataNode {
                        node_id: data_id.clone(),
                        widget_id: Some(widget_id.clone()),
                        kind: DataNodeKind::Text(TextNodeData {
                            text: text_content.clone(),
                            semantic_role: role.map(|r| match r {
                                SemanticRole::Heading(level) => {
                                    crate::data::document::TextSemanticRole::Heading { level }
                                }
                                SemanticRole::Paragraph => {
                                    crate::data::document::TextSemanticRole::Paragraph
                                }
                                SemanticRole::Label => {
                                    crate::data::document::TextSemanticRole::Label
                                }
                            }),
                        }),
                    };
                    self.data_nodes.push(data_node);
                    let mut text_style = to_text_style(&style);
                    let heading_level = role.as_ref().and_then(|semantic| match semantic {
                        SemanticRole::Heading(level) => Some(*level),
                        _ => None,
                    });
                    if let Some(level) = heading_level {
                        apply_heading_text_defaults(&mut text_style, level);
                    }
                    if text_style.color.is_none() {
                        if let Some(parent) = parent_style {
                            text_style.color = parent.color.clone();
                        }
                    }
                    // Inherit font-size and line-height when unspecified to better
                    // mirror CSS inheritance (needed for code/pre blocks styled on parent).
                    if text_style.font_size.is_none() {
                        if let Some(parent) = parent_style {
                            if let Some(sz) = parent.font_size {
                                text_style.font_size = Some(sz);
                            }
                        }
                    }
                    if text_style.line_height.is_none() {
                        if let Some(parent) = parent_style {
                            if let Some(lh) = parent.line_height {
                                text_style.line_height = Some(lh);
                            }
                        }
                    }
                    if text_style.text_align.is_none() {
                        if let Some(parent) = parent_style {
                            if let Some(a) = parent.text_align {
                                text_style.text_align = Some(a);
                            }
                        }
                    }
                    let view_id = self.id_generator.next_view_id();
                    let view_node = ViewNode {
                        id: view_id.clone(),
                        node_id: Some(data_id),
                        widget_id: Some(widget_id),
                        kind: ViewNodeKind::Text(TextSpec { style: text_style }),
                    };
                    self.view_nodes.push(view_node);
                    Ok(Some(view_id))
                }
                ElementKind::Image => {
                    let src = element_ref
                        .value()
                        .attr("src")
                        .map(|value| value.to_string())
                        .unwrap_or_default();
                    if src.is_empty() {
                        return Ok(None);
                    }
                    let resolved_source = self.resolver.resolve_asset(&src);
                    let data_id = self.id_generator.next_data_id();
                    let widget_id = self.id_generator.next_widget_id();
                    let mut image_data = ImageNodeData {
                        source: resolved_source,
                        width: style.width,
                        height: style.height,
                        description: element_ref
                            .value()
                            .attr("alt")
                            .map(|value| value.to_string()),
                    };
                    if image_data.width.is_none() {
                        if let Some(width) =
                            element_ref.value().attr("width").and_then(parse_number)
                        {
                            image_data.width = Some(width);
                        }
                    }
                    if image_data.height.is_none() {
                        if let Some(height) =
                            element_ref.value().attr("height").and_then(parse_number)
                        {
                            image_data.height = Some(height);
                        }
                    }
                    let data_node = DataNode {
                        node_id: data_id.clone(),
                        widget_id: Some(widget_id.clone()),
                        kind: DataNodeKind::Image(image_data),
                    };
                    self.data_nodes.push(data_node);
                    let image_spec = ImageSpec {
                        width: style.width,
                        height: style.height,
                        content_fit: style.content_fit,
                    };
                    let view_id = self.id_generator.next_view_id();
                    let view_node = ViewNode {
                        id: view_id.clone(),
                        node_id: Some(data_id),
                        widget_id: Some(widget_id),
                        kind: ViewNodeKind::Image(image_spec),
                    };
                    self.view_nodes.push(view_node);
                    Ok(Some(view_id))
                }
                ElementKind::Link => {
                    let href = element_ref
                        .value()
                        .attr("href")
                        .map(|value| value.to_string())
                        .unwrap_or_default();
                    let resolved_href = self.resolver.resolve_link(&href);
                    let label = normalize_whitespace(&collect_text(&node));
                    if label.is_empty() {
                        return Ok(None);
                    }
                    let data_id = self.id_generator.next_data_id();
                    let widget_id = self.id_generator.next_widget_id();
                    let data_node = DataNode {
                        node_id: data_id.clone(),
                        widget_id: Some(widget_id.clone()),
                        kind: DataNodeKind::Action(ActionNodeData {
                            label: label.clone(),
                            action: None,
                            href: if resolved_href.is_empty() {
                                None
                            } else {
                                Some(resolved_href)
                            },
                            intent: None,
                        }),
                    };
                    self.data_nodes.push(data_node);
                    let surface = to_surface_style(&style);
                    // Do not force a default background; honor CSS so links are text-only by default.
                    let mut label_style = to_text_style(&style);
                    if label_style.color.is_none() {
                        if let Some(parent) = parent_style {
                            if let Some(parent_color) = &parent.color {
                                label_style.color = Some(parent_color.clone());
                            }
                        }
                    }
                    if label_style.text_align.is_none() {
                        if let Some(parent) = parent_style {
                            if let Some(a) = parent.text_align {
                                label_style.text_align = Some(a);
                            }
                        }
                    }
                    // If still unspecified, use a conventional link color for visibility.
                    if label_style.color.is_none() {
                        label_style.color = Some("#0177ff".to_string());
                    }
                    let view_id = self.id_generator.next_view_id();
                    let view_node = ViewNode {
                        id: view_id.clone(),
                        node_id: Some(data_id),
                        widget_id: Some(widget_id),
                        kind: ViewNodeKind::Link(LinkSpec {
                            style: surface,
                            label_style,
                        }),
                    };
                    self.view_nodes.push(view_node);
                    Ok(Some(view_id))
                }
                ElementKind::InputBox => {
                    let placeholder = element_ref
                        .value()
                        .attr("placeholder")
                        .map(|value| value.to_string());
                    // Preserve the raw input type string if provided
                    let input_type = element_ref
                        .value()
                        .attr("type")
                        .map(|v| v.to_ascii_lowercase());
                    let control_id = element_ref
                        .value()
                        .attr("id")
                        .map(|value| value.to_string());
                    let width = style
                        .width
                        .or_else(|| element_ref.value().attr("width").and_then(parse_number));
                    let view_id = self.id_generator.next_view_id();
                    let widget_id = self.id_generator.next_widget_id();
                    let view_node = ViewNode {
                        id: view_id.clone(),
                        node_id: None,
                        widget_id: Some(widget_id),
                        kind: ViewNodeKind::InputBox(InputBoxSpec {
                            width,
                            placeholder,
                            input_type,
                            control_id,
                            form_id: None,
                            default_value: None,
                            style: SurfaceStyle::default(),
                        }),
                    };
                    self.view_nodes.push(view_node);
                    Ok(Some(view_id))
                }
                ElementKind::TextArea => {
                    let placeholder = element_ref
                        .value()
                        .attr("placeholder")
                        .map(|value| value.to_string());
                    let control_id = element_ref
                        .value()
                        .attr("id")
                        .map(|value| value.to_string());
                    let width = style
                        .width
                        .or_else(|| element_ref.value().attr("width").and_then(parse_number));
                    let view_id = self.id_generator.next_view_id();
                    let widget_id = self.id_generator.next_widget_id();
                    let view_node = ViewNode {
                        id: view_id.clone(),
                        node_id: None,
                        widget_id: Some(widget_id),
                        kind: ViewNodeKind::TextArea(TextAreaSpec {
                            width,
                            placeholder,
                            control_id,
                            form_id: None,
                            default_value: None,
                        }),
                    };
                    self.view_nodes.push(view_node);
                    Ok(Some(view_id))
                }
                ElementKind::Button => {
                    let label = element_ref
                        .value()
                        .attr("value")
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| normalize_whitespace(&collect_text(&node)));
                    if label.is_empty() {
                        return Ok(None);
                    }
                    let data_id = self.id_generator.next_data_id();
                    let widget_id = self.id_generator.next_widget_id();
                    let data_node = DataNode {
                        node_id: data_id.clone(),
                        widget_id: Some(widget_id.clone()),
                        kind: DataNodeKind::Action(ActionNodeData {
                            label: label.clone(),
                            action: Some(tag.clone()),
                            href: None,
                            intent: None,
                        }),
                    };
                    self.data_nodes.push(data_node);
                    let mut surface = to_surface_style(&style);
                    if surface.background.is_none() {
                        surface.background = Some(ViewBackground::Solid {
                            color: "#0f172a".to_string(),
                        });
                    }
                    let mut label_style = to_text_style(&style);
                    if label_style.color.is_none() {
                        if let Some(parent) = parent_style {
                            if let Some(parent_color) = &parent.color {
                                label_style.color = Some(parent_color.clone());
                            }
                        }
                    }
                    if label_style.color.is_none() {
                        label_style.color = Some("#ffffff".to_string());
                    }
                    let view_id = self.id_generator.next_view_id();
                    let view_node = ViewNode {
                        id: view_id.clone(),
                        node_id: Some(data_id),
                        widget_id: Some(widget_id),
                        kind: ViewNodeKind::Button(ButtonSpec {
                            style: surface,
                            label_style,
                            on_click_intent: None,
                            button_type: None,
                            form_id: None,
                            form_action: None,
                            form_method: None,
                            form_encoding: None,
                        }),
                    };
                    self.view_nodes.push(view_node);
                    Ok(Some(view_id))
                }
                ElementKind::Spacer => {
                    let size = style
                        .height
                        .or_else(|| {
                            let between = style.margin.top + style.margin.bottom;
                            if between > 0.0 { Some(between) } else { None }
                        })
                        .unwrap_or(8.0);
                    if size <= 0.0 {
                        return Ok(None);
                    }
                    let view_id = self.id_generator.next_view_id();
                    let view_node = ViewNode {
                        id: view_id.clone(),
                        node_id: None,
                        widget_id: None,
                        kind: ViewNodeKind::Spacer(SpacerSpec { size }),
                    };
                    self.view_nodes.push(view_node);
                    Ok(Some(view_id))
                }
                ElementKind::Hidden => Ok(None),
            };
        }

        match node.value() {
            Node::Text(text) => {
                let content = normalize_whitespace(text.deref());
                if content.is_empty() {
                    return Ok(None);
                }
                let data_id = self.id_generator.next_data_id();
                let widget_id = self.id_generator.next_widget_id();
                let data_node = DataNode {
                    node_id: data_id.clone(),
                    widget_id: Some(widget_id.clone()),
                    kind: DataNodeKind::Text(TextNodeData {
                        text: content.clone(),
                        semantic_role: None,
                    }),
                };
                self.data_nodes.push(data_node);

                // Build a text style from the parent for typographic
                // properties (color, font, alignment), but deliberately
                // drop surface/box properties that belong to the parent
                // element (backgrounds, padding, corner radius, etc.).
                // Otherwise we end up painting a second inner background
                // for runs of text (e.g., inside `.eyebrow`), which shows
                // up as the extra gray pill seen in the demo.
                let mut text_style = parent_style
                    .map(to_text_style)
                    .unwrap_or_else(TextStyle::default);
                if text_style.color.is_none() {
                    if let Some(parent) = parent_style {
                        text_style.color = parent.color.clone();
                    }
                }
                // Backgrounds/padding on a container should be modeled by a
                // Box node. For raw text nodes we suppress these to avoid
                // double-painting (container + text).
                text_style.background = None;
                text_style.padding = Default::default();
                text_style.corner_radius = None;

                let view_id = self.id_generator.next_view_id();
                let view_node = ViewNode {
                    id: view_id.clone(),
                    node_id: Some(data_id),
                    widget_id: Some(widget_id),
                    kind: ViewNodeKind::Text(TextSpec { style: text_style }),
                };
                self.view_nodes.push(view_node);
                Ok(Some(view_id))
            }
            _ => Ok(None),
        }
    }
}

#[derive(Default, Clone)]
pub(crate) struct ComputedStyle {
    display: Display,
    flex_direction: Option<LayoutDirection>,
    justify_content: Option<LayoutJustify>,
    align_items: Option<LayoutAlign>,
    text_align: Option<crate::view::TextAlign>,
    wrap: Option<bool>,
    gap: Option<f64>,
    background_color: Option<String>,
    background_gradient: Option<(String, String, f64)>,
    background_radial: Option<(String, String, f64, f64, f64, f64)>,
    backgrounds: Vec<ViewBackground>,
    color: Option<String>,
    font_family: Option<String>,
    font_size: Option<f64>,
    line_height: Option<f64>,
    font_weight: Option<f64>,
    margin: EdgeValues,
    padding: EdgeValues,
    width: Option<f64>,
    height: Option<f64>,
    min_width: Option<f64>,
    min_height: Option<f64>,
    max_width: Option<f64>,
    max_height: Option<f64>,
    content_fit: Option<ImageContentFit>,
    corner_radius: Option<f64>,
    border_width: Option<f64>,
    border_color: Option<String>,
    box_shadow: Option<(f64, f64, f64, String)>,
}

#[derive(Default, Clone, Copy)]
struct EdgeValues {
    top: f64,
    right: f64,
    bottom: f64,
    left: f64,
}

fn to_edge_insets(values: &EdgeValues) -> EdgeInsets {
    EdgeInsets {
        top: values.top,
        right: values.right,
        bottom: values.bottom,
        left: values.left,
    }
}

fn to_surface_style(style: &ComputedStyle) -> SurfaceStyle {
    // Prefer gradients (radial → linear) when available, else treat CSS variables/inherit/currentColor as no explicit color.
    // no debug prints
    let background = if let Some((start, end, cx, cy, rx, ry)) = &style.background_radial {
        Some(ViewBackground::RadialGradient {
            cx: *cx,
            cy: *cy,
            rx: *rx,
            ry: *ry,
            stops: vec![(start.clone(), 0.0), (end.clone(), 1.0)],
        })
    } else if let Some((start, end, angle)) = &style.background_gradient {
        Some(ViewBackground::LinearGradient {
            angle: *angle,
            stops: vec![(start.clone(), 0.0), (end.clone(), 1.0)],
        })
    } else {
        style.background_color.as_ref().and_then(|c| {
            let v = c.trim();
            if v.eq_ignore_ascii_case("inherit")
                || v.eq_ignore_ascii_case("currentcolor")
                || v.eq_ignore_ascii_case("none")
                || v.starts_with("var(")
            {
                None
            } else {
                Some(ViewBackground::Solid {
                    color: v.to_string(),
                })
            }
        })
    };
    SurfaceStyle {
        background,
        backgrounds: style.backgrounds.clone(),
        padding: to_edge_insets(&style.padding),
        margin: to_edge_insets(&style.margin),
        corner_radius: style.corner_radius,
        width: style.width,
        height: style.height,
        min_width: style.min_width,
        min_height: style.min_height,
        max_width: style.max_width,
        max_height: style.max_height,
        border_width: style.border_width,
        border_color: style.border_color.clone(),
        box_shadow: style.box_shadow.as_ref().map(|(x, y, blur, color)| {
            crate::view::BoxShadowSpec {
                offset_x: *x,
                offset_y: *y,
                blur: *blur,
                color: color.clone(),
            }
        }),
    }
}

fn to_text_style(style: &ComputedStyle) -> TextStyle {
    // Inherit/currentColor/var(...) mean "no explicit color" here; let parent/defaults decide.
    let color = match style.color.as_ref().map(|s| s.trim()) {
        Some(v) if v.eq_ignore_ascii_case("inherit") => None,
        Some(v) if v.eq_ignore_ascii_case("currentcolor") => None,
        Some(v) if v.starts_with("var(") => None,
        other => other.map(|s| s.to_string()),
    };
    let font_family = match style.font_family.as_ref().map(|s| s.trim()) {
        Some(v) if v.eq_ignore_ascii_case("inherit") => None,
        Some(v) if v.starts_with("var(") => None,
        other => other.map(|s| s.to_string()),
    };
    TextStyle {
        color,
        font_family,
        font_size: style.font_size,
        line_height: style.line_height,
        font_weight: style.font_weight,
        background: if let Some((start, end, cx, cy, rx, ry)) = &style.background_radial {
            Some(ViewBackground::RadialGradient {
                cx: *cx,
                cy: *cy,
                rx: *rx,
                ry: *ry,
                stops: vec![(start.clone(), 0.0), (end.clone(), 1.0)],
            })
        } else if let Some((start, end, angle)) = &style.background_gradient {
            Some(ViewBackground::LinearGradient {
                angle: *angle,
                stops: vec![(start.clone(), 0.0), (end.clone(), 1.0)],
            })
        } else {
            style.background_color.as_ref().and_then(|c| {
                let v = c.trim();
                if v.eq_ignore_ascii_case("inherit")
                    || v.eq_ignore_ascii_case("currentcolor")
                    || v.eq_ignore_ascii_case("none")
                    || v.starts_with("var(")
                {
                    None
                } else {
                    Some(ViewBackground::Solid {
                        color: v.to_string(),
                    })
                }
            })
        },
        padding: to_edge_insets(&style.padding),
        margin: to_edge_insets(&style.margin),
        corner_radius: style.corner_radius,
        text_align: style.text_align,
    }
}

fn apply_heading_text_defaults(style: &mut TextStyle, level: u8) {
    let defaults = heading_text_defaults(level);
    if style.font_size.is_none() {
        style.font_size = Some(defaults.font_size);
    }
    if style.line_height.is_none() {
        style.line_height = Some(defaults.line_height);
    }
    if style.font_weight.is_none() {
        style.font_weight = Some(defaults.font_weight);
    }
    // Do not inject UA default vertical margins for headings unless explicitly enabled.
    // This keeps heading and paragraph boxes snapped without gaps in our renderer,
    // until we support full CSS margin-collapsing semantics.
    let enable_heading_margins = std::env::var("RUNE_UA_HEADING_MARGINS")
        .ok()
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "on"))
        .unwrap_or(false);
    if enable_heading_margins && style.margin.top == 0.0 && style.margin.bottom == 0.0 {
        style.margin.top = defaults.margin_top;
        style.margin.bottom = defaults.margin_bottom;
    }
}

struct HeadingTextDefaults {
    font_size: f64,
    line_height: f64,
    font_weight: f64,
    margin_top: f64,
    margin_bottom: f64,
}

fn heading_text_defaults(level: u8) -> HeadingTextDefaults {
    match level {
        1 => HeadingTextDefaults {
            font_size: 32.0,
            line_height: 40.0,
            font_weight: 700.0,
            margin_top: 24.0,
            margin_bottom: 16.0,
        },
        2 => HeadingTextDefaults {
            font_size: 28.0,
            line_height: 36.0,
            font_weight: 700.0,
            margin_top: 20.0,
            margin_bottom: 12.0,
        },
        3 => HeadingTextDefaults {
            font_size: 24.0,
            line_height: 32.0,
            font_weight: 700.0,
            margin_top: 16.0,
            margin_bottom: 10.0,
        },
        4 => HeadingTextDefaults {
            font_size: 20.0,
            line_height: 28.0,
            font_weight: 700.0,
            margin_top: 14.0,
            margin_bottom: 8.0,
        },
        5 => HeadingTextDefaults {
            font_size: 18.0,
            line_height: 26.0,
            font_weight: 700.0,
            margin_top: 12.0,
            margin_bottom: 6.0,
        },
        _ => HeadingTextDefaults {
            font_size: 16.0,
            line_height: 22.0,
            font_weight: 700.0,
            margin_top: 10.0,
            margin_bottom: 4.0,
        },
    }
}

fn build_flex_layout(style: &ComputedStyle) -> FlexLayout {
    let mut layout = FlexLayout::default();
    if let Some(direction) = style.flex_direction {
        layout.direction = direction;
    } else if matches!(style.display, Display::Flex) {
        // CSS default for flex containers is row when unspecified
        layout.direction = LayoutDirection::Row;
    }
    if let Some(justify) = style.justify_content {
        layout.justify = justify;
    }
    if let Some(align) = style.align_items {
        layout.align = align;
    }
    if let Some(wrap) = style.wrap {
        layout.wrap = wrap;
    }
    if let Some(gap) = style.gap {
        layout.gap = gap;
    }
    layout
}

#[derive(Debug, Clone)]
struct CssRule {
    selector: SimpleSelector,
    declarations: Vec<CssDeclaration>,
    specificity: Specificity,
    order: usize,
}

#[derive(Debug, Clone)]
struct CssDeclaration {
    name: String,
    value: String,
}

#[derive(Debug, Clone, Default)]
struct SimpleSelector {
    tag: Option<String>,
    id: Option<String>,
    classes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Specificity {
    inline: bool,
    ids: u32,
    classes: u32,
    elements: u32,
}

impl Specificity {
    fn for_selector(selector: &SimpleSelector) -> Self {
        Self {
            inline: false,
            ids: selector.id.as_ref().map(|_| 1).unwrap_or(0),
            classes: selector.classes.len() as u32,
            elements: selector.tag.as_ref().map(|_| 1).unwrap_or(0),
        }
    }

    fn inline() -> Self {
        Self {
            inline: true,
            ids: 0,
            classes: 0,
            elements: 0,
        }
    }
}

struct StyleResolver {
    rules: Vec<CssRule>,
}

impl StyleResolver {
    fn new(rules: Vec<CssRule>) -> Self {
        Self { rules }
    }

    fn resolve(&self, element: &ElementRef) -> ComputedStyle {
        let mut builder = StyleBuilder::new();
        for rule in &self.rules {
            if rule.selector.matches(element) {
                builder.apply(&rule.declarations, rule.specificity, rule.order);
            }
        }
        if let Some(inline) = element.value().attr("style") {
            let declarations = parse_declarations(inline);
            builder.apply(&declarations, Specificity::inline(), usize::MAX);
        }
        builder.build()
    }
}

impl SimpleSelector {
    fn matches(&self, element: &scraper::ElementRef) -> bool {
        if let Some(tag) = &self.tag {
            if !element.value().name().eq_ignore_ascii_case(tag) {
                return false;
            }
        }
        if let Some(id) = &self.id {
            match element.value().attr("id") {
                Some(value) if value == id => {}
                _ => return false,
            }
        }
        if !self.classes.is_empty() {
            for class in &self.classes {
                let has_class = element
                    .value()
                    .classes()
                    .any(|candidate| candidate.eq_ignore_ascii_case(class));
                if !has_class {
                    return false;
                }
            }
        }
        true
    }
}

struct StyleBuilder {
    properties: HashMap<String, (Specificity, usize, String)>,
}

impl StyleBuilder {
    fn new() -> Self {
        Self {
            properties: HashMap::new(),
        }
    }

    fn apply(&mut self, declarations: &[CssDeclaration], specificity: Specificity, order: usize) {
        for declaration in declarations {
            let key = declaration.name.clone();
            let value = declaration.value.clone();
            let new_priority = (specificity, order);
            let should_insert = match self.properties.get(&key) {
                Some(existing) => compare_priority(existing, &new_priority),
                None => true,
            };
            if should_insert {
                self.properties.insert(key, (specificity, order, value));
            }
        }
    }

    fn build(self) -> ComputedStyle {
        let mut style = ComputedStyle::default();
        let get = |name: &str| -> Option<&str> {
            self.properties
                .get(name)
                .map(|(_, _, value)| value.as_str())
        };

        if let Some(display) = get("display") {
            style.display = match display {
                "none" => Display::None,
                "flex" | "inline-flex" => Display::Flex,
                "inline" => Display::Inline,
                _ => Display::Block,
            };
        }

        if let Some(direction) = get("flex-direction") {
            style.flex_direction = match direction {
                "row" => Some(LayoutDirection::Row),
                "column" => Some(LayoutDirection::Column),
                _ => None,
            };
        }
        if let Some(wrap) = get("flex-wrap") {
            style.wrap = match wrap.to_ascii_lowercase().as_str() {
                "wrap" | "wrap-reverse" => Some(true),
                "nowrap" => Some(false),
                _ => None,
            };
        }

        if let Some(justify) = get("justify-content") {
            style.justify_content = match justify {
                "center" => Some(LayoutJustify::Center),
                "flex-end" | "end" => Some(LayoutJustify::End),
                "space-between" => Some(LayoutJustify::SpaceBetween),
                _ => Some(LayoutJustify::Start),
            };
        }

        if let Some(align) = get("align-items") {
            style.align_items = match align {
                "center" => Some(LayoutAlign::Center),
                "flex-end" | "end" => Some(LayoutAlign::End),
                "stretch" => Some(LayoutAlign::Stretch),
                _ => Some(LayoutAlign::Start),
            };
        }

        if let Some(gap) = get("gap").and_then(parse_length) {
            style.gap = Some(gap);
        }

        if let Some(color) = get("color") {
            style.color = Some(color.to_string());
        }

        if let Some(font_family) = get("font-family") {
            style.font_family = Some(font_family.to_string());
        }

        if let Some(font_size) = get("font-size").and_then(parse_length) {
            style.font_size = Some(font_size);
        }

        if let Some(line_height_value) = get("line-height") {
            if let Some(line_height) = parse_length(line_height_value) {
                style.line_height = Some(line_height);
            }
        }

        if let Some(font_weight_value) = get("font-weight") {
            if let Some(font_weight) = parse_font_weight(font_weight_value) {
                style.font_weight = Some(font_weight);
            }
        }

        if let Some(background) = get("background-color") {
            style.background_color = Some(background.to_string());
        } else if let Some(background) = get("background") {
            if background.starts_with('#') || background.starts_with("rgb") {
                style.background_color = Some(background.to_string());
            } else if let Some((start, end, angle)) = parse_linear_gradient(background) {
                style.background_gradient = Some((start, end, angle));
                style.background_color = None;
            }
        }

        if let Some(margin) = get("margin") {
            if let Some(values) = parse_edge_values(margin) {
                style.margin.apply(values);
            }
        }
        if let Some(margin_top) = get("margin-top").and_then(parse_length) {
            style.margin.top = margin_top;
        }
        if let Some(margin_right) = get("margin-right").and_then(parse_length) {
            style.margin.right = margin_right;
        }
        if let Some(margin_bottom) = get("margin-bottom").and_then(parse_length) {
            style.margin.bottom = margin_bottom;
        }
        if let Some(margin_left) = get("margin-left").and_then(parse_length) {
            style.margin.left = margin_left;
        }

        if let Some(padding) = get("padding") {
            if let Some(values) = parse_edge_values(padding) {
                style.padding.apply(values);
            }
        }
        if let Some(padding_top) = get("padding-top").and_then(parse_length) {
            style.padding.top = padding_top;
        }
        if let Some(padding_right) = get("padding-right").and_then(parse_length) {
            style.padding.right = padding_right;
        }
        if let Some(padding_bottom) = get("padding-bottom").and_then(parse_length) {
            style.padding.bottom = padding_bottom;
        }
        if let Some(padding_left) = get("padding-left").and_then(parse_length) {
            style.padding.left = padding_left;
        }

        if let Some(width) = get("width").and_then(parse_length) {
            style.width = Some(width);
        }
        if let Some(height) = get("height").and_then(parse_length) {
            style.height = Some(height);
        }
        if let Some(v) = get("min-width").and_then(parse_length) {
            style.min_width = Some(v);
        }
        if let Some(v) = get("min-height").and_then(parse_length) {
            style.min_height = Some(v);
        }
        if let Some(v) = get("max-width").and_then(parse_length) {
            style.max_width = Some(v);
        }
        if let Some(v) = get("max-height").and_then(parse_length) {
            style.max_height = Some(v);
        }

        if let Some(fit) = get("object-fit") {
            style.content_fit = match fit {
                "cover" => Some(ImageContentFit::Cover),
                "contain" => Some(ImageContentFit::Contain),
                "fill" => Some(ImageContentFit::Fill),
                _ => None,
            };
        }

        // Diagnostics: log ignored CSS properties not recognized by the translator.
        if diagnostics_enabled("css") {
            use std::collections::HashSet;
            let recognized: HashSet<&'static str> = [
                "display",
                "flex-direction",
                "flex-wrap",
                "justify-content",
                "align-items",
                "gap",
                "color",
                "font-size",
                "line-height",
                "font-weight",
                "background-color",
                "background",
                "margin",
                "margin-top",
                "margin-right",
                "margin-bottom",
                "margin-left",
                "padding",
                "padding-top",
                "padding-right",
                "padding-bottom",
                "padding-left",
                "width",
                "height",
                "object-fit",
                "border-radius",
            ]
            .into_iter()
            .collect();
            for (name, (_, _, value)) in &self.properties {
                // Skip custom properties (--*) and vendor-prefixed (-webkit-*, -moz-*, etc.).
                let n = name.as_str();
                if n.starts_with("--") || n.starts_with('-') {
                    continue;
                }
                if !recognized.contains(n) {
                    info!(property = %name, value = %value, "diagnostics: ignored CSS property");
                }
            }
        }

        style
    }
}

fn compare_priority(existing: &(Specificity, usize, String), new: &(Specificity, usize)) -> bool {
    let (existing_spec, existing_order, _) = existing;
    let (new_spec, new_order) = new;
    if existing_spec.inline != new_spec.inline {
        return new_spec.inline;
    }
    let existing_tuple = (
        existing_spec.ids,
        existing_spec.classes,
        existing_spec.elements,
    );
    let new_tuple = (new_spec.ids, new_spec.classes, new_spec.elements);
    if new_tuple > existing_tuple {
        return true;
    }
    if new_tuple == existing_tuple {
        return new_order >= existing_order;
    }
    false
}

fn parse_declarations(source: &str) -> Vec<CssDeclaration> {
    source
        .split(';')
        .filter_map(|declaration| {
            let (name, value) = declaration.split_once(':')?;
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim();
            if name.is_empty() || value.is_empty() {
                return None;
            }
            Some(CssDeclaration {
                name,
                value: value.to_string(),
            })
        })
        .collect()
}

// moved to crate::css::bridge
#[cfg(feature = "cssv2")]
pub(crate) fn apply_cssv2_inline_to_style(
    style: &mut ComputedStyle,
    v2: &crate::css::ComputedStyle2,
) {
    use crate::css::Display2;
    if let Some(d) = v2.display {
        style.display = match d {
            Display2::Block => Display::Block,
            Display2::Flex => Display::Flex,
            // Do not map grid to flex; keep it as block for classification.
            Display2::Grid => Display::Block,
            Display2::Inline => Display::Inline,
            Display2::None => Display::None,
        };
    }
    if let Some(dir) = v2.flex_direction {
        style.flex_direction = Some(dir);
    }
    if let Some(j) = v2.justify_content {
        style.justify_content = Some(j);
    }
    if let Some(a) = v2.align_items {
        style.align_items = Some(a);
    }
    if let Some(a) = v2.text_align {
        style.text_align = Some(match a {
            crate::css::TextAlign2::Start => crate::view::TextAlign::Start,
            crate::css::TextAlign2::Center => crate::view::TextAlign::Center,
            crate::css::TextAlign2::End => crate::view::TextAlign::End,
        });
    }
    if let Some(w) = v2.wrap {
        style.wrap = Some(w);
    }
    if let Some(g) = v2.gap {
        style.gap = Some(g);
    }
    if let Some(c) = v2.color.clone() {
        style.color = Some(c);
    }
    if let Some(f) = v2.font_family.clone() {
        style.font_family = Some(f);
    }
    if let Some(bg) = v2.background_color.clone() {
        style.background_color = Some(bg);
    }
    if let Some(grad) = v2.background_gradient.clone() {
        let (start, end) = if grad.stops.is_empty() {
            ("#000000".to_string(), "#000000".to_string())
        } else {
            (
                grad.stops
                    .first()
                    .map(|s| s.color.clone())
                    .unwrap_or("#000000".to_string()),
                grad.stops
                    .last()
                    .map(|s| s.color.clone())
                    .unwrap_or("#000000".to_string()),
            )
        };
        style.background_gradient = Some((start, end, grad.angle));
        // Clear background_color to prefer gradient if both are provided.
        style.background_color = None;
    }
    if let Some(rad) = v2.background_radial.clone() {
        let (start, end) = if rad.stops.is_empty() {
            ("#000000".to_string(), "#000000".to_string())
        } else {
            (
                rad.stops
                    .first()
                    .map(|s| s.color.clone())
                    .unwrap_or("#000000".to_string()),
                rad.stops
                    .last()
                    .map(|s| s.color.clone())
                    .unwrap_or("#000000".to_string()),
            )
        };
        style.background_radial = Some((start, end, rad.cx, rad.cy, rad.rx, rad.ry));
        style.background_color = None;
    }
    if !v2.background_layers.is_empty() {
        style.backgrounds = v2
            .background_layers
            .iter()
            .map(|layer| match layer {
                crate::css::BackgroundLayer2::Solid(color) => ViewBackground::Solid {
                    color: color.clone(),
                },
                crate::css::BackgroundLayer2::Linear(g) => {
                    let stops: Vec<(String, f64)> = g
                        .stops
                        .iter()
                        .map(|s| (s.color.clone(), s.offset))
                        .collect();
                    ViewBackground::LinearGradient {
                        angle: g.angle,
                        stops,
                    }
                }
                crate::css::BackgroundLayer2::Radial(g) => {
                    let stops: Vec<(String, f64)> = g
                        .stops
                        .iter()
                        .map(|s| (s.color.clone(), s.offset))
                        .collect();
                    ViewBackground::RadialGradient {
                        cx: g.cx,
                        cy: g.cy,
                        rx: g.rx,
                        ry: g.ry,
                        stops,
                    }
                }
            })
            .collect();
    }
    if let Some(rad) = v2.background_radial.clone() {
        let (start, end) = if rad.stops.is_empty() {
            ("#000000".to_string(), "#000000".to_string())
        } else {
            (
                rad.stops
                    .first()
                    .map(|s| s.color.clone())
                    .unwrap_or("#000000".to_string()),
                rad.stops
                    .last()
                    .map(|s| s.color.clone())
                    .unwrap_or("#000000".to_string()),
            )
        };
        style.background_radial = Some((start, end, rad.cx, rad.cy, rad.rx, rad.ry));
        style.background_color = None;
    }
    if v2.margin.top != 0.0
        || v2.margin.right != 0.0
        || v2.margin.bottom != 0.0
        || v2.margin.left != 0.0
    {
        style.margin.apply([
            v2.margin.top,
            v2.margin.right,
            v2.margin.bottom,
            v2.margin.left,
        ]);
    }
    if v2.padding.top != 0.0
        || v2.padding.right != 0.0
        || v2.padding.bottom != 0.0
        || v2.padding.left != 0.0
    {
        style.padding.apply([
            v2.padding.top,
            v2.padding.right,
            v2.padding.bottom,
            v2.padding.left,
        ]);
    }
    if let Some(w) = v2.width {
        style.width = Some(w);
    }
    if let Some(h) = v2.height {
        style.height = Some(h);
    }
    if let Some(w) = v2.min_width {
        style.min_width = Some(w);
    }
    if let Some(h) = v2.min_height {
        style.min_height = Some(h);
    }
    if let Some(w) = v2.max_width {
        style.max_width = Some(w);
    }
    if let Some(h) = v2.max_height {
        style.max_height = Some(h);
    }
    if let Some(fit) = v2.object_fit {
        style.content_fit = Some(fit);
    }
    if let Some(r) = v2.corner_radius {
        style.corner_radius = Some(r);
    }
    // Border fallback mapping: prefer uniform; else pick a side in bottom→top→right→left order
    // Only map uniform borders; per-side is consumed directly when building containers.
    if let Some(w) = v2.border_width {
        style.border_width = Some(w);
    }
    if let Some(c) = v2.border_color.clone() {
        style.border_color = Some(c);
    }
    if let Some(sh) = v2.box_shadow.clone() {
        style.box_shadow = Some((sh.offset_x, sh.offset_y, sh.blur, sh.color));
    }
}

fn parse_edge_values(input: &str) -> Option<[f64; 4]> {
    let parts: Vec<f64> = input.split_whitespace().filter_map(parse_length).collect();
    match parts.as_slice() {
        [] => None,
        [single] => Some([*single, *single, *single, *single]),
        [vertical, horizontal] => Some([*vertical, *horizontal, *vertical, *horizontal]),
        [top, horizontal, bottom] => Some([*top, *horizontal, *bottom, *horizontal]),
        [top, right, bottom, left, ..] => Some([*top, *right, *bottom, *left]),
    }
}

impl EdgeValues {
    fn apply(&mut self, values: [f64; 4]) {
        self.top = values[0];
        self.right = values[1];
        self.bottom = values[2];
        self.left = values[3];
    }
}

fn parse_length(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(stripped) = trimmed.strip_suffix("px") {
        return stripped.trim().parse().ok();
    }
    trimmed.parse().ok()
}

// Minimal parser for a single box-shadow value of the form:
//   <offset-x> <offset-y> [blur-radius]? <color?>
// Returns (x, y, blur, color). If color is omitted, a default semi-transparent black is used.
// Supports colors with spaces (e.g., rgba(2, 6, 23, 0.45)) by treating the remainder after
// consuming lengths as the color string.
fn parse_box_shadow_value(input: &str) -> Option<(f64, f64, f64, String)> {
    let mut s = input.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("none") {
        return None;
    }

    // Helper: parse a leading length token from the start of the string.
    fn parse_leading_length_token(s: &str) -> Option<(f64, usize)> {
        let bytes = s.as_bytes();
        // Skip leading whitespace
        let mut i = 0usize;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let start = i;
        if start >= bytes.len() {
            return None;
        }
        // Accept optional sign
        if bytes[i] == b'+' || bytes[i] == b'-' {
            i += 1;
        }
        // Parse digits/decimal
        while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
            i += 1;
        }
        // Optional unit suffix (only px supported here)
        let mut j = i;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        let token_end = if s[start..i].is_empty() {
            return None;
        } else if s[i..].starts_with("px") {
            i + 2
        } else {
            i
        };
        let token = &s[start..token_end];
        let value = if let Some(num) = token.strip_suffix("px") {
            num.trim()
        } else {
            token
        };
        let parsed = value.trim().parse::<f64>().ok()?;
        Some((parsed, token_end))
    }

    // Parse x and y
    let (x, used1) = parse_leading_length_token(s)?;
    s = &s[used1..];
    let (y, used2) = parse_leading_length_token(s)?;
    s = &s[used2..];

    // Optional blur
    let (blur, used3_opt) = if let Some((b, used3)) = parse_leading_length_token(s) {
        (b, Some(used3))
    } else {
        (0.0, None)
    };
    if let Some(used3) = used3_opt {
        s = &s[used3..];
    }

    // Remainder is the color string, if any
    let color = {
        let c = s.trim();
        if c.is_empty() {
            "rgba(0,0,0,0.25)".to_string()
        } else {
            c.to_string()
        }
    };

    Some((x, y, blur, color))
}

fn parse_linear_gradient(input: &str) -> Option<(String, String, f64)> {
    let s = input.trim();
    let lower = s.to_ascii_lowercase();
    if !lower.starts_with("linear-gradient(") || !s.ends_with(')') {
        return None;
    }
    let inner = &s[s.find('(')? + 1..s.rfind(')')?];
    let mut parts = inner.split(',').map(|p| p.trim()).collect::<Vec<_>>();
    if parts.len() < 2 {
        return None;
    }
    let mut angle: f64 = 0.0;
    if parts[0].ends_with("deg") {
        let num = parts[0].trim_end_matches("deg").trim();
        if let Ok(a) = num.parse::<f64>() {
            angle = a;
            parts.remove(0);
        }
    }
    if parts.len() < 2 {
        return None;
    }
    let start = parts[0].to_string();
    let end = parts[1].to_string();
    Some((start, end, angle))
}

fn parse_number(value: &str) -> Option<f64> {
    value.trim().parse().ok()
}

fn parse_font_weight(value: &str) -> Option<f64> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "normal" => Some(400.0),
        "bold" => Some(700.0),
        "lighter" => Some(300.0),
        "bolder" => Some(700.0),
        other => parse_number(other),
    }
}

fn collect_css_rules(document: &Html) -> Vec<CssRule> {
    let mut rules = Vec::new();
    let mut order = 0usize;
    if let Ok(selector) = Selector::parse("style") {
        for node in document.select(&selector) {
            let css = node.text().collect::<String>();
            let parsed = parse_rules(&css, &mut order);
            rules.extend(parsed);
        }
    }
    rules
}

// Minimal extraction of custom properties from :root blocks.
// This is intentionally narrow: only top-level :root selectors are considered,
// and only simple "--name: value;" declarations are collected.
fn collect_root_custom_properties(document: &Html) -> HashMap<String, String> {
    let mut vars: HashMap<String, String> = HashMap::new();
    let Some(selector) = Selector::parse("style").ok() else {
        return vars;
    };
    for node in document.select(&selector) {
        let raw_css = node.text().collect::<String>();
        let css = strip_css_comments(&raw_css);
        // Parse top-level blocks: selector { body }
        let mut chars = css.chars().peekable();
        let mut current_selector = String::new();
        let mut in_selector = true;
        let mut depth: i32 = 0;
        let mut body = String::new();
        while let Some(ch) = chars.next() {
            match ch {
                '{' => {
                    depth += 1;
                    if depth == 1 {
                        in_selector = false;
                        body.clear();
                        continue;
                    } else {
                        body.push(ch);
                    }
                }
                '}' => {
                    if depth > 0 {
                        depth -= 1;
                    }
                    if depth == 0 {
                        // We have a full block: current_selector { body }
                        let selector_text = current_selector.trim();
                        if selector_text.contains(":root") {
                            for decl in parse_declarations(&body) {
                                if decl.name.starts_with("--") {
                                    vars.insert(decl.name, decl.value.trim().to_string());
                                }
                            }
                        }
                        current_selector.clear();
                        in_selector = true;
                        continue;
                    } else {
                        body.push(ch);
                    }
                }
                _ => {
                    if in_selector {
                        current_selector.push(ch);
                    } else {
                        body.push(ch);
                    }
                }
            }
        }
    }
    vars
}

// Resolve var(--name[, fallback]) for color and background-color fields only.
fn resolve_vars_in_style(style: &mut ComputedStyle, vars: &HashMap<String, String>) {
    if let Some(value) = style.color.as_ref() {
        if let Some(resolved) = resolve_var_expression(value, vars) {
            style.color = Some(resolved);
        }
    }
    if let Some(value) = style.background_color.as_ref() {
        if let Some(resolved) = resolve_var_expression(value, vars) {
            style.background_color = Some(resolved);
        }
    }
    if let Some((start, end, angle)) = style.background_gradient.clone() {
        let mut changed = false;
        let start2 = if let Some(resolved) = resolve_var_expression(&start, vars) {
            changed = true;
            resolved
        } else {
            start
        };
        let end2 = if let Some(resolved) = resolve_var_expression(&end, vars) {
            changed = true;
            resolved
        } else {
            end
        };
        if changed {
            style.background_gradient = Some((start2, end2, angle));
        }
    }
    if !style.backgrounds.is_empty() {
        let mut changed_any = false;
        let mut new_layers: Vec<ViewBackground> = Vec::with_capacity(style.backgrounds.len());
        for layer in &style.backgrounds {
            match layer {
                ViewBackground::Solid { color } => {
                    if let Some(res) = resolve_var_expression(color, vars) {
                        new_layers.push(ViewBackground::Solid { color: res });
                        changed_any = true;
                    } else {
                        new_layers.push(layer.clone());
                    }
                }
                ViewBackground::LinearGradient { angle, stops } => {
                    let mut any = false;
                    let mut new_stops: Vec<(String, f64)> = Vec::with_capacity(stops.len());
                    for (c, off) in stops {
                        if let Some(res) = resolve_var_expression(c, vars) {
                            new_stops.push((res, *off));
                            any = true;
                        } else {
                            new_stops.push((c.clone(), *off));
                        }
                    }
                    if any {
                        changed_any = true;
                    }
                    new_layers.push(ViewBackground::LinearGradient {
                        angle: *angle,
                        stops: new_stops,
                    });
                }
                ViewBackground::RadialGradient {
                    cx,
                    cy,
                    rx,
                    ry,
                    stops,
                } => {
                    let mut any = false;
                    let mut new_stops: Vec<(String, f64)> = Vec::with_capacity(stops.len());
                    for (c, off) in stops {
                        if let Some(res) = resolve_var_expression(c, vars) {
                            new_stops.push((res, *off));
                            any = true;
                        } else {
                            new_stops.push((c.clone(), *off));
                        }
                    }
                    if any {
                        changed_any = true;
                    }
                    new_layers.push(ViewBackground::RadialGradient {
                        cx: *cx,
                        cy: *cy,
                        rx: *rx,
                        ry: *ry,
                        stops: new_stops,
                    });
                }
            }
        }
        if changed_any {
            style.backgrounds = new_layers;
        }
    }
    if let Some((start, end, cx, cy, rx, ry)) = style.background_radial.clone() {
        let mut changed = false;
        let start2 = if let Some(resolved) = resolve_var_expression(&start, vars) {
            changed = true;
            resolved
        } else {
            start
        };
        let end2 = if let Some(resolved) = resolve_var_expression(&end, vars) {
            changed = true;
            resolved
        } else {
            end
        };
        if changed {
            style.background_radial = Some((start2, end2, cx, cy, rx, ry));
        }
    }
    if let Some(value) = style.border_color.as_ref() {
        if let Some(resolved) = resolve_var_expression(value, vars) {
            style.border_color = Some(resolved);
        }
    }
    if let Some((x, y, blur, color)) = style.box_shadow.clone() {
        if let Some(resolved) = resolve_var_expression(&color, vars) {
            // If the variable expands to a full box-shadow string (e.g., "0 8px 24px rgba(...)"),
            // parse it into components; otherwise treat it as just a color.
            if let Some((ox, oy, bl, col)) = parse_box_shadow_value(&resolved) {
                style.box_shadow = Some((ox, oy, bl, col));
            } else {
                style.box_shadow = Some((x, y, blur, resolved));
            }
        }
    }
}

fn resolve_var_expression(input: &str, vars: &HashMap<String, String>) -> Option<String> {
    let trimmed = input.trim();
    if !trimmed.to_ascii_lowercase().starts_with("var(") {
        return None;
    }
    // Find the first '(' and the matching ')'
    let open = trimmed.find('(')?;
    let close = trimmed.rfind(')')?;
    if close <= open + 1 {
        return None;
    }
    let inner = &trimmed[open + 1..close];
    let mut parts = inner.splitn(2, ',');
    let name = parts.next()?.trim();
    let fallback = parts.next().map(|s| s.trim());
    // Expect a custom property name like --color
    if !name.starts_with("--") {
        return fallback.map(|s| s.to_string());
    }
    if let Some(value) = vars.get(name) {
        Some(value.clone())
    } else {
        fallback.map(|s| s.to_string())
    }
}

fn strip_css_comments(source: &str) -> String {
    // Remove /* ... */ comment blocks conservatively.
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
            continue;
        } else if ch == '/' {
            if let Some('*') = chars.peek().copied() {
                chars.next();
                in_comment = true;
                continue;
            }
        }
        out.push(ch);
    }
    out
}

fn parse_rules(css: &str, order: &mut usize) -> Vec<CssRule> {
    let css = strip_css_comments(css);
    let mut rules = Vec::new();
    for raw_rule in css.split('}') {
        let raw_rule = raw_rule.trim();
        if raw_rule.is_empty() {
            continue;
        }
        let Some((selector_part, body_part)) = raw_rule.split_once('{') else {
            continue;
        };
        let selectors = parse_selectors(selector_part);
        if selectors.is_empty() {
            continue;
        }
        let declarations = parse_declarations(body_part);
        if declarations.is_empty() {
            continue;
        }
        for selector in selectors {
            *order += 1;
            let specificity = Specificity::for_selector(&selector);
            rules.push(CssRule {
                selector,
                declarations: declarations.clone(),
                specificity,
                order: *order,
            });
        }
    }
    rules
}

fn parse_selectors(raw: &str) -> Vec<SimpleSelector> {
    raw.split(',')
        .filter_map(|selector| parse_simple_selector(selector.trim()))
        .collect()
}

fn parse_simple_selector(selector: &str) -> Option<SimpleSelector> {
    if selector.is_empty() || selector.contains(' ') || selector.contains('>') {
        return None;
    }
    let mut simple = SimpleSelector::default();
    let mut buffer = String::new();
    let mut chars = selector.chars().peekable();
    while let Some(ch) = chars.peek().copied() {
        match ch {
            '.' => {
                chars.next();
                buffer.clear();
                while let Some(next) = chars.peek().copied() {
                    if next.is_alphanumeric() || next == '-' || next == '_' {
                        buffer.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if !buffer.is_empty() {
                    simple.classes.push(buffer.to_ascii_lowercase());
                }
            }
            '#' => {
                chars.next();
                buffer.clear();
                while let Some(next) = chars.peek().copied() {
                    if next.is_alphanumeric() || next == '-' || next == '_' {
                        buffer.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if !buffer.is_empty() {
                    simple.id = Some(buffer.to_string());
                }
            }
            ':' => {
                // Skip pseudo-classes and pseudo-elements (e.g., :root, :not(...)).
                // They are currently unsupported; consume so we don't hang.
                chars.next(); // consume ':'
                buffer.clear();
                // Capture pseudo name
                while let Some(next) = chars.peek().copied() {
                    if next.is_alphanumeric() || next == '-' || next == '_' {
                        buffer.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                // Optionally skip (:...) content
                if let Some('(') = chars.peek().copied() {
                    let mut depth: i32 = 0;
                    while let Some(c) = chars.next() {
                        if c == '(' {
                            depth += 1;
                        } else if c == ')' {
                            depth -= 1;
                            if depth <= 0 {
                                break;
                            }
                        }
                    }
                }
                if diagnostics_enabled("css") && !buffer.is_empty() {
                    info!(pseudo = %buffer, "diagnostics: ignored CSS pseudo selector");
                }
            }
            '[' => {
                // Skip attribute selectors (unsupported). Consume until the closing ']'.
                chars.next(); // consume '['
                while let Some(c) = chars.next() {
                    if c == ']' {
                        break;
                    }
                }
                if diagnostics_enabled("css") {
                    info!("diagnostics: ignored CSS attribute selector");
                }
            }
            '*' => {
                chars.next();
            }
            _ => {
                buffer.clear();
                while let Some(next) = chars.peek().copied() {
                    if next.is_alphanumeric() || next == '-' {
                        buffer.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if !buffer.is_empty() {
                    simple.tag = Some(buffer.to_ascii_lowercase());
                }
                // If we couldn't consume anything (unknown char), advance by one to avoid infinite loops.
                if buffer.is_empty() {
                    let _ = chars.next();
                }
            }
        }
    }
    Some(simple)
}

fn empty_container_spec() -> FlexContainerSpec {
    FlexContainerSpec {
        layout: FlexLayout::default(),
        padding: EdgeInsets::default(),
        margin: EdgeInsets::default(),
        margin_left_auto: false,
        margin_right_auto: false,
        background: None,
        backgrounds: Vec::new(),
        corner_radius: None,
        width: None,
        height: None,
        min_width: None,
        min_height: None,
        max_width: None,
        max_height: None,
        border_width: None,
        border_color: None,
        border_top_width: None,
        border_right_width: None,
        border_bottom_width: None,
        border_left_width: None,
        border_top_color: None,
        border_right_color: None,
        border_bottom_color: None,
        border_left_color: None,
        box_shadow: None,
        scroll: ScrollBehavior::default(),
        children: Vec::new(),
    }
}

fn collect_text(node: &NodeRef<Node>) -> String {
    match node.value() {
        Node::Text(text) => text.deref().to_string(),
        _ => {
            let mut content = String::new();
            for child in node.children() {
                content.push_str(&collect_text(&child));
            }
            content
        }
    }
}

fn normalize_whitespace(input: &str) -> String {
    let mut result = String::new();
    let mut prev_was_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !prev_was_space {
                if !result.is_empty() {
                    result.push(' ');
                }
                prev_was_space = true;
            }
        } else {
            result.push(ch);
            prev_was_space = false;
        }
    }
    result.trim().to_string()
}

fn collect_raw_text(node: &NodeRef<Node>) -> String {
    match node.value() {
        Node::Text(text) => text.deref().to_string(),
        _ => {
            let mut content = String::new();
            for child in node.children() {
                content.push_str(&collect_raw_text(&child));
            }
            content
        }
    }
}

fn log_fallback_tag_once(tag: &str, fallback: &str) {
    static SEEN: OnceLock<Mutex<std::collections::HashSet<String>>> = OnceLock::new();
    if !(diagnostics_enabled("html")) {
        return;
    }
    let set = SEEN.get_or_init(|| Mutex::new(std::collections::HashSet::new()));
    if let Ok(mut guard) = set.lock() {
        if guard.insert(tag.to_string()) {
            info!(tag = %tag, fallback = %fallback, "diagnostics: unsupported tag fell back");
        }
    }
}

fn sanitize_identifier(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            result.push(ch.to_ascii_lowercase());
        } else if ch.is_whitespace() {
            result.push('-');
        }
    }
    if result.is_empty() {
        result.push_str("document");
    }
    result
}

struct ResourceResolver {
    base_path: Option<PathBuf>,
    base_url: Option<Url>,
}

impl ResourceResolver {
    fn new(options: &HtmlOptions) -> Self {
        Self {
            base_path: options.base_path.clone(),
            base_url: options.base_url.clone(),
        }
    }

    fn resolve_asset(&self, raw: &str) -> String {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with("data:") || is_absolute_url(trimmed) {
            return trimmed.to_string();
        }
        if let Some(base_url) = &self.base_url {
            if let Ok(joined) = base_url.join(trimmed) {
                if joined.scheme() == "file" {
                    if let Ok(path) = joined.to_file_path() {
                        if path.exists() {
                            return path.to_string_lossy().into_owned();
                        }
                    }
                } else {
                    return joined.to_string();
                }
            }
        }
        let path = Path::new(trimmed);
        if path.is_absolute() {
            if path.exists() {
                return trimmed.to_string();
            }
            let stripped = trimmed.trim_start_matches('/');
            if !stripped.is_empty() {
                if let Some(resolved) = self.resolve_in_base_hierarchy(Path::new(stripped)) {
                    return resolved.to_string_lossy().into_owned();
                }
                return stripped.to_string();
            }
            return trimmed.to_string();
        }
        if let Some(resolved) = self.resolve_in_base_hierarchy(path) {
            return resolved.to_string_lossy().into_owned();
        }
        trimmed.to_string()
    }

    fn resolve_link(&self, raw: &str) -> String {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return String::new();
        }
        if let Ok(parsed) = Url::parse(trimmed) {
            if parsed.scheme() == "file" {
                if let Ok(path) = parsed.to_file_path() {
                    return path.to_string_lossy().into_owned();
                }
            }
            return parsed.to_string();
        }
        if let Some(base_url) = &self.base_url {
            if let Ok(joined) = base_url.join(trimmed) {
                if joined.scheme() == "file" {
                    if let Ok(path) = joined.to_file_path() {
                        return path.to_string_lossy().into_owned();
                    }
                }
                return joined.to_string();
            }
        }
        if let Some(base_path) = &self.base_path {
            let path = Path::new(trimmed);
            let candidate = if path.is_absolute() {
                path.to_path_buf()
            } else {
                base_path.join(path)
            };
            return candidate.to_string_lossy().into_owned();
        }
        trimmed.to_string()
    }

    fn resolve_in_base_hierarchy(&self, relative: &Path) -> Option<PathBuf> {
        let mut current = self.base_path.as_deref();
        while let Some(dir) = current {
            let candidate = dir.join(relative);
            if candidate.exists() {
                return Some(candidate);
            }
            current = dir.parent();
        }
        None
    }
}

fn is_absolute_url(candidate: &str) -> bool {
    matches!(Url::parse(candidate), Ok(url) if url.has_host())
}

#[derive(Clone, Copy)]
struct IdGenerator {
    data_counter: u64,
    view_counter: u64,
    widget_counter: u64,
}

impl IdGenerator {
    fn new() -> Self {
        Self {
            data_counter: 1,
            view_counter: 1,
            widget_counter: 1,
        }
    }

    fn next_data_id(&mut self) -> String {
        let id = encode_base36(self.data_counter);
        self.data_counter += 1;
        id
    }

    fn next_widget_id(&mut self) -> String {
        let id = encode_base36(self.widget_counter);
        self.widget_counter += 1;
        id
    }

    fn next_view_id(&mut self) -> String {
        let id = format!("view{:04}", self.view_counter);
        self.view_counter += 1;
        id
    }
}

fn encode_base36(mut value: u64) -> String {
    const DIGITS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut buf = [b'0'; 8];
    for ch in buf.iter_mut().rev() {
        *ch = DIGITS[(value % 36) as usize];
        value /= 36;
    }
    String::from_utf8(buf.to_vec()).unwrap()
}

#[derive(Clone, Copy)]
enum ElementKind {
    Container,
    Text(Option<SemanticRole>),
    Image,
    Link,
    InputBox,
    TextArea,
    Button,
    Spacer,
    Hidden,
}

#[derive(Clone, Copy)]
enum SemanticRole {
    Heading(u8),
    Paragraph,
    Label,
}

fn classify_element(element: &ElementRef<'_>, style: &ComputedStyle) -> ElementKind {
    if matches!(style.display, Display::None) {
        return ElementKind::Hidden;
    }
    let tag = element.value().name().to_ascii_lowercase();
    match tag.as_str() {
        "img" => ElementKind::Image,
        "a" => {
            // Anchors styled as flex act as containers (cards, nav groups).
            // But inline-block anchors should remain as links (buttons).
            if matches!(style.display, Display::Flex) {
                ElementKind::Container
            } else {
                ElementKind::Link
            }
        }
        "textarea" => ElementKind::TextArea,
        "input" => {
            let input_type = element
                .value()
                .attr("type")
                .map(|value| value.trim().to_ascii_lowercase())
                .unwrap_or_else(|| "text".to_string());
            match input_type.as_str() {
                "button" | "submit" | "reset" => ElementKind::Button,
                "hidden" => ElementKind::Hidden,
                "checkbox" | "radio" => ElementKind::Hidden,
                _ => ElementKind::InputBox,
            }
        }
        "button" => ElementKind::Button,
        "br" => ElementKind::Spacer,
        "h1" => ElementKind::Text(Some(SemanticRole::Heading(1))),
        "h2" => ElementKind::Text(Some(SemanticRole::Heading(2))),
        "h3" => ElementKind::Text(Some(SemanticRole::Heading(3))),
        "h4" => ElementKind::Text(Some(SemanticRole::Heading(4))),
        "h5" => ElementKind::Text(Some(SemanticRole::Heading(5))),
        "h6" => ElementKind::Text(Some(SemanticRole::Heading(6))),
        "label" => ElementKind::Text(Some(SemanticRole::Label)),
        "p" => ElementKind::Text(Some(SemanticRole::Paragraph)),
        "span" | "strong" | "em" | "small" => ElementKind::Text(None),
        _ => {
            if matches!(style.display, Display::Flex) || matches!(style.display, Display::Block) {
                log_fallback_tag_once(&tag, "Container");
                ElementKind::Container
            } else {
                log_fallback_tag_once(&tag, "Text");
                ElementKind::Text(None)
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Display {
    Block,
    Flex,
    Inline,
    None,
}

impl Default for Display {
    fn default() -> Self {
        Display::Block
    }
}
