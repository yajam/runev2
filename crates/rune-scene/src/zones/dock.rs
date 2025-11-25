//! App Dock overlay component.
//!
//! The App Dock provides a unified, system-level way to access apps and active browser tabs.
//! It appears as a floating overlay above all content when triggered.
//!
//! ## Structure
//! - Pinned Apps row (Peco always first)
//! - Active Browser Tabs section
//! - Recent Apps/Sites section
//! - Add Shortcut button
//!
//! ## Interactions
//! - Single click → switch to app/tab
//! - Right-click → context menu (Close, Pin/Unpin)
//! - Drag-and-drop → reorder pinned apps
//! - Dismiss: tap outside, Esc key, Home button, swipe

use super::common::ZoneStyle;
use crate::persistence::{BookmarkEntry, TabEntry};
use engine_core::{Brush, Color, ColorLinPremul, Rect, RoundedRadii, RoundedRect, SvgStyle};

/// App entry for the dock (pinned or recent)
#[derive(Debug, Clone)]
pub struct DockApp {
    /// Unique identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// Icon path (SVG)
    pub icon: Option<String>,
    /// URL or IR path to navigate to
    pub url: String,
    /// Whether this app is pinned
    pub pinned: bool,
}

impl DockApp {
    pub fn new(id: impl Into<String>, name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            icon: None,
            url: url.into(),
            pinned: false,
        }
    }

    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn pinned(mut self) -> Self {
        self.pinned = true;
        self
    }
}

/// Dock visibility state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DockState {
    #[default]
    Hidden,
    Visible,
    Animating,
}

/// App Dock manager
pub struct Dock {
    pub style: ZoneStyle,
    /// Visibility state
    pub state: DockState,
    /// Pinned apps (Peco is always first and cannot be removed)
    pub pinned_apps: Vec<DockApp>,
    /// Recent apps/sites (chronologically sorted, most recent first)
    pub recent_items: Vec<DockApp>,
    /// Animation progress (0.0 = hidden, 1.0 = fully visible)
    pub animation_progress: f32,
}

impl Default for Dock {
    fn default() -> Self {
        Self::new()
    }
}

impl Dock {
    pub fn new() -> Self {
        // Initialize with default pinned apps
        let pinned_apps = vec![
            DockApp::new("peco", "Peco", "rune://home")
                .with_icon("images/layout-grid.svg")
                .pinned(),
            DockApp::new("first-node", "First Node", "rune://sample/first-node")
                .with_icon("images/file.svg")
                .pinned(),
            DockApp::new("webview", "WebView", "rune://sample/webview")
                .with_icon("images/image.svg")
                .pinned(),
            DockApp::new("form", "Form Demo", "rune://sample/form")
                .with_icon("images/file.svg")
                .pinned(),
        ];

        Self {
            style: Self::default_style(),
            state: DockState::Hidden,
            pinned_apps,
            recent_items: Vec::new(),
            animation_progress: 0.0,
        }
    }

    pub fn default_style() -> ZoneStyle {
        ZoneStyle {
            bg_color: ColorLinPremul::from_srgba_u8([28, 32, 48, 245]),
            border_color: ColorLinPremul::from_srgba_u8([70, 75, 95, 255]),
            border_width: 1.0,
        }
    }

    /// Show the dock overlay
    pub fn show(&mut self) {
        self.state = DockState::Visible;
        self.animation_progress = 1.0;
    }

    /// Hide the dock overlay
    pub fn hide(&mut self) {
        self.state = DockState::Hidden;
        self.animation_progress = 0.0;
    }

    /// Toggle dock visibility
    pub fn toggle(&mut self) {
        match self.state {
            DockState::Hidden => self.show(),
            DockState::Visible | DockState::Animating => self.hide(),
        }
    }

    /// Check if dock is visible
    pub fn is_visible(&self) -> bool {
        self.state == DockState::Visible || self.state == DockState::Animating
    }

    /// Update animation progress (call each frame)
    pub fn update_animation(&mut self, delta_time: f32) {
        const ANIMATION_SPEED: f32 = 8.0;

        match self.state {
            DockState::Visible => {
                if self.animation_progress < 1.0 {
                    self.animation_progress =
                        (self.animation_progress + delta_time * ANIMATION_SPEED).min(1.0);
                }
            }
            DockState::Hidden => {
                if self.animation_progress > 0.0 {
                    self.animation_progress =
                        (self.animation_progress - delta_time * ANIMATION_SPEED).max(0.0);
                }
            }
            DockState::Animating => {
                // Animation state is handled by show/hide
            }
        }
    }

