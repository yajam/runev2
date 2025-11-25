//! Chat Panel (Peco) - AI assistant sidebar.
//!
//! The chat panel provides persistent AI assistance throughout the user journey:
//! - In Home mode: Chat is the main content (full featured)
//! - In IRApp/Browser mode: Toggleable sidebar + FAB access
//!
//! Features:
//! - Message history display
//! - Input box for new messages
//! - Context-aware assistance (can reference current page)
//! - Persists across navigation

use super::common::ZoneStyle;
use crate::elements::InputBox;
use engine_core::{Brush, Color, ColorLinPremul, Rect, RoundedRadii, RoundedRect, SvgStyle};

/// A single chat message
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// Message content
    pub content: String,
    /// Whether this is from the user (true) or Peco (false)
    pub is_user: bool,
    /// Timestamp (for display)
    pub timestamp: String,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_user: true,
            timestamp: String::new(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_user: false,
            timestamp: String::new(),
        }
    }
}

/// Chat panel visibility state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChatPanelState {
    #[default]
    Hidden,
    Visible,
}

/// Chat panel component
pub struct ChatPanel {
    pub style: ZoneStyle,
    /// Visibility state
    pub state: ChatPanelState,
    /// Chat message history
    pub messages: Vec<ChatMessage>,
    /// Input box for new messages
    pub input: InputBox,
    /// Scroll offset for message list
    pub scroll_offset: f32,
    /// Whether there are unread messages from Peco
    pub has_unread: bool,
}

impl Default for ChatPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatPanel {
    pub fn new() -> Self {
        // Create input box for chat input
        let input = InputBox::new(
            Rect {
                x: 0.0,
                y: 0.0,
                w: 280.0,
                h: 40.0,
            },
            String::new(),
            14.0,
            ColorLinPremul::from_srgba_u8([255, 255, 255, 255]),
            Some("Ask Peco...".to_string()),
            false,
        );

        // Start with a welcome message from Peco
        let messages = vec![
            ChatMessage::assistant("Hi! I'm Peco, your AI assistant. How can I help you today?"),
        ];

        Self {
            style: Self::default_style(),
            state: ChatPanelState::Hidden,
            messages,
            input,
            scroll_offset: 0.0,
            has_unread: false,
        }
    }

    pub fn default_style() -> ZoneStyle {
        ZoneStyle {
            bg_color: ColorLinPremul::from_srgba_u8([22, 27, 45, 255]),
            border_color: ColorLinPremul::from_srgba_u8([50, 55, 75, 255]),
            border_width: 1.0,
        }
    }

    /// Show the chat panel
    pub fn show(&mut self) {
        self.state = ChatPanelState::Visible;
        self.has_unread = false;
    }

    /// Hide the chat panel
    pub fn hide(&mut self) {
        self.state = ChatPanelState::Hidden;
    }

    /// Toggle chat panel visibility
    pub fn toggle(&mut self) {
        match self.state {
            ChatPanelState::Hidden => self.show(),
            ChatPanelState::Visible => self.hide(),
        }
    }

    /// Check if chat panel is visible
    pub fn is_visible(&self) -> bool {
        self.state == ChatPanelState::Visible
    }

    /// Add a user message
    pub fn add_user_message(&mut self, content: String) {
        self.messages.push(ChatMessage::user(content));
        // TODO: Send to AI backend and get response
        // For now, add a mock response
        self.messages.push(ChatMessage::assistant(
            "I understand. Let me help you with that.",
        ));
    }

    /// Add an assistant message (from Peco)
    pub fn add_assistant_message(&mut self, content: String) {
        self.messages.push(ChatMessage::assistant(content));
        if !self.is_visible() {
            self.has_unread = true;
        }
    }

    /// Get the width of the chat panel
    pub const fn width() -> f32 {
        320.0
    }

