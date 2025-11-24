use super::common::ZoneStyle;
use crate::persistence::{BookmarkEntry, BookmarksStore, TabEntry, TabsStore};
use engine_core::{ColorLinPremul, Rect};
use tracing::warn;

// Layout constants (shared by render + hit testing)
const SECTION_HEADER_HEIGHT: f32 = 32.0;
const ITEM_HEIGHT: f32 = 42.0;
const PADDING_X: f32 = 16.0;
const PADDING_Y: f32 = 16.0;
const ICON_SIZE: f32 = 18.0;
const TEXT_SIZE: f32 = 13.0;
const HEADER_SIZE: f32 = 11.0;
const CLOSE_BUTTON_SIZE: f32 = 18.0;
const CLOSE_HIT_PADDING: f32 = 8.0;
const ICON_TEXT_GAP: f32 = 12.0;
const SECTION_GAP: f32 = 20.0;
const TEXT_BASELINE_OFFSET: f32 = TEXT_SIZE * 0.35;
const HEADER_BASELINE_OFFSET: f32 = HEADER_SIZE * 0.35;

/// Region ID base for bookmark items (0-99 reserved for bookmarks)
pub const SIDEBAR_BOOKMARK_REGION_BASE: u32 = 2000;
/// Region ID base for tab items (100-199 reserved for tabs)
pub const SIDEBAR_TAB_REGION_BASE: u32 = 2100;
/// Region ID for "Add Bookmark" button
pub const SIDEBAR_ADD_BOOKMARK_REGION_ID: u32 = 2200;
/// Region ID for tab close buttons base (one per tab)
pub const SIDEBAR_TAB_CLOSE_REGION_BASE: u32 = 2300;
/// Region ID for bookmark delete buttons base (one per bookmark)
pub const SIDEBAR_BOOKMARK_DELETE_REGION_BASE: u32 = 2400;

/// Maximum number of items per section
const MAX_SIDEBAR_ITEMS: usize = 50;

/// Sidebar configuration and state
pub struct Sidebar {
    pub style: ZoneStyle,
    pub visible: bool,
    pub bookmarks_store: BookmarksStore,
    pub tabs_store: TabsStore,
    /// Scroll offset for bookmarks section
    pub bookmarks_scroll: f32,
    /// Scroll offset for tabs section
    pub tabs_scroll: f32,
}

/// Logical hit targets for sidebar interactions.
pub enum SidebarHit {
    Tab(usize),
    TabClose(usize),
    Bookmark(usize),
    AddBookmark,
}

impl Sidebar {
    pub fn new() -> Self {
        let bookmarks_store = BookmarksStore::load().unwrap_or_else(|e| {
            eprintln!("Failed to load bookmarks: {}", e);
            // Return a default store - this shouldn't fail in practice
            BookmarksStore::load().expect("BookmarksStore::load should not fail twice")
        });
        let tabs_store = TabsStore::load().unwrap_or_else(|e| {
            eprintln!("Failed to load tabs: {}", e);
            TabsStore::load().expect("TabsStore::load should not fail twice")
        });

        // Start with sidebar closed by default
        Self {
            style: Self::default_style(),
            visible: false,
            bookmarks_store,
            tabs_store,
            bookmarks_scroll: 0.0,
            tabs_scroll: 0.0,
        }
    }

