#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{fs, io, path::PathBuf};
use tracing::warn;

const APP_HOME_DIR: &str = ".rune";
const WINDOW_STATE_FILE: &str = "window_state.json";
const BOOKMARKS_FILE: &str = "bookmarks.json";
const TABS_FILE: &str = "tabs.json";
const HISTORY_FILE: &str = "history.json";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct WindowSize {
    pub width: f64,
    pub height: f64,
}

impl WindowSize {
    pub fn is_valid(&self) -> bool {
        self.width > 0.0 && self.height > 0.0
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowPreferences {
    pub size: Option<WindowSize>,
    pub position: Option<WindowPosition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximized: Option<bool>,
}

pub struct WindowStateStore {
    path: PathBuf,
    state: WindowPreferences,
    dirty: bool,
}

impl WindowStateStore {
    pub fn load() -> Result<Self> {
        let path = storage_path()?;
        let state = match fs::read(&path) {
            Ok(data) => match serde_json::from_slice::<WindowPreferences>(&data) {
                Ok(parsed) => parsed,
                Err(error) => {
                    warn!(?error, ?path, "failed to parse persisted window state");
                    WindowPreferences::default()
                }
            },
            Err(error) => {
                if error.kind() != io::ErrorKind::NotFound {
                    warn!(?error, ?path, "failed to read persisted window state");
                }
                // Ensure the .rune folder and an initial file exist on first run
                let default_state = WindowPreferences::default();
                if let Err(err) = write_state(&path, &default_state) {
                    warn!(?err, ?path, "failed to create initial window state file");
                }
                default_state
            }
        };

        Ok(Self {
            path,
            state,
            dirty: false,
        })
    }

    pub fn window_size(&self) -> Option<WindowSize> {
        self.state.size
    }

    pub fn window_position(&self) -> Option<WindowPosition> {
        self.state.position
    }

    pub fn window_maximized(&self) -> Option<bool> {
        self.state.maximized
    }

    pub fn update_size(&mut self, width: f64, height: f64) {
        if !(width.is_finite() && height.is_finite()) {
            return;
        }

        let size = WindowSize { width, height };
        if !size.is_valid() {
            return;
        }

        if self.state.size != Some(size) {
            self.state.size = Some(size);
            self.dirty = true;
        }
    }

    pub fn update_position(&mut self, x: i32, y: i32) {
        let position = WindowPosition { x, y };
        if self.state.position != Some(position) {
            self.state.position = Some(position);
            self.dirty = true;
        }
    }

    pub fn update_maximized(&mut self, maximized: bool) {
        if self.state.maximized != Some(maximized) {
            self.state.maximized = Some(maximized);
            self.dirty = true;
        }
    }

    pub fn save(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }

        write_state(&self.path, &self.state)?;
        self.dirty = false;
        Ok(())
    }
}

impl Drop for WindowStateStore {
    fn drop(&mut self) {
        if self.dirty
            && let Err(error) = write_state(&self.path, &self.state)
        {
            warn!(?error, ?self.path, "failed to persist window state during drop");
        }
    }
}

fn storage_path() -> Result<PathBuf> {
    if let Some(mut home) = dirs::home_dir() {
        home.push(APP_HOME_DIR);
        home.push(WINDOW_STATE_FILE);
        Ok(home)
    } else {
        let mut cwd = std::env::current_dir()?;
        cwd.push(WINDOW_STATE_FILE);
        Ok(cwd)
    }
}

fn write_state(path: &PathBuf, state: &WindowPreferences) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(state)?;
    fs::write(path, json)?;
    Ok(())
}

// ------------------------------
// Bookmarks persistence
// ------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BookmarkEntry {
    pub title: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BookmarksDocument {
    pub bookmarks: Vec<BookmarkEntry>,
}

pub struct BookmarksStore {
    path: PathBuf,
    state: BookmarksDocument,
    dirty: bool,
}

