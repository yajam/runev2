use super::*;
use crate::ir_renderer::style::parse_color;
use engine_core::ColorLinPremul;

#[test]
fn test_parse_hex_color() {
    assert_eq!(
        parse_color("#ff0000"),
        Some(ColorLinPremul::from_srgba_u8([255, 0, 0, 255]))
    );

    assert_eq!(
        parse_color("#00ff00"),
        Some(ColorLinPremul::from_srgba_u8([0, 255, 0, 255]))
    );

    assert_eq!(
        parse_color("#0000ff80"),
        Some(ColorLinPremul::from_srgba_u8([0, 0, 255, 128]))
    );
}

#[test]
fn test_parse_color_invalid() {
    assert_eq!(parse_color("invalid"), None);
    assert_eq!(parse_color("#ff"), None);
    assert_eq!(parse_color("#gggggg"), None);
}
