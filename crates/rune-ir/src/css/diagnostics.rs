use std::sync::OnceLock;

pub fn diagnostics_enabled(category: &str) -> bool {
    static SET: OnceLock<std::collections::HashSet<String>> = OnceLock::new();
    let set = SET.get_or_init(|| {
        let raw = std::env::var("RUNE_DIAGNOSTICS").unwrap_or_default();
        raw.split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect::<std::collections::HashSet<String>>()
    });
    set.contains("all") || set.contains(&category.to_ascii_lowercase())
}