    /// Add a recent item (moves to front if already exists)
    pub fn add_recent(&mut self, id: String, name: String, url: String, icon: Option<String>) {
        // Remove if already in recent list
        self.recent_items.retain(|item| item.url != url);

        // Add to front of recent list
        let mut app = DockApp::new(id, name, url);
        if let Some(icon_path) = icon {
            app = app.with_icon(icon_path);
        }
        self.recent_items.insert(0, app);

        // Limit recent items to 10
        const MAX_RECENT: usize = 10;
        if self.recent_items.len() > MAX_RECENT {
            self.recent_items.truncate(MAX_RECENT);
        }
    }

    /// Pin an app (move from recent to pinned)
    pub fn pin_app(&mut self, url: &str) {
        if let Some(pos) = self.recent_items.iter().position(|item| item.url == url) {
            let mut app = self.recent_items.remove(pos);
            app.pinned = true;
            self.pinned_apps.push(app);
        }
    }

    /// Unpin an app (move from pinned to recent)
    /// Note: Peco (index 0) cannot be unpinned
    pub fn unpin_app(&mut self, index: usize) {
        if index == 0 {
            return; // Cannot unpin Peco
        }
        if index < self.pinned_apps.len() {
            let mut app = self.pinned_apps.remove(index);
            app.pinned = false;
            self.recent_items.insert(0, app);
        }
    }

    /// Update dock with current tabs from sidebar
    pub fn sync_from_tabs(&mut self, tabs: &[TabEntry]) {
        // Update recent items from tabs, but don't duplicate pinned apps
        let pinned_urls: Vec<String> = self.pinned_apps.iter().map(|a| a.url.clone()).collect();

        for tab in tabs.iter().rev() {
            if !pinned_urls.contains(&tab.url) {
                self.add_recent(
                    format!("tab_{}", tab.url),
                    tab.title.clone(),
                    tab.url.clone(),
                    None,
                );
            }
        }
    }

    /// Update dock with bookmarks
    pub fn sync_from_bookmarks(&mut self, bookmarks: &[BookmarkEntry]) {
        // Bookmarks can optionally appear in recent items if visited
        // For now, we don't auto-add bookmarks to recent
        let _ = bookmarks;
    }

