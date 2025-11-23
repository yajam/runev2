use anyhow::Result;
use rune_ir::package::RunePackage;
use std::path::PathBuf;

#[test]
fn loads_sample_first_node_package() -> Result<()> {
    let package_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/sample_first_node");

    assert!(
        package_dir.exists(),
        "sample_first_node package dir missing at {:?}",
        package_dir
    );

    let package = RunePackage::from_directory(&package_dir)?;
    let (data, view) = package.entrypoint_documents()?;

    assert_eq!(data.document_id, "welcome-first-data");
    assert_eq!(view.view_id, "welcome_simple");
    assert_eq!(view.root, "root_view");
    assert_eq!(package.manifest.entrypoint.id, "welcome_first");
    assert_eq!(
        package.manifest.entrypoint.view,
        "views/layout/welcome.vizr"
    );
    assert_eq!(package.manifest.entrypoint.data, "views/data/welcome.json");
    assert_eq!(
        package.manifest.entrypoint.logic.as_deref(),
        Some("logic/hello.wasm")
    );
    assert!(
        package.logic_modules.contains_key("logic/hello.wasm"),
        "logic entry should be registered"
    );

    Ok(())
}

#[test]
fn loads_sample_form_package() -> Result<()> {
    let package_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/sample_form");

    assert!(
        package_dir.exists(),
        "sample_form package dir missing at {:?}",
        package_dir
    );

    let package = RunePackage::from_directory(&package_dir)?;
    let (data, view) = package.entrypoint_documents()?;

    assert_eq!(data.document_id, "sample-form-data");
    assert_eq!(view.view_id, "sample_form");
    assert_eq!(view.root, "root");
    assert_eq!(package.manifest.entrypoint.id, "sample_form");
    assert_eq!(
        package.manifest.entrypoint.view,
        "views/layout/form.vizr"
    );
    assert_eq!(package.manifest.entrypoint.data, "views/data/form.json");
    assert_eq!(
        package.manifest.entrypoint.logic.as_deref(),
        Some("logic/sample_form.wasm")
    );
    assert!(
        package
            .logic_modules
            .contains_key("logic/sample_form.wasm"),
        "logic entry should be registered"
    );

    Ok(())
}
