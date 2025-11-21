use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type NodeId = String;
pub type WidgetKey = String;

/// Semantic data-layer document referenced by view/layout artifacts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataDocument {
    pub document_id: String,
    #[serde(default)]
    pub nodes: Vec<DataNode>,
    #[serde(default)]
    pub bindings: Vec<DataBinding>,
    #[serde(default)]
    pub channels: Vec<DataChannel>,
}

impl DataDocument {
    pub fn node(&self, node_id: &str) -> Option<&DataNode> {
        self.nodes.iter().find(|node| node.node_id == node_id)
    }

    pub fn node_map(&self) -> HashMap<&str, &DataNode> {
        self.nodes
            .iter()
            .map(|node| (node.node_id.as_str(), node))
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataNode {
    pub node_id: NodeId,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub widget_id: Option<WidgetKey>,
    #[serde(flatten)]
    pub kind: DataNodeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DataNodeKind {
    Group {
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    Text(TextNodeData),
    Action(ActionNodeData),
    Image(ImageNodeData),
    /// Tabular data for simple tables. Values are plain strings for now.
    Table(TableNodeData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextNodeData {
    pub text: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_role: Option<TextSemanticRole>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionNodeData {
    pub label: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageNodeData {
    pub source: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Simple table payload: column headers and row values as strings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableNodeData {
    #[serde(default)]
    pub columns: Vec<String>,
    #[serde(default)]
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum TextSemanticRole {
    Heading { level: u8 },
    Paragraph,
    Label,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataBinding {
    pub target: NodeId,
    pub path: String,
    #[serde(default)]
    pub mode: BindingMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingMode {
    Pull,
    Push,
    Duplex,
}

impl Default for BindingMode {
    fn default() -> Self {
        BindingMode::Pull
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataChannel {
    pub channel_id: String,
    #[serde(default)]
    pub kind: ChannelKind,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelKind {
    Broadcast,
    RequestResponse,
}

impl Default for ChannelKind {
    fn default() -> Self {
        ChannelKind::Broadcast
    }
}
