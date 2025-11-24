//! Navigation handler for routing URL requests to IR or CEF rendering.
//!
//! This module provides:
//! - URL routing logic to determine if a URL should render via IR or CEF
//! - Navigation command queuing for async FFI communication
//! - Back/forward history tracking
//!
//! The design separates the navigation decision-making (Rust) from the actual
//! CEF browser control (Objective-C), communicating via FFI.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

/// Navigation commands that can be sent to the native CEF browser.
#[derive(Debug, Clone, PartialEq)]
pub enum NavigationCommand {
    /// Navigate to a URL (may be IR or CEF)
    LoadUrl(String),
    /// Go back in browser history
    GoBack,
    /// Go forward in browser history
    GoForward,
    /// Reload the current page
    Reload,
    /// Stop loading
    Stop,
}

/// The target renderer for a URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderTarget {
    /// Render using the IR engine (AI-native)
    Ir,
    /// Render using CEF (web standards)
    Cef,
}

/// Navigation state tracking for the browser.
#[derive(Debug, Default)]
pub struct NavigationState {
    /// Current URL being displayed
    pub current_url: Option<String>,
    /// Current page title
    pub current_title: Option<String>,
    /// Current render target
    pub render_target: RenderTarget,
    /// Whether we can go back
    pub can_go_back: bool,
    /// Whether we can go forward
    pub can_go_forward: bool,
    /// Whether page is currently loading
    pub is_loading: bool,
}

impl Default for RenderTarget {
    fn default() -> Self {
        RenderTarget::Cef
    }
}

/// Global navigation command queue.
/// Commands are pushed by Rust and consumed by the Objective-C side via FFI.
static NAVIGATION_QUEUE: OnceLock<Mutex<VecDeque<NavigationCommand>>> = OnceLock::new();

/// Global navigation state.
static NAVIGATION_STATE: OnceLock<Mutex<NavigationState>> = OnceLock::new();

fn queue() -> &'static Mutex<VecDeque<NavigationCommand>> {
    NAVIGATION_QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn state() -> &'static Mutex<NavigationState> {
    NAVIGATION_STATE.get_or_init(|| Mutex::new(NavigationState::default()))
}

/// Determine if a URL should be rendered via IR or CEF.
///
/// Current heuristics:
/// - `rune://` scheme → IR rendering
/// - `ir://` scheme → IR rendering
/// - `.rune` file extension → IR rendering
/// - Everything else → CEF rendering
///
/// This function can be extended to support more sophisticated routing,
/// such as checking for IR package manifests at the URL.
pub fn determine_render_target(url: &str) -> RenderTarget {
    let url_lower = url.to_lowercase();

    // Check for IR-specific schemes
    if url_lower.starts_with("rune://") || url_lower.starts_with("ir://") {
        return RenderTarget::Ir;
    }

    // Check for IR package file extension
    if url_lower.ends_with(".rune") || url_lower.ends_with("/rune.manifest.json") {
        return RenderTarget::Ir;
    }

    // Check for local file paths that might be IR packages
    if url_lower.starts_with("file://") {
        // Check if it's a directory with RUNE.MANIFEST.json
        // For now, default to CEF for file:// URLs
        // TODO: Add async manifest detection
    }

    // Default to CEF for standard web URLs
    RenderTarget::Cef
}

/// Normalize a URL for navigation.
/// - Adds https:// if no scheme is present
/// - Handles common URL patterns
pub fn normalize_url(input: &str) -> String {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return String::new();
    }

    // Already has a scheme
    if trimmed.contains("://") {
        return trimmed.to_string();
    }

    // Check for IR scheme shortcuts
    if trimmed.starts_with("rune:") {
        return format!("rune://{}", &trimmed[5..]);
    }
    if trimmed.starts_with("ir:") {
        return format!("ir://{}", &trimmed[3..]);
    }

    // Check if it looks like a domain or IP
    let looks_like_url = trimmed.contains('.')
        || trimmed.starts_with("localhost")
        || trimmed.starts_with("127.")
        || trimmed.starts_with("[");

    if looks_like_url {
        format!("https://{}", trimmed)
    } else {
        // Treat as search query - for now just use Google
        format!("https://www.google.com/search?q={}", urlencoding::encode(trimmed))
    }
}

