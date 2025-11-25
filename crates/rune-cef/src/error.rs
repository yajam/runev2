//! Error types for the CEF renderer.

use thiserror::Error;

/// Result type for CEF operations.
pub type Result<T> = std::result::Result<T, CefError>;

/// Errors that can occur during CEF operations.
#[derive(Error, Debug)]
pub enum CefError {
    /// CEF library failed to load.
    #[error("failed to load CEF library: {0}")]
    LibraryLoad(String),

    /// CEF initialization failed.
    #[error("CEF initialization failed: {0}")]
    InitFailed(String),

    /// Navigation failed.
    #[error("navigation failed: {0}")]
    NavigationFailed(String),

    /// Frame capture failed.
    #[error("frame capture failed: {0}")]
    CaptureFailed(String),

    /// JavaScript execution failed.
    #[error("JavaScript execution failed: {0}")]
    JsError(String),

    /// Timeout waiting for operation.
    #[error("operation timed out after {0}ms")]
    Timeout(u64),

    /// Renderer not initialized.
    #[error("renderer not initialized")]
    NotInitialized,

    /// Renderer already shutdown.
    #[error("renderer has been shutdown")]
    Shutdown,

    /// Invalid state for operation.
    #[error("invalid state: {0}")]
    InvalidState(String),

    /// GPU/wgpu error.
    #[error("GPU error: {0}")]
    GpuError(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Symbol lookup failed.
    #[error("symbol not found: {0}")]
    SymbolNotFound(String),
}