    pub fn default_style() -> ZoneStyle {
        ZoneStyle {
            bg_color: ColorLinPremul::from_srgba_u8([22, 27, 47, 255]),
            border_color: ColorLinPremul::from_srgba_u8([60, 65, 85, 255]),
            border_width: 1.0,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self) {
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Add a bookmark to the store
    pub fn add_bookmark(&mut self, title: String, url: String) {
        self.bookmarks_store.add(title, url, None);
        let _ = self.bookmarks_store.save();
    }

    /// Add a tab to the store
    pub fn add_tab(&mut self, title: String, url: String) {
        self.tabs_store.add(title, url);
        let _ = self.tabs_store.save();
    }

    /// Create a new tab and make it active
    pub fn new_tab(&mut self, title: String, url: String) -> usize {
        let index = self.tabs_store.new_tab(title, url);
        let _ = self.tabs_store.save();
        index
    }

    /// Get the active tab index
    pub fn active_tab(&self) -> Option<usize> {
        self.tabs_store.active_tab()
    }

    /// Set the active tab index
    pub fn set_active_tab(&mut self, index: Option<usize>) {
        self.tabs_store.set_active_tab(index);
        let _ = self.tabs_store.save();
    }

    /// Update the active tab's URL and title
    pub fn update_active_tab(&mut self, title: String, url: String) -> bool {
        let updated = self.tabs_store.update_active_tab(title, url);
        if updated {
            let _ = self.tabs_store.save();
        }
        updated
    }

    /// Remove a tab at the given index
    pub fn remove_tab(&mut self, index: usize) -> bool {
        let removed = self.tabs_store.remove_at(index);
        if removed {
            if let Err(error) = self.tabs_store.save() {
                warn!(?error, "failed to persist tabs after removal");
            }
        }
        removed
    }

    /// Remove a bookmark at the given index
    pub fn remove_bookmark(&mut self, index: usize) -> bool {
        let removed = self.bookmarks_store.remove_at(index);
        if removed {
            if let Err(error) = self.bookmarks_store.save() {
                warn!(?error, "failed to persist bookmarks after removal");
            }
        }
        removed
    }

    /// Get bookmark at index (for navigation on click)
    pub fn get_bookmark(&self, index: usize) -> Option<&BookmarkEntry> {
        self.bookmarks_store.list().get(index)
    }

    /// Get tab at index (for navigation on click)
    pub fn get_tab(&self, index: usize) -> Option<&TabEntry> {
        self.tabs_store.list().get(index)
    }

    /// Render sidebar content with bookmarks and tabs sections.
    ///
    /// Renders in LOCAL coordinates (0,0 origin) - caller applies transform.
    pub fn render(
        &self,
        canvas: &mut rune_surface::Canvas,
        sidebar_rect: Rect,
        _provider: &dyn engine_core::TextProvider,
    ) {
        // Colors
        let header_color = ColorLinPremul::from_srgba_u8([100, 110, 130, 255]);
        let item_color = ColorLinPremul::from_srgba_u8([200, 205, 215, 255]);
        let icon_color = engine_core::Color::rgba(150, 160, 180, 255);
        let close_color = engine_core::Color::rgba(100, 110, 130, 255);
        let add_color = engine_core::Color::rgba(100, 180, 100, 255);

        let icon_style = engine_core::SvgStyle::new()
            .with_stroke(icon_color)
            .with_stroke_width(1.5);
        let close_style = engine_core::SvgStyle::new()
            .with_stroke(close_color)
            .with_stroke_width(1.5);
        let add_style = engine_core::SvgStyle::new()
            .with_stroke(add_color)
            .with_stroke_width(2.0);

        let mut y = PADDING_Y;

        // ── Tabs Section ──
        // Section header
        let tabs_header_center = y + SECTION_HEADER_HEIGHT * 0.5;
        canvas.draw_text_run(
            [PADDING_X, tabs_header_center + HEADER_BASELINE_OFFSET],
            "TABS".to_string(),
            HEADER_SIZE,
            header_color,
            8500,
        );
        y += SECTION_HEADER_HEIGHT;

        let tabs = self.tabs_store.list();
        if tabs.is_empty() {
            // Empty state
            canvas.draw_text_run(
                [PADDING_X, y + ITEM_HEIGHT * 0.5 + TEXT_BASELINE_OFFSET],
                "No open tabs".to_string(),
                TEXT_SIZE,
                ColorLinPremul::from_srgba_u8([80, 90, 110, 255]),
                8500,
            );
            y += ITEM_HEIGHT;
        } else {
            for (i, tab) in tabs.iter().take(MAX_SIDEBAR_ITEMS).enumerate() {
                let item_y = y + (i as f32) * ITEM_HEIGHT;

                // Calculate vertical center of the row
                let row_center_y = item_y + ITEM_HEIGHT / 2.0;

                // Close button hit region FIRST (higher z so it takes priority)
                let close_x = sidebar_rect.w - PADDING_X - CLOSE_BUTTON_SIZE;
                let close_y = row_center_y - CLOSE_BUTTON_SIZE / 2.0;
                let close_rect = Rect {
                    x: close_x - CLOSE_HIT_PADDING,
                    y: close_y - CLOSE_HIT_PADDING,
                    w: CLOSE_BUTTON_SIZE + CLOSE_HIT_PADDING * 2.0,
                    h: CLOSE_BUTTON_SIZE + CLOSE_HIT_PADDING * 2.0,
                };
                let close_region_id = SIDEBAR_TAB_CLOSE_REGION_BASE + i as u32;
                canvas.hit_region_rect(close_region_id, close_rect, 8600);

                // Hit region for the tab item (full row, but lower z than close button)
                let item_rect = Rect {
                    x: 0.0,
                    y: item_y,
                    w: sidebar_rect.w,
                    h: ITEM_HEIGHT,
                };
                let region_id = SIDEBAR_TAB_REGION_BASE + i as u32;
                canvas.hit_region_rect(region_id, item_rect, 8550);

                // Tab icon - vertically centered
                let icon_y = row_center_y - ICON_SIZE / 2.0;
                canvas.draw_svg_styled(
                    "images/file.svg",
                    [PADDING_X, icon_y],
                    [ICON_SIZE, ICON_SIZE],
                    icon_style.clone(),
                    8500,
                );

                // Tab title - vertically centered with icon
                let text_x = PADDING_X + ICON_SIZE + ICON_TEXT_GAP;
                let max_title_width =
                    sidebar_rect.w - text_x - PADDING_X - CLOSE_BUTTON_SIZE - CLOSE_HIT_PADDING * 2.0;
                let title = truncate_text(&tab.title, max_title_width, TEXT_SIZE);
                let text_y = row_center_y + TEXT_BASELINE_OFFSET;
                canvas.draw_text_run(
                    [text_x, text_y],
                    title,
                    TEXT_SIZE,
                    item_color,
                    8500,
                );

                // Close button icon
                canvas.draw_svg_styled(
                    "images/x.svg",
                    [close_x, close_y],
                    [CLOSE_BUTTON_SIZE, CLOSE_BUTTON_SIZE],
                    close_style.clone(),
                    8510,
                );
            }
            y += (tabs.len().min(MAX_SIDEBAR_ITEMS) as f32) * ITEM_HEIGHT;
        }

        y += SECTION_GAP;

        // ── Bookmarks Section ──
        // Section header with add button
        let bookmarks_header_center = y + SECTION_HEADER_HEIGHT * 0.5;
        canvas.draw_text_run(
            [PADDING_X, bookmarks_header_center + HEADER_BASELINE_OFFSET],
            "BOOKMARKS".to_string(),
            HEADER_SIZE,
            header_color,
            8500,
        );

        // Add bookmark button (+ icon)
        let add_btn_size = 18.0;
        let add_btn_x = sidebar_rect.w - PADDING_X - add_btn_size;
        let add_btn_y = bookmarks_header_center - add_btn_size * 0.5;
        let add_btn_rect = Rect {
            x: add_btn_x - 6.0,
            y: add_btn_y - 6.0,
            w: add_btn_size + 12.0,
            h: add_btn_size + 12.0,
        };
        canvas.hit_region_rect(SIDEBAR_ADD_BOOKMARK_REGION_ID, add_btn_rect, 8550);
        canvas.draw_svg_styled(
            "images/plus.svg",
            [add_btn_x, add_btn_y],
            [add_btn_size, add_btn_size],
            add_style,
            8500,
        );

        y += SECTION_HEADER_HEIGHT;

        let bookmarks = self.bookmarks_store.list();
        if bookmarks.is_empty() {
            // Empty state
            canvas.draw_text_run(
                [PADDING_X, y + ITEM_HEIGHT * 0.5 + TEXT_BASELINE_OFFSET],
                "No bookmarks yet".to_string(),
                TEXT_SIZE,
                ColorLinPremul::from_srgba_u8([80, 90, 110, 255]),
                8500,
            );
        } else {
            for (i, bookmark) in bookmarks.iter().take(MAX_SIDEBAR_ITEMS).enumerate() {
                let item_y = y + (i as f32) * ITEM_HEIGHT;

                // Calculate vertical center of the row
                let row_center_y = item_y + ITEM_HEIGHT / 2.0;

                // Delete button hit region FIRST (higher z so it takes priority)
                let delete_x = sidebar_rect.w - PADDING_X - CLOSE_BUTTON_SIZE;
                let delete_y = row_center_y - CLOSE_BUTTON_SIZE / 2.0;
                let delete_rect = Rect {
                    x: delete_x - CLOSE_HIT_PADDING,
                    y: delete_y - CLOSE_HIT_PADDING,
                    w: CLOSE_BUTTON_SIZE + CLOSE_HIT_PADDING * 2.0,
                    h: CLOSE_BUTTON_SIZE + CLOSE_HIT_PADDING * 2.0,
                };
                let delete_region_id = SIDEBAR_BOOKMARK_DELETE_REGION_BASE + i as u32;
                canvas.hit_region_rect(delete_region_id, delete_rect, 8600);

                // Hit region for the bookmark item (full row, but lower z than delete button)
                let item_rect = Rect {
                    x: 0.0,
                    y: item_y,
                    w: sidebar_rect.w,
                    h: ITEM_HEIGHT,
                };
                let region_id = SIDEBAR_BOOKMARK_REGION_BASE + i as u32;
                canvas.hit_region_rect(region_id, item_rect, 8550);

                // Bookmark icon - vertically centered
                let icon_y = row_center_y - ICON_SIZE / 2.0;
                canvas.draw_svg_styled(
                    "images/bookmark.svg",
                    [PADDING_X, icon_y],
                    [ICON_SIZE, ICON_SIZE],
                    icon_style.clone(),
                    8500,
                );

                // Bookmark title - vertically centered with icon
                let text_x = PADDING_X + ICON_SIZE + ICON_TEXT_GAP;
                let max_title_width =
                    sidebar_rect.w - text_x - PADDING_X - CLOSE_BUTTON_SIZE - CLOSE_HIT_PADDING * 2.0;
                let title = truncate_text(&bookmark.title, max_title_width, TEXT_SIZE);
                let text_y = row_center_y + TEXT_BASELINE_OFFSET;
                canvas.draw_text_run(
                    [text_x, text_y],
                    title,
                    TEXT_SIZE,
                    item_color,
                    8500,
                );

                // Delete button icon
                canvas.draw_svg_styled(
                    "images/x.svg",
                    [delete_x, delete_y],
                    [CLOSE_BUTTON_SIZE, CLOSE_BUTTON_SIZE],
                    close_style.clone(),
                    8510,
                );
            }
        }
    }

    /// Hit test a sidebar-local point and return the logical target.
    pub fn hit_test(&self, local_x: f32, local_y: f32, sidebar_width: f32) -> Option<SidebarHit> {
        if local_x < 0.0 || local_y < 0.0 || local_x > sidebar_width {
            return None;
        }

        let mut y = PADDING_Y;

        // Skip past tabs header
        let tabs_header_bottom = y + SECTION_HEADER_HEIGHT;
        if local_y < tabs_header_bottom {
            // Inside header area, but no actionable hit
        }
        y = tabs_header_bottom;

        // Tabs list
        let tabs = self.tabs_store.list();
        if tabs.is_empty() {
            y += ITEM_HEIGHT;
        } else {
            for (i, _tab) in tabs.iter().take(MAX_SIDEBAR_ITEMS).enumerate() {
                let item_y = y + (i as f32) * ITEM_HEIGHT;
                let row_center_y = item_y + ITEM_HEIGHT / 2.0;

                // Close button rect
                let close_x = sidebar_width - PADDING_X - CLOSE_BUTTON_SIZE;
                let close_y = row_center_y - CLOSE_BUTTON_SIZE / 2.0;
                let close_rect = Rect {
                    x: close_x - CLOSE_HIT_PADDING,
                    y: close_y - CLOSE_HIT_PADDING,
                    w: CLOSE_BUTTON_SIZE + CLOSE_HIT_PADDING * 2.0,
                    h: CLOSE_BUTTON_SIZE + CLOSE_HIT_PADDING * 2.0,
                };
                if point_in_rect(local_x, local_y, close_rect) {
                    return Some(SidebarHit::TabClose(i));
                }

                // Full row rect
                let item_rect = Rect {
                    x: 0.0,
                    y: item_y,
                    w: sidebar_width,
                    h: ITEM_HEIGHT,
                };
                if point_in_rect(local_x, local_y, item_rect) {
                    return Some(SidebarHit::Tab(i));
                }
            }
            y += (tabs.len().min(MAX_SIDEBAR_ITEMS) as f32) * ITEM_HEIGHT;
        }

        // Gap + bookmarks header
        y += SECTION_GAP;
        let bookmarks_header_bottom = y + SECTION_HEADER_HEIGHT;

        // Add bookmark button
        let add_btn_size = 18.0;
        let add_btn_x = sidebar_width - PADDING_X - add_btn_size;
        let add_btn_y = y + SECTION_HEADER_HEIGHT * 0.5 - add_btn_size * 0.5;
        let add_btn_rect = Rect {
            x: add_btn_x - 6.0,
            y: add_btn_y - 6.0,
            w: add_btn_size + 12.0,
            h: add_btn_size + 12.0,
        };
        if point_in_rect(local_x, local_y, add_btn_rect) {
            return Some(SidebarHit::AddBookmark);
        }

        y = bookmarks_header_bottom;

        // Bookmarks list
        let bookmarks = self.bookmarks_store.list();
        if bookmarks.is_empty() {
            // No rows
        } else {
            for (i, _bookmark) in bookmarks.iter().take(MAX_SIDEBAR_ITEMS).enumerate() {
                let item_y = y + (i as f32) * ITEM_HEIGHT;
                let item_rect = Rect {
                    x: 0.0,
                    y: item_y,
                    w: sidebar_width,
                    h: ITEM_HEIGHT,
                };
                if point_in_rect(local_x, local_y, item_rect) {
                    return Some(SidebarHit::Bookmark(i));
                }
            }
        }

        None
    }
}

/// Truncate text to fit within max_width (rough estimate based on char count)
fn truncate_text(text: &str, max_width: f32, font_size: f32) -> String {
    // Rough estimate: average char width is ~0.5 * font_size
    let avg_char_width = font_size * 0.55;
    let max_chars = (max_width / avg_char_width) as usize;

    if text.len() <= max_chars {
        text.to_string()
    } else if max_chars > 3 {
        format!("{}...", &text[..max_chars - 3])
    } else {
        text[..max_chars.min(text.len())].to_string()
    }
}

impl Default for Sidebar {
    fn default() -> Self {
        Self::new()
    }
}

fn point_in_rect(x: f32, y: f32, rect: Rect) -> bool {
    x >= rect.x && x <= rect.x + rect.w && y >= rect.y && y <= rect.y + rect.h
}
