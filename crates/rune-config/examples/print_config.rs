/// Example program to print the loaded configuration
///
/// Run with: cargo run -p rune-config --example print_config

fn main() {
    // Load configuration from rune.toml
    let config = rune_config::RuneConfig::load();

    println!("=== Rune Draw Configuration ===\n");

    println!("Demo Settings:");
    println!("  Scene: {:?}", config.demo.scene);
    println!();

    println!("Text Settings:");
    println!("  Font: {:?}", config.text.font);
    println!("  Use FreeType: {}", config.text.use_freetype);
    println!("  Subpixel Offset: {:?}", config.text.subpixel_offset);
    println!("  Snap X: {}", config.text.snap_x);
    println!("  Text Size: {:?}", config.text.text_size);
    println!("  Line Padding: {:?}", config.text.line_padding);
    println!();

    println!("Rendering Settings:");
    println!("  Use Intermediate: {}", config.rendering.use_intermediate);
    println!("  Debug Radial: {}", config.rendering.debug_radial);
    println!("  Lyon Tolerance: {:?}", config.rendering.lyon_tolerance);
    println!();

    println!("IR Settings:");
    println!("  Use IR: {}", config.ir.use_ir);
    println!("  Package Path: {:?}", config.ir.package_path);
    println!("  Diagnostics: {:?}", config.ir.diagnostics);
    println!("  UA Heading Margins: {}", config.ir.ua_heading_margins);
    println!();

    // Try to serialize to TOML for verification
    match toml::to_string_pretty(&config) {
        Ok(toml_str) => {
            println!("=== Serialized Configuration ===");
            println!("{}", toml_str);
        }
        Err(e) => {
            eprintln!("Failed to serialize config: {}", e);
        }
    }
}
