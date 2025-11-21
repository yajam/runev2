use std::sync::OnceLock;

use anyhow::{Result, anyhow};
use jsonschema::{Draft, JSONSchema};
use serde_json::Value;

use crate::{data::document::DataDocument, view::ViewDocument};

static DATA_SCHEMA: OnceLock<JSONSchema> = OnceLock::new();
static VIEW_SCHEMA: OnceLock<JSONSchema> = OnceLock::new();

fn compile_schema(source: &'static str) -> JSONSchema {
    let schema_value: Value =
        serde_json::from_str(source).expect("embedded schema should parse as JSON");
    JSONSchema::options()
        .with_draft(Draft::Draft202012)
        .compile(&schema_value)
        .expect("embedded schema should compile")
}

fn data_schema() -> &'static JSONSchema {
    DATA_SCHEMA.get_or_init(|| compile_schema(include_str!("../schema/data_document.schema.json")))
}

fn view_schema() -> &'static JSONSchema {
    VIEW_SCHEMA.get_or_init(|| compile_schema(include_str!("../schema/view_document.schema.json")))
}

fn validate_value(schema: &JSONSchema, value: &Value, label: &str) -> Result<()> {
    if let Err(errors) = schema.validate(value) {
        let messages: Vec<String> = errors.into_iter().map(|err| err.to_string()).collect();
        let joined = messages.join("\n");
        return Err(anyhow!("{label} failed schema validation:\n{joined}"));
    }
    Ok(())
}

/// Validates a `serde_json::Value` against the data document schema.
pub fn validate_data_document_value(value: &Value) -> Result<()> {
    validate_value(data_schema(), value, "data document")
}

/// Validates a `serde_json::Value` against the view document schema.
pub fn validate_view_document_value(value: &Value) -> Result<()> {
    validate_value(view_schema(), value, "view document")
}

/// Validates a parsed [`DataDocument`] against the data schema.
pub fn validate_data_document(document: &DataDocument) -> Result<()> {
    let value = serde_json::to_value(document)?;
    validate_data_document_value(&value)
}

/// Validates a parsed [`ViewDocument`] against the view schema.
pub fn validate_view_document(document: &ViewDocument) -> Result<()> {
    let value = serde_json::to_value(document)?;
    validate_view_document_value(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_sample_data_document() {
        let value: Value = serde_json::from_str(include_str!("../home_tab/views/data/home.json"))
            .expect("sample data should parse");
        validate_data_document_value(&value).expect("sample data should satisfy schema");
    }

    #[test]
    fn validates_sample_view_document() {
        let value: Value = serde_json::from_str(include_str!("../home_tab/views/layout/home.vizr"))
            .expect("sample view should parse");
        validate_view_document_value(&value).expect("sample view should satisfy schema");
    }
}