impl BookmarksStore {
    pub fn load() -> Result<Self> {
        let path = bookmarks_storage_path()?;
        let state = match fs::read(&path) {
            Ok(data) => match serde_json::from_slice::<BookmarksDocument>(&data) {
                Ok(parsed) => parsed,
                Err(error) => {
                    warn!(
                        ?error,
                        ?path,
                        "failed to parse bookmarks file; starting fresh"
                    );
                    BookmarksDocument::default()
                }
            },
            Err(error) => {
                if error.kind() != io::ErrorKind::NotFound {
                    warn!(?error, ?path, "failed to read bookmarks file");
                }
                // Ensure the .rune folder and an initial file exist on first run
                let default_state = BookmarksDocument::default();
                if let Err(err) = write_bookmarks(&path, &default_state) {
                    warn!(?err, ?path, "failed to create initial bookmarks file");
                }
                default_state
            }
        };

        Ok(Self {
            path,
            state,
            dirty: false,
        })
    }

    pub fn list(&self) -> &[BookmarkEntry] {
        &self.state.bookmarks
    }

    pub fn add(&mut self, title: String, url: String, folder: Option<String>) {
        self.state
            .bookmarks
            .push(BookmarkEntry { title, url, folder });
        self.dirty = true;
    }

    pub fn save(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        write_bookmarks(&self.path, &self.state)?;
        self.dirty = false;
        Ok(())
    }

    pub fn remove_at(&mut self, index: usize) -> bool {
        if index < self.state.bookmarks.len() {
            self.state.bookmarks.remove(index);
            self.dirty = true;
            true
        } else {
            false
        }
    }
}

impl Drop for BookmarksStore {
    fn drop(&mut self) {
        if self.dirty
            && let Err(error) = write_bookmarks(&self.path, &self.state)
        {
            warn!(?error, ?self.path, "failed to persist bookmarks during drop");
        }
    }
}

fn bookmarks_storage_path() -> Result<PathBuf> {
    if let Some(mut home) = dirs::home_dir() {
        home.push(APP_HOME_DIR);
        fs::create_dir_all(&home)?;
        home.push(BOOKMARKS_FILE);
        Ok(home)
    } else {
        let mut cwd = std::env::current_dir()?;
        cwd.push(BOOKMARKS_FILE);
        Ok(cwd)
    }
}

fn write_bookmarks(path: &PathBuf, doc: &BookmarksDocument) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(doc)?;
    fs::write(path, json)?;
    Ok(())
}

// ------------------------------
// Tabs persistence
// ------------------------------

/// A single history entry within a tab
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TabEntry {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TabsDocument {
    pub tabs: Vec<TabEntry>,
    /// Index of the currently active tab (None if no tabs)
    #[serde(default)]
    pub active_tab: Option<usize>,
}

impl Default for TabsDocument {
    fn default() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab: None,
        }
    }
}

pub struct TabsStore {
    path: PathBuf,
    state: TabsDocument,
    dirty: bool,
}

impl TabsStore {
    pub fn load() -> Result<Self> {
        let path = tabs_storage_path()?;
        let state = match fs::read(&path) {
            Ok(data) => match serde_json::from_slice::<TabsDocument>(&data) {
                Ok(parsed) => parsed,
                Err(error) => {
                    warn!(?error, ?path, "failed to parse tabs file; starting fresh");
                    TabsDocument::default()
                }
            },
            Err(error) => {
                if error.kind() != io::ErrorKind::NotFound {
                    warn!(?error, ?path, "failed to read tabs file");
                }
                let default_state = TabsDocument::default();
                if let Err(err) = write_tabs(&path, &default_state) {
                    warn!(?err, ?path, "failed to create initial tabs file");
                }
                default_state
            }
        };

        Ok(Self {
            path,
            state,
            dirty: false,
        })
    }

    pub fn list(&self) -> &[TabEntry] {
        &self.state.tabs
    }