    /// Render the dock overlay.
    ///
    /// The dock is rendered as a centered floating panel with:
    /// - Scrim (semi-transparent background)
    /// - Dock panel with pinned apps and recent items
    ///
    /// All coordinates are in LOCAL space (0,0 origin) - caller applies transform.
    pub fn render(
        &self,
        canvas: &mut rune_surface::Canvas,
        window_width: f32,
        window_height: f32,
        provider: &dyn engine_core::TextProvider,
    ) {
        if !self.is_visible() {
            return;
        }

        // Layout constants
        const DOCK_WIDTH: f32 = 720.0;
        const DOCK_MIN_HEIGHT: f32 = 320.0;
        const DOCK_MAX_HEIGHT: f32 = 500.0;
        const DOCK_PADDING: f32 = 28.0;
        const APP_ICON_SIZE: f32 = 64.0;
        const APP_GAP: f32 = 24.0;
        const SECTION_GAP: f32 = 32.0;
        const LABEL_HEIGHT: f32 = 24.0;

        // Calculate dock height based on content
        let pinned_row_height = APP_ICON_SIZE + LABEL_HEIGHT + 8.0;
        let recent_row_height = if self.recent_items.is_empty() {
            0.0
        } else {
            APP_ICON_SIZE + LABEL_HEIGHT + 8.0
        };

        let content_height =
            DOCK_PADDING * 2.0 + pinned_row_height + SECTION_GAP + recent_row_height;
        let dock_height = content_height.clamp(DOCK_MIN_HEIGHT, DOCK_MAX_HEIGHT);

        // Center the dock
        let dock_x = (window_width - DOCK_WIDTH) * 0.5;
        let dock_y = (window_height - dock_height) * 0.5;

        // Apply animation (scale from center)
        let scale = self.animation_progress;
        let alpha = (self.animation_progress * 255.0) as u8;

        // 1. Scrim using the same stencil-cutout approach as modal overlays.
        //
        // This draws a fullscreen scrim OVER existing content but cuts out
        // a rounded-rect hole where the dock panel sits so the panel (and
        // its text) are not double-darkened by alpha blending.
        let scrim_color = ColorLinPremul::from_srgba_u8([0, 0, 0, (180.0 * scale) as u8]);

        // Scrim hit region for dismissing
        canvas.hit_region_rect(
            DOCK_SCRIM_REGION_ID,
            Rect {
                x: 0.0,
                y: 0.0,
                w: window_width,
                h: window_height,
            },
            9501,
        );

        // 2. Dock panel background
        let panel_bg = ColorLinPremul::from_srgba_u8([28, 32, 48, alpha]);
        let panel_border = ColorLinPremul::from_srgba_u8([70, 75, 95, alpha]);
        let corner_radius = 16.0;

        // Helper to create a rounded rect
        let make_rrect = |x: f32, y: f32, w: f32, h: f32, r: f32| -> RoundedRect {
            RoundedRect {
                rect: Rect { x, y, w, h },
                radii: RoundedRadii {
                    tl: r,
                    tr: r,
                    br: r,
                    bl: r,
                },
            }
        };

        // Use the modal-style scrim cutout so the dock panel is not dimmed.
        let panel_rrect = make_rrect(dock_x, dock_y, DOCK_WIDTH, dock_height, corner_radius);
        canvas.fill_scrim_with_cutout(panel_rrect, scrim_color);

        // Panel shadow
        let shadow_color = ColorLinPremul::from_srgba_u8([0, 0, 0, (60.0 * scale) as u8]);
        canvas.rounded_rect(
            make_rrect(
                dock_x + 4.0,
                dock_y + 4.0,
                DOCK_WIDTH,
                dock_height,
                corner_radius,
            ),
            Brush::Solid(shadow_color),
            9510,
        );

        // Panel body
        canvas.rounded_rect(
            make_rrect(dock_x, dock_y, DOCK_WIDTH, dock_height, corner_radius),
            Brush::Solid(panel_bg),
            9520,
        );

        // Panel border
        canvas.stroke_rounded_rect(
            make_rrect(dock_x, dock_y, DOCK_WIDTH, dock_height, corner_radius),
            1.0,
            Brush::Solid(panel_border),
            9525,
        );

        // Hit region for dock panel (prevents clicks from going through to scrim)
        canvas.hit_region_rect(
            DOCK_PANEL_REGION_ID,
            Rect {
                x: dock_x,
                y: dock_y,
                w: DOCK_WIDTH,
                h: dock_height,
            },
            9530,
        );

        // 3. Section: Pinned Apps
        let mut y = dock_y + DOCK_PADDING;

        // Section label
        let label_color = ColorLinPremul::from_srgba_u8([180, 185, 200, alpha]);
        canvas.draw_text_run(
            [dock_x + DOCK_PADDING, y + 14.0],
            "Pinned Apps".to_string(),
            13.0,
            label_color,
            9540,
        );
        y += LABEL_HEIGHT + 4.0;

        // Pinned apps row
        let mut x = dock_x + DOCK_PADDING;
        let icon_style = SvgStyle::new()
            .with_stroke(Color::rgba(255, 255, 255, alpha))
            .with_stroke_width(1.5);

        for (idx, app) in self.pinned_apps.iter().enumerate() {
            // App icon background (hover state would be brighter)
            let icon_bg = ColorLinPremul::from_srgba_u8([50, 55, 75, alpha]);
            canvas.rounded_rect(
                make_rrect(x, y, APP_ICON_SIZE, APP_ICON_SIZE, 8.0),
                Brush::Solid(icon_bg),
                9550,
            );

            // Hit region for this app
            let region_id = DOCK_PINNED_APP_REGION_BASE + idx as u32;
            canvas.hit_region_rect(
                region_id,
                Rect {
                    x,
                    y,
                    w: APP_ICON_SIZE,
                    h: APP_ICON_SIZE + LABEL_HEIGHT,
                },
                9555,
            );

            // App icon
            let icon_padding = 12.0;
            let icon_inner_size = APP_ICON_SIZE - icon_padding * 2.0;
            if let Some(ref icon_path) = app.icon {
                canvas.draw_svg_styled(
                    icon_path,
                    [x + icon_padding, y + icon_padding],
                    [icon_inner_size, icon_inner_size],
                    icon_style.clone(),
                    9560,
                );
            } else {
                // Default app icon placeholder
                let text_color = ColorLinPremul::from_srgba_u8([255, 255, 255, alpha]);
                let initial = app
                    .name
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_uppercase()
                    .to_string();
                canvas.draw_text_run(
                    [x + APP_ICON_SIZE * 0.35, y + APP_ICON_SIZE * 0.55],
                    initial,
                    24.0,
                    text_color,
                    9560,
                );
            }

            // App name (truncated)
            let name_color = ColorLinPremul::from_srgba_u8([220, 225, 240, alpha]);
            let display_name: String = app.name.chars().take(10).collect();
            canvas.draw_text_run(
                [x + 4.0, y + APP_ICON_SIZE + 16.0],
                display_name,
                12.0,
                name_color,
                9560,
            );

            x += APP_ICON_SIZE + APP_GAP;

            // Check if we need to wrap
            if x + APP_ICON_SIZE > dock_x + DOCK_WIDTH - DOCK_PADDING {
                x = dock_x + DOCK_PADDING;
                y += APP_ICON_SIZE + LABEL_HEIGHT + 8.0;
            }
        }

        y += APP_ICON_SIZE + LABEL_HEIGHT + SECTION_GAP;

        // 4. Section: Recent Items (if any)
        if !self.recent_items.is_empty() {
            // Section label
            canvas.draw_text_run(
                [dock_x + DOCK_PADDING, y + 14.0],
                "Recent".to_string(),
                13.0,
                label_color,
                9540,
            );
            y += LABEL_HEIGHT + 4.0;

            // Recent items row
            x = dock_x + DOCK_PADDING;

            for (idx, item) in self.recent_items.iter().take(8).enumerate() {
                // Item background
                let item_bg = ColorLinPremul::from_srgba_u8([40, 45, 65, alpha]);
                canvas.rounded_rect(
                    make_rrect(x, y, APP_ICON_SIZE, APP_ICON_SIZE, 8.0),
                    Brush::Solid(item_bg),
                    9550,
                );

                // Hit region for this recent item
                let region_id = DOCK_RECENT_ITEM_REGION_BASE + idx as u32;
                canvas.hit_region_rect(
                    region_id,
                    Rect {
                        x,
                        y,
                        w: APP_ICON_SIZE,
                        h: APP_ICON_SIZE + LABEL_HEIGHT,
                    },
                    9555,
                );

                // Item icon or initial
                let icon_padding = 12.0;
                let icon_inner_size = APP_ICON_SIZE - icon_padding * 2.0;
                if let Some(ref icon_path) = item.icon {
                    canvas.draw_svg_styled(
                        icon_path,
                        [x + icon_padding, y + icon_padding],
                        [icon_inner_size, icon_inner_size],
                        icon_style.clone(),
                        9560,
                    );
                } else {
                    let text_color = ColorLinPremul::from_srgba_u8([200, 205, 220, alpha]);
                    let initial = item
                        .name
                        .chars()
                        .next()
                        .unwrap_or('?')
                        .to_uppercase()
                        .to_string();
                    canvas.draw_text_run(
                        [x + APP_ICON_SIZE * 0.35, y + APP_ICON_SIZE * 0.55],
                        initial,
                        24.0,
                        text_color,
                        9560,
                    );
                }

                // Item name (truncated)
                let name_color = ColorLinPremul::from_srgba_u8([180, 185, 200, alpha]);
                let display_name: String = item.name.chars().take(10).collect();
                canvas.draw_text_run(
                    [x + 4.0, y + APP_ICON_SIZE + 16.0],
                    display_name,
                    12.0,
                    name_color,
                    9560,
                );

                x += APP_ICON_SIZE + APP_GAP;

                // Check if we need to wrap
                if x + APP_ICON_SIZE > dock_x + DOCK_WIDTH - DOCK_PADDING {
                    break; // Don't wrap recent items, just cut off
                }
            }
        }

        let _ = provider; // Silence unused warning for now
    }
}

// Dock hit region IDs
/// Scrim region (clicking dismisses dock)
pub const DOCK_SCRIM_REGION_ID: u32 = 5000;
/// Dock panel region (prevents click-through)
pub const DOCK_PANEL_REGION_ID: u32 = 5001;
/// Base region ID for pinned apps (5100-5199)
pub const DOCK_PINNED_APP_REGION_BASE: u32 = 5100;
/// Base region ID for recent items (5200-5299)
pub const DOCK_RECENT_ITEM_REGION_BASE: u32 = 5200;