/// Request navigation to a URL.
/// This queues the navigation command for the native side to process.
pub fn navigate_to(url: &str) {
    let normalized = normalize_url(url);
    if normalized.is_empty() {
        return;
    }

    let target = determine_render_target(&normalized);
    log::info!("Navigation requested: {} -> {:?}", normalized, target);

    // Update state
    if let Ok(mut s) = state().lock() {
        s.current_url = Some(normalized.clone());
        s.render_target = target;
        s.is_loading = true;
    }

    // Queue the command
    if let Ok(mut q) = queue().lock() {
        q.push_back(NavigationCommand::LoadUrl(normalized));
    }
}

/// Request navigation back in history.
pub fn go_back() {
    log::info!("Navigation: go back");
    if let Ok(mut q) = queue().lock() {
        q.push_back(NavigationCommand::GoBack);
    }
}

/// Request navigation forward in history.
pub fn go_forward() {
    log::info!("Navigation: go forward");
    if let Ok(mut q) = queue().lock() {
        q.push_back(NavigationCommand::GoForward);
    }
}

/// Request page reload.
pub fn reload() {
    log::info!("Navigation: reload");
    if let Ok(mut q) = queue().lock() {
        q.push_back(NavigationCommand::Reload);
    }
}

/// Request stop loading.
pub fn stop() {
    log::info!("Navigation: stop");
    if let Ok(mut q) = queue().lock() {
        q.push_back(NavigationCommand::Stop);
    }
}

/// Pop the next navigation command from the queue.
/// Called by FFI to get pending commands.
pub fn pop_navigation_command() -> Option<NavigationCommand> {
    queue().lock().ok()?.pop_front()
}

/// Check if there are pending navigation commands.
pub fn has_pending_commands() -> bool {
    queue().lock().map(|q| !q.is_empty()).unwrap_or(false)
}

/// Update navigation state from CEF callbacks.
/// Called when CEF reports navigation state changes.
pub fn update_state(url: Option<String>, can_back: bool, can_forward: bool, loading: bool) {
    if let Ok(mut s) = state().lock() {
        if let Some(ref u) = url {
            s.current_url = Some(u.clone());
            s.render_target = determine_render_target(u);
        }
        s.can_go_back = can_back;
        s.can_go_forward = can_forward;
        s.is_loading = loading;
    }
}

/// Get the current navigation state.
pub fn get_state() -> NavigationState {
    state().lock().map(|s| NavigationState {
        current_url: s.current_url.clone(),
        current_title: s.current_title.clone(),
        render_target: s.render_target,
        can_go_back: s.can_go_back,
        can_go_forward: s.can_go_forward,
        is_loading: s.is_loading,
    }).unwrap_or_default()
}

/// Get the current URL.
pub fn get_current_url() -> Option<String> {
    state().lock().ok()?.current_url.clone()
}

/// Get the current page title.
pub fn get_current_title() -> Option<String> {
    state().lock().ok()?.current_title.clone()
}

/// Update the current page title.
pub fn set_current_title(title: Option<String>) {
    if let Ok(mut s) = state().lock() {
        s.current_title = title;
    }
}

/// Get the current render target.
pub fn get_render_target() -> RenderTarget {
    state().lock().map(|s| s.render_target).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_url() {
        assert_eq!(normalize_url("google.com"), "https://google.com");
        assert_eq!(normalize_url("https://example.com"), "https://example.com");
        assert_eq!(normalize_url("http://example.com"), "http://example.com");
        assert_eq!(normalize_url("localhost:3000"), "https://localhost:3000");
        assert_eq!(normalize_url("rune:app"), "rune://app");
        assert_eq!(normalize_url("ir:scene"), "ir://scene");
    }

    #[test]
    fn test_determine_render_target() {
        assert_eq!(determine_render_target("rune://app"), RenderTarget::Ir);
        assert_eq!(determine_render_target("ir://scene"), RenderTarget::Ir);
        assert_eq!(determine_render_target("https://example.com"), RenderTarget::Cef);
        assert_eq!(determine_render_target("file:///path/to/app.rune"), RenderTarget::Ir);
    }
}