    /// Render the chat panel sidebar.
    ///
    /// `panel_rect` defines the area for the chat panel.
    /// Renders in LOCAL coordinates (0,0 origin) - caller applies transform.
    pub fn render(
        &mut self,
        canvas: &mut rune_surface::Canvas,
        panel_rect: Rect,
        provider: &dyn engine_core::TextProvider,
    ) {
        if !self.is_visible() {
            return;
        }

        const PADDING: f32 = 16.0;
        const MESSAGE_GAP: f32 = 12.0;
        const INPUT_HEIGHT: f32 = 44.0;
        const HEADER_HEIGHT: f32 = 48.0;

        let z_base = 8500; // Below toolbar (9000) but above viewport content

        // Background
        canvas.fill_rect(
            0.0,
            0.0,
            panel_rect.w,
            panel_rect.h,
            Brush::Solid(self.style.bg_color),
            z_base,
        );

        // Left border
        canvas.fill_rect(
            0.0,
            0.0,
            self.style.border_width,
            panel_rect.h,
            Brush::Solid(self.style.border_color),
            z_base + 1,
        );

        // Header: "Peco" title
        let header_bg = ColorLinPremul::from_srgba_u8([28, 33, 55, 255]);
        canvas.fill_rect(0.0, 0.0, panel_rect.w, HEADER_HEIGHT, Brush::Solid(header_bg), z_base + 2);

        // Header title
        let title_color = ColorLinPremul::from_srgba_u8([255, 255, 255, 255]);
        canvas.draw_text_run(
            [PADDING, 30.0],
            "Peco".to_string(),
            16.0,
            title_color,
            z_base + 3,
        );

        // Close button hit region
        let close_x = panel_rect.w - PADDING - 24.0;
        canvas.hit_region_rect(
            CHAT_CLOSE_BUTTON_REGION_ID,
            Rect {
                x: close_x,
                y: 12.0,
                w: 24.0,
                h: 24.0,
            },
            z_base + 10,
        );

        // Close button icon
        let icon_style = SvgStyle::new()
            .with_stroke(Color::rgba(180, 185, 200, 255))
            .with_stroke_width(1.5);
        canvas.draw_svg_styled(
            "images/x.svg",
            [close_x, 12.0],
            [24.0, 24.0],
            icon_style,
            z_base + 4,
        );

        // Messages area
        let messages_top = HEADER_HEIGHT + PADDING;
        let messages_height = panel_rect.h - HEADER_HEIGHT - INPUT_HEIGHT - PADDING * 3.0;

        // Render messages (from top down, most recent at bottom region)
        let mut y = messages_top;
        let max_y = messages_top + messages_height;

        for (idx, msg) in self.messages.iter().enumerate() {
            if y > max_y {
                break;
            }

            let bubble_width = (panel_rect.w - PADDING * 2.0 - 40.0).min(250.0);
            let text_lines = (msg.content.len() as f32 / 30.0).ceil().max(1.0);
            let bubble_height = 20.0 + text_lines * 18.0;

            if msg.is_user {
                // User message: right-aligned bubble
                let bubble_x = panel_rect.w - PADDING - bubble_width;
                let bubble_color = ColorLinPremul::from_srgba_u8([59, 130, 246, 255]);
                let text_color = ColorLinPremul::from_srgba_u8([255, 255, 255, 255]);

                let rrect = RoundedRect {
                    rect: Rect {
                        x: bubble_x,
                        y,
                        w: bubble_width,
                        h: bubble_height,
                    },
                    radii: RoundedRadii {
                        tl: 12.0,
                        tr: 12.0,
                        br: 4.0,
                        bl: 12.0,
                    },
                };
                canvas.rounded_rect(rrect, Brush::Solid(bubble_color), z_base + 5);

                // Hit region for message (for future interactions like copy, etc.)
                canvas.hit_region_rect(
                    CHAT_MESSAGE_REGION_BASE + idx as u32,
                    Rect {
                        x: bubble_x,
                        y,
                        w: bubble_width,
                        h: bubble_height,
                    },
                    z_base + 10,
                );

                let display_text: String = msg.content.chars().take(100).collect();
                canvas.draw_text_run(
                    [bubble_x + 12.0, y + 22.0],
                    display_text,
                    13.0,
                    text_color,
                    z_base + 6,
                );
            } else {
                // Assistant message: plain text, left-aligned, no bubble
                let text_color = ColorLinPremul::from_srgba_u8([220, 225, 240, 255]);
                let display_text: String = msg.content.chars().take(200).collect();

                canvas.draw_text_run(
                    [PADDING, y + 18.0],
                    display_text,
                    13.0,
                    text_color,
                    z_base + 6,
                );
            }

            y += bubble_height + MESSAGE_GAP;
        }

        // Input area at bottom
        let input_y = panel_rect.h - INPUT_HEIGHT - PADDING;
        self.input.rect = Rect {
            x: PADDING,
            y: input_y,
            w: panel_rect.w - PADDING * 2.0,
            h: INPUT_HEIGHT,
        };

        // Input hit region
        canvas.hit_region_rect(CHAT_INPUT_REGION_ID, self.input.rect, z_base + 10);

        // Render input box
        self.input.render(canvas, z_base + 5, provider);

        // Send button (inside input area, right side)
        let send_x = panel_rect.w - PADDING - 36.0;
        let send_y = input_y + (INPUT_HEIGHT - 28.0) * 0.5;
        canvas.hit_region_rect(
            CHAT_SEND_BUTTON_REGION_ID,
            Rect {
                x: send_x,
                y: send_y,
                w: 28.0,
                h: 28.0,
            },
            z_base + 15,
        );

        let send_icon_style = SvgStyle::new()
            .with_stroke(Color::rgba(100, 150, 255, 255))
            .with_stroke_width(2.0);
        canvas.draw_svg_styled(
            "images/send.svg",
            [send_x, send_y],
            [28.0, 28.0],
            send_icon_style,
            z_base + 11,
        );
    }
}