    pub fn add(&mut self, title: String, url: String) {
        // De-duplicate by URL: move to end (most-recent) and refresh title
        if let Some(pos) = self.state.tabs.iter().position(|t| t.url == url) {
            self.state.tabs.remove(pos);
            // Adjust active_tab if needed
            if let Some(active) = self.state.active_tab {
                if pos < active {
                    self.state.active_tab = Some(active - 1);
                } else if pos == active {
                    self.state.active_tab = None;
                }
            }
        }
        self.state.tabs.push(TabEntry { title, url });
        self.dirty = true;
    }

    /// Get the currently active tab index
    pub fn active_tab(&self) -> Option<usize> {
        self.state.active_tab
    }

    /// Set the active tab index
    pub fn set_active_tab(&mut self, index: Option<usize>) {
        if index != self.state.active_tab {
            self.state.active_tab = index;
            self.dirty = true;
        }
    }

    /// Create a new tab and make it active. Returns the new tab's index.
    pub fn new_tab(&mut self, title: String, url: String) -> usize {
        self.state.tabs.push(TabEntry { title, url });
        let index = self.state.tabs.len() - 1;
        self.state.active_tab = Some(index);
        self.dirty = true;
        index
    }

    /// Update the active tab's URL and title.
    /// Returns true if updated.
    pub fn update_active_tab(&mut self, title: String, url: String) -> bool {
        if let Some(index) = self.state.active_tab {
            if index < self.state.tabs.len() {
                let tab = &mut self.state.tabs[index];
                let url_changed = tab.url != url;
                let title_changed = tab.title != title;

                if url_changed || title_changed {
                    tab.title = title;
                    tab.url = url;
                    self.dirty = true;
                    return true;
                }
            }
        }
        false
    }

    /// Get the active tab entry
    pub fn get_active_tab(&self) -> Option<&TabEntry> {
        self.state.active_tab.and_then(|i| self.state.tabs.get(i))
    }

    pub fn save(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        write_tabs(&self.path, &self.state)?;
        self.dirty = false;
        Ok(())
    }

    /// Reload tabs from disk if the file changed. Returns true if state updated.
    pub fn reload(&mut self) -> bool {
        match fs::read(&self.path) {
            Ok(data) => match serde_json::from_slice::<TabsDocument>(&data) {
                Ok(parsed) => {
                    if parsed != self.state {
                        self.state = parsed;
                        // Reset dirty; reflects on-disk state now
                        self.dirty = false;
                        true
                    } else {
                        false
                    }
                }
                Err(_) => false,
            },
            Err(_) => false,
        }
    }

    pub fn remove_at(&mut self, index: usize) -> bool {
        if index < self.state.tabs.len() {
            self.state.tabs.remove(index);
            // Adjust active_tab if needed
            if let Some(active) = self.state.active_tab {
                if index < active {
                    self.state.active_tab = Some(active - 1);
                } else if index == active {
                    // Removed the active tab - select previous or next
                    if self.state.tabs.is_empty() {
                        self.state.active_tab = None;
                    } else if active >= self.state.tabs.len() {
                        self.state.active_tab = Some(self.state.tabs.len() - 1);
                    }
                    // else keep same index (now points to next tab)
                }
            }
            self.dirty = true;
            true
        } else {
            false
        }
    }
}

impl Drop for TabsStore {
    fn drop(&mut self) {
        if self.dirty
            && let Err(error) = write_tabs(&self.path, &self.state)
        {
            warn!(?error, ?self.path, "failed to persist tabs during drop");
        }
    }
}

fn tabs_storage_path() -> Result<PathBuf> {
    if let Some(mut home) = dirs::home_dir() {
        home.push(APP_HOME_DIR);
        fs::create_dir_all(&home)?;
        home.push(TABS_FILE);
        Ok(home)
    } else {
        let mut cwd = std::env::current_dir()?;
        cwd.push(TABS_FILE);
        Ok(cwd)
    }
}

fn write_tabs(path: &PathBuf, doc: &TabsDocument) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(doc)?;
    fs::write(path, json)?;
    Ok(())
}
