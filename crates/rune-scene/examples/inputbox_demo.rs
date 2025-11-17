use engine_core::{Color, Rect};
/// Example demonstrating the InputBox element with rune-text integration.
///
/// This shows:
/// - Cursor blinking animation
/// - Text editing (insert, delete, backspace)
/// - Horizontal scrolling when text exceeds width
/// - Cursor movement (left, right, home, end)
/// - Placeholder text
use rune_scene::elements::input_box::InputBox;
use rune_text::font::FontCache;

fn main() -> anyhow::Result<()> {
    // Load a font using rune-text FontCache
    let mut font_cache = FontCache::new();

    // Try to load a system font (you can also specify a path)
    let font_path = if cfg!(target_os = "macos") {
        "/System/Library/Fonts/SFNS.ttf"
    } else if cfg!(target_os = "windows") {
        "C:\\Windows\\Fonts\\segoeui.ttf"
    } else {
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"
    };

    let font = font_cache.get_or_load(font_path, 0)?;

    // Create an InputBox
    let rect = Rect {
        x: 50.0,
        y: 50.0,
        w: 300.0,
        h: 40.0,
    };

    let mut input_box = InputBox::new(
        rect,
        "Hello, World!".to_string(),
        16.0,
        Color::rgba(240, 240, 240, 255),
        Some("Enter text...".to_string()),
        true, // focused
        &font,
    );

    println!("InputBox created with text: '{}'", input_box.text());

    // Simulate text editing operations
    println!("\n--- Text Editing Demo ---");

    // Move cursor to end
    input_box.move_cursor_to_end();
    println!("Moved cursor to end");

    // Insert a character
    input_box.insert_char('!', &font);
    println!("After inserting '!': '{}'", input_box.text());

    // Delete before cursor (backspace)
    input_box.delete_before_cursor(&font);
    println!("After backspace: '{}'", input_box.text());

    // Move cursor left
    input_box.move_cursor_left();
    input_box.move_cursor_left();
    println!("Moved cursor left twice");

    // Insert text in the middle
    input_box.insert_char('X', &font);
    println!("After inserting 'X': '{}'", input_box.text());

    // Move to start and insert
    input_box.move_cursor_to_start();
    input_box.insert_char('>', &font);
    println!("After inserting '>' at start: '{}'", input_box.text());

    // Demonstrate horizontal scrolling
    println!("\n--- Horizontal Scrolling Demo ---");
    input_box.set_text("This is a very long text that will definitely exceed the width of the input box and trigger horizontal scrolling".to_string(), &font);
    println!("Set long text: '{}'", input_box.text());
    println!("Scroll offset: {}", input_box.scroll_offset());

    input_box.move_cursor_to_end();
    println!(
        "After moving to end, scroll offset: {}",
        input_box.scroll_offset()
    );

    input_box.move_cursor_to_start();
    println!(
        "After moving to start, scroll offset: {}",
        input_box.scroll_offset()
    );

    // Demonstrate cursor blinking
    println!("\n--- Cursor Blinking Demo ---");
    println!("Simulating cursor blink animation:");
    for i in 0..10 {
        input_box.update_blink(0.1); // 100ms per update
        println!(
            "  Frame {}: cursor visible = {}",
            i,
            input_box.is_cursor_visible()
        );
    }

    println!("\n✓ InputBox demo completed successfully!");
    println!("\nTo use InputBox in your application:");
    println!("1. Create a FontCache and load a font");
    println!("2. Create an InputBox with InputBox::new()");
    println!("3. Call update_blink() in your render loop for cursor animation");
    println!("4. Call render() to draw the input box");
    println!("5. Handle keyboard events to call insert_char(), delete_before_cursor(), etc.");

    Ok(())
}
