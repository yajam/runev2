//! CDP (Chrome DevTools Protocol) backend using chromiumoxide.
//!
//! This provides an alternative to native CEF that's easier to set up,
//! using headless Chrome/Chromium via the DevTools Protocol.

use crate::error::{CefError, Result};
use crate::frame::{FrameBuffer, PixelFormat};
use crate::{HeadlessConfig, HeadlessRenderer, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind};

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::page::{CaptureScreenshotFormat, CaptureScreenshotParams};
use chromiumoxide::page::Page;
use futures::StreamExt;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;

/// CDP-based headless browser renderer.
pub struct CdpHeadless {
    runtime: Runtime,
    browser: Arc<Mutex<Browser>>,
    page: Arc<Mutex<Page>>,
    config: HeadlessConfig,
    loading: bool,
}

impl CdpHeadless {
    /// Create a new CDP-based headless renderer.
    pub async fn new(config: HeadlessConfig) -> Result<Self> {
        let mut browser_config = BrowserConfig::builder()
            .window_size(config.width, config.height)
            .viewport(None); // We handle viewport ourselves

        if config.disable_gpu {
            browser_config = browser_config.arg("--disable-gpu");
        }

        if let Some(ref ua) = config.user_agent {
            browser_config = browser_config.arg(format!("--user-agent={}", ua));
        }

        if !config.javascript_enabled {
            browser_config = browser_config.arg("--disable-javascript");
        }

        let browser_config = browser_config
            .build()
            .map_err(|e| CefError::InitFailed(e.to_string()))?;

        let (browser, mut handler) = Browser::launch(browser_config)
            .await
            .map_err(|e| CefError::InitFailed(e.to_string()))?;

        // Spawn handler in background
        let runtime = Runtime::new().map_err(|e| CefError::InitFailed(e.to_string()))?;
        runtime.spawn(async move {
            while let Some(_) = handler.next().await {}
        });

        let page = browser
            .new_page("about:blank")
            .await
            .map_err(|e| CefError::InitFailed(e.to_string()))?;

        // Set viewport size
        page.set_viewport(chromiumoxide::handler::viewport::Viewport {
            width: config.width,
            height: config.height,
            device_scale_factor: Some(config.scale_factor as f64),
            ..Default::default()
        })
        .await
        .map_err(|e| CefError::InitFailed(e.to_string()))?;

        Ok(Self {
            runtime,
            browser: Arc::new(Mutex::new(browser)),
            page: Arc::new(Mutex::new(page)),
            config,
            loading: false,
        })
    }
}

impl HeadlessRenderer for CdpHeadless {
    fn navigate(&mut self, url: &str) -> Result<()> {
        self.loading = true;
        let page = self.page.clone();
        let url = url.to_string();

        self.runtime.block_on(async {
            let page = page.lock().await;
            page.goto(&url)
                .await
                .map_err(|e| CefError::NavigationFailed(e.to_string()))?;
            Ok::<_, CefError>(())
        })?;

        self.loading = false;
        Ok(())
    }

    fn load_html(&mut self, html: &str, base_url: Option<&str>) -> Result<()> {
        self.loading = true;
        let page = self.page.clone();

        // Use data URL for HTML content
        let data_url = if let Some(base) = base_url {
            format!("data:text/html;base64,{}", base64_encode(html))
        } else {
            format!("data:text/html;charset=utf-8,{}", urlencoding_encode(html))
        };

        self.runtime.block_on(async {
            let page = page.lock().await;
            page.goto(&data_url)
                .await
                .map_err(|e| CefError::NavigationFailed(e.to_string()))?;
            Ok::<_, CefError>(())
        })?;

        self.loading = false;
        Ok(())
    }

    fn capture_frame(&mut self) -> Result<FrameBuffer> {
        let page = self.page.clone();
        let width = self.config.width;
        let height = self.config.height;

        self.runtime.block_on(async {
            let page = page.lock().await;

            // Capture screenshot as PNG
            let screenshot = page
                .execute(CaptureScreenshotParams::builder()
                    .format(CaptureScreenshotFormat::Png)
                    .build())
                .await
                .map_err(|e| CefError::CaptureFailed(e.to_string()))?;

            let png_data = screenshot.result.data;
            let png_bytes = base64_decode(&png_data)?;

            // Decode PNG to raw RGBA
            let img = image::load_from_memory(&png_bytes)
                .map_err(|e| CefError::CaptureFailed(e.to_string()))?;
            let rgba = img.to_rgba8();

            let frame_width = rgba.width();
            let frame_height = rgba.height();

            Ok(FrameBuffer::from_raw(
                rgba.into_raw(),
                frame_width,
                frame_height,
                frame_width * 4,
                PixelFormat::Rgba8,
            ))
        })
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        self.config.width = width;
        self.config.height = height;

        let page = self.page.clone();
        let scale = self.config.scale_factor;

        self.runtime.block_on(async {
            let page = page.lock().await;
            page.set_viewport(chromiumoxide::handler::viewport::Viewport {
                width,
                height,
                device_scale_factor: Some(scale as f64),
                ..Default::default()
            })
            .await
            .map_err(|e| CefError::InvalidState(e.to_string()))?;
            Ok::<_, CefError>(())
        })
    }

