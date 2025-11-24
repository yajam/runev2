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

/// Navigation mode determines the UI chrome behavior.
///
/// This maps to the three navigation modes described in the user flow:
/// - Home: AI-native home screen (Peco), no toolbar
/// - IRApp: IR-native apps, no toolbar
/// - Browser: Web content via CEF, toolbar visible
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavigationMode {
    /// Home tab (Peco) - system hub, AI workspace, no toolbar
    #[default]
    Home,
    /// IR App mode - local IR apps, no toolbar
    IRApp,
    /// Browser mode - web content, toolbar visible
    Browser,
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
    /// Current navigation mode (determines UI chrome)
    pub navigation_mode: NavigationMode,
    /// Whether we can go back
    pub can_go_back: bool,
    /// Whether we can go forward
    pub can_go_forward: bool,
    /// Whether page is currently loading
    pub is_loading: bool,
}

impl Default for RenderTarget {
    fn default() -> Self {
        // Default to IR since the default NavigationMode is Home (IR-based)
        RenderTarget::Ir
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
    let mode = derive_navigation_mode(&normalized, target);
    log::info!("Navigation requested: {} -> {:?} (mode: {:?})", normalized, target, mode);

    // Update state
    if let Ok(mut s) = state().lock() {
        s.current_url = Some(normalized.clone());
        s.render_target = target;
        s.navigation_mode = mode;
        // Only set loading=true for CEF navigation.
        // IR content loads immediately (same frame), so no loading indicator needed.
        s.is_loading = target == RenderTarget::Cef;
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
///
/// IMPORTANT: This function is careful NOT to overwrite the navigation_mode
/// or render_target when we're in IR mode. CEF continues to report its state
/// even when we've navigated away from CEF content, which would cause
/// flickering and incorrect mode switching if we blindly overwrote the state.
pub fn update_state(url: Option<String>, can_back: bool, can_forward: bool, loading: bool) {
    if let Ok(mut s) = state().lock() {
        // Only update URL and render target from CEF if we're currently in CEF/Browser mode.
        // If we've navigated to IR content, ignore CEF's URL updates to avoid overwriting
        // the intentional IR navigation state.
        let current_mode = s.navigation_mode;
        let currently_in_ir_mode = matches!(current_mode, NavigationMode::Home | NavigationMode::IRApp);

        if let Some(ref u) = url {
            // Only update from CEF if:
            // 1. We're not in IR mode, OR
            // 2. The URL itself is an IR URL (meaning CEF is reporting an IR navigation)
            let incoming_target = determine_render_target(u);
            let should_update_url = !currently_in_ir_mode || incoming_target == RenderTarget::Ir;

            if should_update_url {
                s.current_url = Some(u.clone());
                s.render_target = incoming_target;
            }
            // If we're in IR mode and CEF reports a non-IR URL, ignore it
            // This prevents CEF's stale state from overwriting our IR navigation
        }

        // Always update navigation capability flags (back/forward still work)
        s.can_go_back = can_back;
        s.can_go_forward = can_forward;

        // Only update loading state if we're not in IR mode, or if loading is false
        // (to clear the loading state when CEF finishes loading its content)
        if !currently_in_ir_mode || !loading {
            s.is_loading = loading;
        }
    }
}

/// Get the current navigation state.
pub fn get_state() -> NavigationState {
    state().lock().map(|s| NavigationState {
        current_url: s.current_url.clone(),
        current_title: s.current_title.clone(),
        render_target: s.render_target,
        navigation_mode: s.navigation_mode,
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

/// Get the current navigation mode.
pub fn get_navigation_mode() -> NavigationMode {
    state().lock().map(|s| s.navigation_mode).unwrap_or_default()
}

/// Set the navigation mode directly.
pub fn set_navigation_mode(mode: NavigationMode) {
    if let Ok(mut s) = state().lock() {
        s.navigation_mode = mode;
        log::info!("Navigation mode set to: {:?}", mode);
    }
}

/// Derive navigation mode from URL and render target.
/// - `rune://home` → Home mode
/// - `rune://` or `ir://` (non-home) → IRApp mode
/// - Everything else (CEF) → Browser mode
pub fn derive_navigation_mode(url: &str, target: RenderTarget) -> NavigationMode {
    let url_lower = url.to_lowercase();

    // Check for home URL
    if url_lower == "rune://home" || url_lower == "rune://peco" {
        return NavigationMode::Home;
    }

    // IR URLs are IRApp mode
    if target == RenderTarget::Ir {
        return NavigationMode::IRApp;
    }

    // Default to Browser mode for CEF content
    NavigationMode::Browser
}

/// Navigate to the Home tab (Peco).
/// This sets the navigation mode to Home and navigates to rune://home.
pub fn navigate_home() {
    log::info!("Navigation: go home");
    if let Ok(mut s) = state().lock() {
        s.current_url = Some("rune://home".to_string());
        s.render_target = RenderTarget::Ir;
        s.navigation_mode = NavigationMode::Home;
        s.is_loading = false;
    }
    // Queue navigation command for any listeners
    if let Ok(mut q) = queue().lock() {
        q.push_back(NavigationCommand::LoadUrl("rune://home".to_string()));
    }
}

/// Check if toolbar should be visible based on current navigation mode.
/// Toolbar is visible in Browser mode and IRApp mode, but NOT in Home mode.
pub fn should_show_toolbar() -> bool {
    matches!(get_navigation_mode(), NavigationMode::Browser | NavigationMode::IRApp)
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
