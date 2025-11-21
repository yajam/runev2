use std::fs;

use anyhow::Result;
use rune_ir::{
    data::document::DataNodeKind,
    html::{HtmlOptions, package_from_html},
    logic::LogicEngine,
    view::ViewNodeKind,
};
use url::Url;

#[test]
fn translates_basic_html_document() -> Result<()> {
    let mut options = HtmlOptions::default();
    options.document_id = Some("basic".to_string());

    let package = package_from_html(
        r#"
        <html>
            <head><title>Example</title></head>
            <body>
                <h1>Hello Rune</h1>
                <p>Rendering pipeline</p>
            </body>
        </html>
        "#,
        options,
    )?;

    let (data, view) = package.entrypoint_documents()?;
    let mut texts = Vec::new();
    for node in &data.nodes {
        if let DataNodeKind::Text(content) = &node.kind {
            texts.push(content.text.clone());
        }
    }

    assert!(
        texts.iter().any(|text| text == "Hello Rune"),
        "expected heading text among data nodes"
    );
    assert!(
        texts.iter().any(|text| text == "Rendering pipeline"),
        "expected paragraph text among data nodes"
    );
    // one root container plus two text nodes.
    assert!(
        view.nodes.len() >= 3,
        "view should contain root container and text nodes"
    );
    Ok(())
}

#[test]
fn extracts_inline_script_into_logic_module() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let base = temp.path().to_path_buf();

    let mut options = HtmlOptions::default();
    options.document_id = Some("with-script".to_string());
    options.base_path = Some(base.clone());

    let package = package_from_html(
        r#"
        <html>
            <body>
                <script>
                    // Tiny logic snippet
                    queueMicrotask(() => rune.core.dispatchMutation('{"type":"noop","from":"test"}'));
                </script>
                <p>Body</p>
            </body>
        </html>
        "#,
        options,
    )?;

    let entry = package
        .manifest
        .entrypoint
        .logic
        .clone()
        .expect("logic entrypoint should be set for inline script");
    let desc = package
        .logic_modules
        .get(&entry)
        .expect("module descriptor should exist");
    assert_eq!(desc.engine, LogicEngine::Js);
    let path = package.base_path().join(&desc.module);
    let source = std::fs::read_to_string(&path)?;
    assert!(
        source.contains("dispatchMutation"),
        "inline script should be written to disk"
    );
    Ok(())
}

#[test]
fn extracts_external_script_into_logic_module() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let root = temp.path();
    let html_dir = root.join("pages");
    std::fs::create_dir_all(&html_dir)?;
    let script_path = html_dir.join("app.js");
    std::fs::write(&script_path, b"globalThis.__rune_test = true;")?;

    let mut options = HtmlOptions::default();
    options.base_path = Some(html_dir.clone());
    let document_path = html_dir.join("index.html");
    std::fs::write(&document_path, b"<!doctype html>")?;
    options.base_url = Some(Url::from_file_path(&document_path).expect("file url"));
    options.document_id = Some("external-script".to_string());

    let package = package_from_html(
        r#"
        <html>
            <body>
                <script src="./app.js"></script>
            </body>
        </html>
        "#,
        options,
    )?;
    let entry = package
        .manifest
        .entrypoint
        .logic
        .clone()
        .expect("logic entrypoint should be set for external script");
    let desc = package
        .logic_modules
        .get(&entry)
        .expect("module descriptor should exist");
    assert_eq!(desc.engine, LogicEngine::Js);
    // Path can be absolute or relative; if relative, join with base.
    let path = if std::path::Path::new(&desc.module).is_absolute() {
        std::path::PathBuf::from(&desc.module)
    } else {
        package.base_path().join(&desc.module)
    };
    assert!(path.exists(), "resolved external script path should exist");
    Ok(())
}

#[test]
fn resolves_remote_image_sources() -> Result<()> {
    let mut options = HtmlOptions::default();
    options.base_url = Some(Url::parse("https://example.com/articles/page.html")?);
    options.document_id = Some("remote".to_string());

    let package = package_from_html(
        r#"
        <html>
            <body>
                <img src="/images/logo.png" alt="Logo" />
            </body>
        </html>
        "#,
        options,
    )?;

    let (data, _) = package.entrypoint_documents()?;
    let image_sources: Vec<_> = data
        .nodes
        .iter()
        .filter_map(|node| match &node.kind {
            DataNodeKind::Image(image) => Some(image.source.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(
        image_sources,
        vec!["https://example.com/images/logo.png".to_string()]
    );

    Ok(())
}

#[test]
fn resolves_local_root_relative_image_sources() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let root = temp.path();
    let html_dir = root.join("pages");
    fs::create_dir_all(&html_dir)?;
    let image_dir = root.join("images");
    fs::create_dir_all(&image_dir)?;
    let image_path = image_dir.join("logo.png");
    fs::write(&image_path, b"")?;

    let mut options = HtmlOptions::default();
    options.base_path = Some(html_dir.clone());
    let document_path = html_dir.join("index.html");
    fs::write(&document_path, b"<!doctype html>")?;
    options.base_url = Some(Url::from_file_path(&document_path).expect("file url"));
    options.document_id = Some("local".to_string());

    let package = package_from_html(
        r#"
        <html>
            <body>
                <img src="/images/logo.png" alt="Logo" />
            </body>
        </html>
        "#,
        options,
    )?;

    let (data, _) = package.entrypoint_documents()?;
    let sources: Vec<_> = data
        .nodes
        .iter()
        .filter_map(|node| match &node.kind {
            DataNodeKind::Image(image) => Some(image.source.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(sources, vec![image_path.to_string_lossy().to_string()]);

    Ok(())
}

#[test]
fn translates_form_controls_with_widget_ids() -> Result<()> {
    let mut options = HtmlOptions::default();
    options.document_id = Some("form-controls".to_string());

    let package = package_from_html(
        r#"
        <html>
            <body>
                <form>
                    <input id="firstName" type="text" placeholder="First name" />
                    <input type="text" placeholder="Last name" />
                    <textarea id="message" placeholder="Message"></textarea>
                </form>
            </body>
        </html>
        "#,
        options,
    )?;

    let (_, view) = package.entrypoint_documents()?;

    let input_nodes: Vec<_> = view
        .nodes
        .iter()
        .filter_map(|node| match &node.kind {
            ViewNodeKind::InputBox(spec) => Some((node, spec)),
            _ => None,
        })
        .collect();

    assert_eq!(input_nodes.len(), 2, "expected two input boxes");
    for (node, spec) in &input_nodes {
        let widget_id = node
            .widget_id
            .as_ref()
            .expect("input nodes should have widget ids");
        assert_eq!(
            widget_id.len(),
            8,
            "widget id should be eight characters long"
        );
        if let Some(control_id) = &spec.control_id {
            assert!(
                !control_id.is_empty(),
                "control id should not be empty when present"
            );
        }
    }

    let (text_area_node, text_area_spec) = view
        .nodes
        .iter()
        .find_map(|node| match &node.kind {
            ViewNodeKind::TextArea(spec) => Some((node, spec)),
            _ => None,
        })
        .expect("expected a textarea node");

    let widget_id = text_area_node
        .widget_id
        .as_ref()
        .expect("textarea should have widget id");
    assert_eq!(
        widget_id.len(),
        8,
        "textarea widget id should be eight characters long"
    );
    assert_eq!(
        text_area_spec.placeholder.as_deref(),
        Some("Message"),
        "textarea placeholder should be preserved"
    );

    Ok(())
}
