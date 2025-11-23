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

#[test]
fn message_label_color_matches_spec() {
    use crate::ir_adapter::IrAdapter;
    use rune_ir::package::RunePackage;
    use std::path::PathBuf;

    // Load the sample form package to validate color parsing end-to-end.
    let package_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/sample_form");
    let package =
        RunePackage::from_directory(&package_dir).expect("failed to load sample_form package");
    let (_data, view) = package
        .entrypoint_documents()
        .expect("failed to read entrypoint documents");

    let view_node = view
        .node("message_label")
        .expect("message_label node missing in view");
    let spec = match &view_node.kind {
        rune_ir::view::ViewNodeKind::Text(spec) => spec,
        other => panic!("expected text node, got {:?}", other),
    };

    // Style color should round-trip to the authored sRGBA bytes.
    let color = IrAdapter::color_from_text_style(&spec.style);
    assert_eq!(color.to_srgba_u8(), [0xf2, 0xf2, 0xf2, 0xff]);
}