    fn execute_js(&mut self, script: &str) -> Result<Option<String>> {
        let page = self.page.clone();
        let script = script.to_string();

        self.runtime.block_on(async {
            let page = page.lock().await;
            let result = page
                .evaluate(script)
                .await
                .map_err(|e| CefError::JsError(e.to_string()))?;

            Ok(result.value().map(|v| v.to_string()))
        })
    }

    fn is_loading(&self) -> bool {
        self.loading
    }

    fn wait_for_load(&mut self, timeout_ms: u64) -> Result<()> {
        let page = self.page.clone();

        self.runtime.block_on(async {
            let page = page.lock().await;
            tokio::time::timeout(
                std::time::Duration::from_millis(timeout_ms),
                page.wait_for_navigation(),
            )
            .await
            .map_err(|_| CefError::Timeout(timeout_ms))?
            .map_err(|e| CefError::NavigationFailed(e.to_string()))?;
            Ok(())
        })
    }

    fn send_mouse_event(&mut self, event: MouseEvent) -> Result<()> {
        let page = self.page.clone();

        self.runtime.block_on(async {
            let page = page.lock().await;

            match event.kind {
                MouseEventKind::Move => {
                    page.move_mouse(event.x as f64, event.y as f64)
                        .await
                        .map_err(|e| CefError::InvalidState(e.to_string()))?;
                }
                MouseEventKind::Down => {
                    let button = match event.button {
                        MouseButton::Left | MouseButton::None => chromiumoxide::cdp::browser_protocol::input::MouseButton::Left,
                        MouseButton::Middle => chromiumoxide::cdp::browser_protocol::input::MouseButton::Middle,
                        MouseButton::Right => chromiumoxide::cdp::browser_protocol::input::MouseButton::Right,
                    };
                    // Note: chromiumoxide click API is limited, this is simplified
                    page.click(chromiumoxide::page::ClickParams::builder()
                        .selector("body")
                        .build()
                        .unwrap())
                        .await
                        .map_err(|e| CefError::InvalidState(e.to_string()))?;
                }
                MouseEventKind::Up => {
                    // Mouse up is implicit in click
                }
                MouseEventKind::Wheel { delta_x, delta_y } => {
                    // Scroll events would need raw CDP commands
                }
            }
            Ok(())
        })
    }

    fn send_key_event(&mut self, event: KeyEvent) -> Result<()> {
        let page = self.page.clone();

        self.runtime.block_on(async {
            let page = page.lock().await;

            if let Some(c) = event.char {
                page.type_str(&c.to_string())
                    .await
                    .map_err(|e| CefError::InvalidState(e.to_string()))?;
            }
            Ok(())
        })
    }

    fn pump_messages(&mut self) {
        // CDP doesn't need explicit message pumping as it runs on a tokio runtime
    }

    fn shutdown(&mut self) -> Result<()> {
        // Browser will be closed when dropped
        Ok(())
    }
}

// Helper functions for encoding
fn base64_encode(s: &str) -> String {
    use std::io::Write;
    let mut enc = base64_encoder();
    enc.write_all(s.as_bytes()).unwrap();
    String::from_utf8(enc.finish()).unwrap()
}

fn base64_encoder() -> base64::write::EncoderStringWriter<base64::engine::GeneralPurpose> {
    use base64::Engine;
    base64::write::EncoderStringWriter::new(&base64::engine::general_purpose::STANDARD)
}

fn base64_decode(s: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|e| CefError::CaptureFailed(format!("base64 decode error: {}", e)))
}

fn urlencoding_encode(s: &str) -> String {
    // Simple percent encoding for data URLs
    s.chars()
        .map(|c| match c {
            ' ' => "%20".to_string(),
            '<' => "%3C".to_string(),
            '>' => "%3E".to_string(),
            '#' => "%23".to_string(),
            '%' => "%25".to_string(),
            '"' => "%22".to_string(),
            '\n' => "%0A".to_string(),
            '\r' => "%0D".to_string(),
            _ if c.is_ascii_alphanumeric() || "-_.~".contains(c) => c.to_string(),
            _ => format!("%{:02X}", c as u32),
        })
        .collect()
}
