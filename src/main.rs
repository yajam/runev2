use anyhow::Result;

fn main() -> Result<()> {
    // Very small shim/dispatcher. For now, just run the rune-scene demo.
    // Future: parse CLI/env to choose different runners.
    rune_scene::run()
}