/// Render the FAB (floating action button) for chat.
/// This appears in the bottom-right corner when chat panel is hidden.
pub fn render_chat_fab(
    canvas: &mut rune_surface::Canvas,
    viewport_rect: Rect,
    has_unread: bool,
) {
    const FAB_SIZE: f32 = 56.0;
    const FAB_MARGIN: f32 = 24.0;

    let x = viewport_rect.w - FAB_SIZE - FAB_MARGIN;
    let y = viewport_rect.h - FAB_SIZE - FAB_MARGIN;

    let z_base = 9400; // High z-index, below dock but above content

    // FAB background
    let fab_bg = ColorLinPremul::from_srgba_u8([59, 130, 246, 255]);
    let rrect = RoundedRect {
        rect: Rect {
            x,
            y,
            w: FAB_SIZE,
            h: FAB_SIZE,
        },
        radii: RoundedRadii {
            tl: FAB_SIZE / 2.0,
            tr: FAB_SIZE / 2.0,
            br: FAB_SIZE / 2.0,
            bl: FAB_SIZE / 2.0,
        },
    };
    canvas.rounded_rect(rrect, Brush::Solid(fab_bg), z_base);

    // FAB shadow
    let shadow_color = ColorLinPremul::from_srgba_u8([0, 0, 0, 60]);
    let shadow_rrect = RoundedRect {
        rect: Rect {
            x: x + 2.0,
            y: y + 4.0,
            w: FAB_SIZE,
            h: FAB_SIZE,
        },
        radii: RoundedRadii {
            tl: FAB_SIZE / 2.0,
            tr: FAB_SIZE / 2.0,
            br: FAB_SIZE / 2.0,
            bl: FAB_SIZE / 2.0,
        },
    };
    canvas.rounded_rect(shadow_rrect, Brush::Solid(shadow_color), z_base - 1);

    // Hit region
    canvas.hit_region_rect(
        CHAT_FAB_REGION_ID,
        Rect {
            x,
            y,
            w: FAB_SIZE,
            h: FAB_SIZE,
        },
        z_base + 5,
    );

    // Chat icon
    let icon_style = SvgStyle::new()
        .with_stroke(Color::rgba(255, 255, 255, 255))
        .with_stroke_width(2.0);
    canvas.draw_svg_styled(
        "images/message-circle.svg",
        [x + 14.0, y + 14.0],
        [28.0, 28.0],
        icon_style,
        z_base + 1,
    );

    // Unread indicator (red dot)
    if has_unread {
        let dot_color = ColorLinPremul::from_srgba_u8([239, 68, 68, 255]);
        let dot_rrect = RoundedRect {
            rect: Rect {
                x: x + FAB_SIZE - 16.0,
                y: y + 4.0,
                w: 12.0,
                h: 12.0,
            },
            radii: RoundedRadii {
                tl: 6.0,
                tr: 6.0,
                br: 6.0,
                bl: 6.0,
            },
        };
        canvas.rounded_rect(dot_rrect, Brush::Solid(dot_color), z_base + 2);
    }
}

// Hit region IDs for chat panel
pub const CHAT_FAB_REGION_ID: u32 = 6000;
pub const CHAT_CLOSE_BUTTON_REGION_ID: u32 = 6001;
pub const CHAT_INPUT_REGION_ID: u32 = 6002;
pub const CHAT_SEND_BUTTON_REGION_ID: u32 = 6003;
pub const CHAT_MESSAGE_REGION_BASE: u32 = 6100; // 6100-6199 for messages
