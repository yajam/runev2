# Rune Config

Configuration management for Rune Draw runtime environment.

## Overview

This crate provides centralized configuration management for Rune Draw, allowing users to configure the runtime via `rune.toml` instead of environment variables. Environment variables can still be used and will override file-based settings.

## Usage

### Basic Usage

```rust
use rune_config::RuneConfig;

// Load config from rune.toml with environment variable overrides
let config = RuneConfig::load();

// Access configuration values
if let Some(scene) = &config.demo.scene {
    println!("Loading scene: {}", scene);
}
```

### Configuration File

Create a `rune.toml` file in your project root:

```toml
[demo]
scene = "zones"

[text]
font = "/path/to/font.ttf"
use_freetype = false

[rendering]
use_intermediate = true
debug_radial = false

[ir]
use_ir = true
package_path = "examples/sample_first_node"
```

See the example `rune.toml` in the project root for all available options.

### Environment Variables

Environment variables override file-based configuration:

| Environment Variable | Config Field | Description |
|---------------------|--------------|-------------|
| `DEMO_SCENE` | `demo.scene` | Scene to load |
| `DEMO_FONT` | `text.font` | Font file path |
| `DEMO_FREETYPE` | `text.use_freetype` | Use FreeType renderer |
| `USE_INTERMEDIATE` | `rendering.use_intermediate` | Use intermediate texture |
| `DEBUG_RADIAL` | `rendering.debug_radial` | Debug radial backgrounds |
| `USE_IR` | `ir.use_ir` | Use IR rendering mode |
| `RUNE_PACKAGE_PATH` | `ir.package_path` | Path to Rune package |
| `RUNE_DIAGNOSTICS` | `ir.diagnostics` | Enable diagnostics |

## Examples

Run the example to see loaded configuration:

```bash
cargo run -p rune-config --example print_config
```

## Testing

```bash
cargo test -p rune-config
```
