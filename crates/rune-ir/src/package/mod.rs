use crate::{
    data::document::DataDocument, logic::LogicModuleDescriptor, schema, view::ViewDocument,
};
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

const SAMPLE_MANIFEST_STR: &str = include_str!("../../home_tab/RUNE.MANIFEST.json");
const SAMPLE_TOC_STR: &str = include_str!("../../home_tab/RUNE.TOC.json");
const SAMPLE_DATA_PATH: &str = "views/data/home.json";
const SAMPLE_DATA_STR: &str = include_str!("../../home_tab/views/data/home.json");
const SAMPLE_VIEW_PATH: &str = "views/layout/home.vizr";
const SAMPLE_VIEW_STR: &str = include_str!("../../home_tab/views/layout/home.vizr");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuneManifest {
    pub schema_version: String,
    pub entrypoint: ManifestEntrypoint,
    #[serde(default)]
    pub locales: HashMap<String, LocaleOverrides>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub integrity: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntrypoint {
    pub id: String,
    pub data: String,
    pub view: String,
    #[serde(default)]
    pub page_title: Option<String>,
    #[serde(default)]
    pub logic: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LocaleOverrides {
    #[serde(default)]
    pub data: Option<String>,
    #[serde(default)]
    pub view: Option<String>,
    #[serde(default)]
    pub logic: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableOfContents {
    pub entries: HashMap<String, TocEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TocEntry {
    pub sha256: String,
    pub size: u64,
    pub offset: u64,
}

#[derive(Debug, Clone)]
pub struct RunePackage {
    pub manifest: RuneManifest,
    pub toc: TableOfContents,
    pub data_documents: HashMap<String, DataDocument>,
    pub view_documents: HashMap<String, ViewDocument>,
    pub logic_modules: HashMap<String, LogicModuleDescriptor>,
    base_path: PathBuf,
}

impl RunePackage {
    /// Load a package from a directory containing RUNE.MANIFEST.json and RUNE.TOC.json.
    /// Only the entrypoint data/view documents are required; TOC entries are recomputed
    /// for those files at load time.
    pub fn from_directory(dir: &Path) -> Result<Self> {
        let manifest_path = dir.join("RUNE.MANIFEST.json");
        let toc_path = dir.join("RUNE.TOC.json");

        let manifest_str = std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("failed to read {}", manifest_path.display()))?;
        let manifest: RuneManifest =
            serde_json::from_str(&manifest_str).context("failed to parse RUNE.MANIFEST.json")?;

        // Read the entrypoint data/view JSON files relative to the directory.
        let data_abs = dir.join(&manifest.entrypoint.data);
        let view_abs = dir.join(&manifest.entrypoint.view);

        let data_value: Value = serde_json::from_str(
            &std::fs::read_to_string(&data_abs)
                .with_context(|| format!("failed to read {}", data_abs.display()))?,
        )
        .context("failed to parse data document as JSON")?;
        schema::validate_data_document_value(&data_value)
            .context("data document failed schema validation")?;
        let mut data_document: DataDocument =
            serde_json::from_value(data_value).context("failed to deserialize data document")?;

        let view_value: Value = serde_json::from_str(
            &std::fs::read_to_string(&view_abs)
                .with_context(|| format!("failed to read {}", view_abs.display()))?,
        )
        .context("failed to parse view document as JSON")?;
        schema::validate_view_document_value(&view_value)
            .context("view document failed schema validation")?;
        let mut view_document: ViewDocument =
            serde_json::from_value(view_value).context("failed to deserialize view document")?;

        // Normalize identifiers and validate again post-normalization
        normalize_identifiers(&mut data_document, &mut view_document)?;
        schema::validate_data_document(&data_document)
            .context("normalized data document failed schema validation")?;
        schema::validate_view_document(&view_document)
            .context("normalized view document failed schema validation")?;

        let mut data_documents = HashMap::new();
        data_documents.insert(manifest.entrypoint.data.clone(), data_document);
        let mut view_documents = HashMap::new();
        view_documents.insert(manifest.entrypoint.view.clone(), view_document);

        // Build minimal TOC with recomputed hashes for entrypoint files; ignore offsets.
        let data_bytes = std::fs::read(&data_abs)
            .with_context(|| format!("failed to read {}", data_abs.display()))?;
        let view_bytes = std::fs::read(&view_abs)
            .with_context(|| format!("failed to read {}", view_abs.display()))?;
        let mut entries = HashMap::new();
        entries.insert(
            manifest.entrypoint.data.clone(),
            TocEntry {
                sha256: sha256_hex(&data_bytes),
                size: data_bytes.len() as u64,
                offset: 0,
            },
        );
        entries.insert(
            manifest.entrypoint.view.clone(),
            TocEntry {
                sha256: sha256_hex(&view_bytes),
                size: view_bytes.len() as u64,
                offset: 0,
            },
        );
        // If a TOC file exists, we read it only to keep shape parity; otherwise we use our minimal entries.
        let toc = if toc_path.exists() {
            serde_json::from_str::<TableOfContents>(
                &std::fs::read_to_string(&toc_path)
                    .with_context(|| format!("failed to read {}", toc_path.display()))?,
            )
            .unwrap_or(TableOfContents { entries })
        } else {
            TableOfContents { entries }
        };

        // Populate logic_modules based on manifest logic fields (entry and locales).
        // Convention: if `entrypoint.logic` is a path like "logic/foo.wasm", we insert a
        // descriptor keyed by the same string and use the appropriate engine by extension.
        let mut logic_modules: HashMap<String, crate::logic::LogicModuleDescriptor> =
            HashMap::new();
        if let Some(path) = manifest.entrypoint.logic.clone() {
            let engine = if path.ends_with(".js") {
                crate::logic::LogicEngine::Js
            } else {
                crate::logic::LogicEngine::Wasm
            };
            logic_modules.insert(
                path.clone(),
                crate::logic::LogicModuleDescriptor {
                    module: path,
                    capabilities: Vec::new(),
                    engine,
                },
            );
        }
        for (_locale, overrides) in manifest.locales.iter() {
            if let Some(path) = overrides.logic.clone() {
                let engine = if path.ends_with(".js") {
                    crate::logic::LogicEngine::Js
                } else {
                    crate::logic::LogicEngine::Wasm
                };
                logic_modules
                    .entry(path.clone())
                    .or_insert(crate::logic::LogicModuleDescriptor {
                        module: path,
                        capabilities: Vec::new(),
                        engine,
                    });
            }
        }

        Ok(Self {
            manifest,
            toc,
            data_documents,
            view_documents,
            logic_modules,
            base_path: dir.to_path_buf(),
        })
    }
    pub fn sample() -> Result<Self> {
        let manifest: RuneManifest =
            serde_json::from_str(SAMPLE_MANIFEST_STR).context("failed to parse sample manifest")?;
        let toc: TableOfContents =
            serde_json::from_str(SAMPLE_TOC_STR).context("failed to parse sample TOC")?;

        if manifest.entrypoint.data != SAMPLE_DATA_PATH {
            return Err(anyhow!(
                "sample manifest entrypoint references unexpected data document '{}'",
                manifest.entrypoint.data
            ));
        }
        if manifest.entrypoint.view != SAMPLE_VIEW_PATH {
            return Err(anyhow!(
                "sample manifest entrypoint references unexpected view document '{}'",
                manifest.entrypoint.view
            ));
        }

        let data_value: Value = serde_json::from_str(SAMPLE_DATA_STR)
            .context("failed to parse sample data document as JSON")?;
        schema::validate_data_document_value(&data_value)
            .context("sample data document failed schema validation")?;
        let mut data_document: DataDocument = serde_json::from_value(data_value)
            .context("failed to deserialize sample data document")?;

        let view_value: Value = serde_json::from_str(SAMPLE_VIEW_STR)
            .context("failed to parse sample view document as JSON")?;
        schema::validate_view_document_value(&view_value)
            .context("sample view document failed schema validation")?;
        let mut view_document: ViewDocument = serde_json::from_value(view_value)
            .context("failed to deserialize sample view document")?;
        normalize_identifiers(&mut data_document, &mut view_document)?;
        schema::validate_data_document(&data_document)
            .context("normalized data document failed schema validation")?;
        schema::validate_view_document(&view_document)
            .context("normalized view document failed schema validation")?;

        let mut data_documents = HashMap::new();
        data_documents.insert(SAMPLE_DATA_PATH.to_string(), data_document);

        let mut view_documents = HashMap::new();
        view_documents.insert(SAMPLE_VIEW_PATH.to_string(), view_document);

        Ok(Self {
            manifest,
            toc,
            data_documents,
            view_documents,
            logic_modules: HashMap::new(),
            base_path: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        })
    }

    pub fn from_documents(
        entrypoint_id: impl Into<String>,
        mut data_document: DataDocument,
        mut view_document: ViewDocument,
        base_path: PathBuf,
    ) -> Result<Self> {
        schema::validate_data_document(&data_document)
            .context("data document did not pass schema validation")?;
        schema::validate_view_document(&view_document)
            .context("view document did not pass schema validation")?;
        normalize_identifiers(&mut data_document, &mut view_document)?;
        schema::validate_data_document(&data_document)
            .context("normalized data document did not pass schema validation")?;
        schema::validate_view_document(&view_document)
            .context("normalized view document did not pass schema validation")?;

        let entrypoint_id = sanitize_entrypoint_id(&entrypoint_id.into());
        let data_path = format!("views/data/{}.json", entrypoint_id);
        let view_path = format!("views/layout/{}.vizr", entrypoint_id);

        let data_bytes = serde_json::to_vec(&data_document)
            .context("failed to serialize data document to JSON")?;
        let view_bytes = serde_json::to_vec(&view_document)
            .context("failed to serialize view document to JSON")?;

        let mut data_documents = HashMap::new();
        data_documents.insert(data_path.clone(), data_document);

        let mut view_documents = HashMap::new();
        view_documents.insert(view_path.clone(), view_document);

        let manifest = RuneManifest {
            schema_version: "1.0.0".to_string(),
            entrypoint: ManifestEntrypoint {
                id: entrypoint_id.clone(),
                data: data_path.clone(),
                view: view_path.clone(),
                page_title: None,
                logic: None,
            },
            locales: HashMap::new(),
            capabilities: Vec::new(),
            integrity: None,
        };

        let mut toc_entries = HashMap::new();
        toc_entries.insert(
            data_path,
            TocEntry {
                sha256: sha256_hex(&data_bytes),
                size: data_bytes.len() as u64,
                offset: 0,
            },
        );
        toc_entries.insert(
            view_path,
            TocEntry {
                sha256: sha256_hex(&view_bytes),
                size: view_bytes.len() as u64,
                offset: 0,
            },
        );

        Ok(Self {
            manifest,
            toc: TableOfContents {
                entries: toc_entries,
            },
            data_documents,
            view_documents,
            logic_modules: HashMap::new(),
            base_path,
        })
    }

    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    pub fn entrypoint_documents(&self) -> Result<(&DataDocument, &ViewDocument)> {
        let data = self
            .data_documents
            .get(self.manifest.entrypoint.data.as_str())
            .with_context(|| {
                format!("missing data document '{}'", self.manifest.entrypoint.data)
            })?;
        let view = self
            .view_documents
            .get(self.manifest.entrypoint.view.as_str())
            .with_context(|| {
                format!("missing view document '{}'", self.manifest.entrypoint.view)
            })?;
        Ok((data, view))
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex::encode(digest)
}

fn sanitize_entrypoint_id(value: &str) -> String {
    let mut sanitized = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            sanitized.push(ch);
        } else if ch.is_whitespace() {
            sanitized.push('-');
        }
    }
    if sanitized.is_empty() {
        "document".to_string()
    } else {
        sanitized
    }
}

fn normalize_identifiers(data: &mut DataDocument, view: &mut ViewDocument) -> Result<()> {
    fn derive_alnum_id(seed: &str) -> String {
        // Deterministically derive an 8-char [A-Za-z0-9] ID from a seed
        let hex = sha256_hex(seed.as_bytes());
        hex.chars().take(8).collect()
    }
    fn is_valid_id(value: &str) -> bool {
        value.len() == 8 && value.chars().all(|ch| ch.is_ascii_alphanumeric())
    }

    let mut id_map: HashMap<String, String> = HashMap::new();
    let mut widget_map: HashMap<String, String> = HashMap::new();

    for node in &mut data.nodes {
        let old_id = node.node_id.clone();
        let mut candidate_id = old_id.clone();
        if !is_valid_id(&candidate_id) {
            candidate_id = derive_alnum_id(&old_id);
        }
        let final_id = candidate_id.clone();
        node.node_id = final_id.clone();
        id_map.insert(old_id, final_id.clone());
        id_map.insert(final_id.clone(), final_id.clone());

        let widget_id = node
            .widget_id
            .take()
            .filter(|value| is_valid_id(value))
            .unwrap_or_else(|| derive_alnum_id(&final_id));
        node.widget_id = Some(widget_id.clone());
        widget_map.insert(final_id, widget_id);
    }

    for binding in &mut data.bindings {
        let target = binding.target.clone();
        let Some(new_id) = id_map.get(&target) else {
            return Err(anyhow!(
                "binding references unknown data node '{}'",
                binding.target
            ));
        };
        binding.target = new_id.clone();
    }

    for node in &mut view.nodes {
        if let Some(reference) = node.node_id.clone() {
            let Some(new_id) = id_map.get(&reference) else {
                return Err(anyhow!(
                    "view node '{}' references unknown data node '{}'",
                    node.id,
                    reference
                ));
            };
            node.node_id = Some(new_id.clone());
            if let Some(widget_id) = widget_map.get(new_id) {
                node.widget_id = Some(widget_id.clone());
            }
        }
    }

    Ok(())
}
