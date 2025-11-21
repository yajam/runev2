use super::IrDiffOp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Minimal IR mutation set for Phase 1.
///
/// Serde uses an external tag `type` in snake_case to match inbound JSON from JS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IrMutation {
    /// Replace text for a widget or document node target.
    ///
    /// Accepted target forms (Phase 1):
    /// - `widget:InputBox`
    /// - `widget:TextArea`
    /// - `widget:DocumentInputBox:<id>`
    /// - `widget:DocumentTextArea:<id>`
    /// - `node:element:<index>` (temporary numeric form until stable node_id mapping is wired)
    ///
    /// Reserved (not yet resolved in Phase 1):
    /// - `node:<8-char-node-id>` — planned once data/view node_id mapping is active at runtime.
    ReplaceText { target: String, text: String },
    /// Augment the next pending native form submit by adding key/value pairs.
    /// Expected to be sent after a FormWillSubmit for the same form_id.
    AugmentPendingForm {
        form_id: String,
        extra: HashMap<String, String>,
    },
    /// Prevent the next pending native form submission for a given form_id
    /// (logic will handle submission itself).
    CancelFormSubmit { form_id: String },
    /// Open a runtime overlay (alert, modal, or confirm). Title/description and
    /// button labels are optional and may be provided by IR or logic.
    OpenOverlay {
        kind: String, // "alert" | "modal" | "confirm"
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        left_label: Option<String>,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        right_label: Option<String>,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        width: Option<f64>,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        height: Option<f64>,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        dismissible: Option<bool>,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        show_close: Option<bool>,
    },
    /// Close any currently shown overlay.
    CloseOverlay,
    /// Convenience: host performs HTTP GET, extracts text, and applies a text replace by node_id.
    /// Intended for demos where guest selects the target but delegates fetch + patch to host.
    HttpGetReplaceTextByNodeId {
        url: String,
        node_id: String,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        field: Option<String>,
    },
    /// Apply a batch of IR diff operations.
    IrDiff { ops: Vec<IrDiffOp> },
    /// Generic fetch with a guest callback export name to receive the response.
    /// The host performs the request and then invokes the given WASM export with
    /// a JSON string payload: { url, status, content_type, body }.
    HttpFetch {
        url: String,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        method: Option<String>,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        content_type: Option<String>,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        body: Option<String>,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        headers: Option<HashMap<String, String>>,
        callback: String,
    },
    /// Cancel a previously issued HTTP request by `request_id`.
    /// The request id is allocated on the host and surfaced to the guest via
    /// provider APIs; this mutation allows the guest to drop outstanding work.
    CancelHttpRequest { request_id: u64 },
}

#[cfg(test)]
mod tests {
    use super::IrMutation;

    #[test]
    fn serde_round_trip_replace_text() {
        let m = IrMutation::ReplaceText {
            target: "widget:InputBox".to_string(),
            text: "Hello".to_string(),
        };
        let json = serde_json::to_string(&m).expect("serialize mutation");
        let back: IrMutation = serde_json::from_str(&json).expect("deserialize mutation");
        assert_eq!(m, back);
    }

    #[test]
    fn serde_round_trip_ir_diff() {
        let m = IrMutation::IrDiff {
            ops: vec![super::IrDiffOp::ReplaceText {
                target: "node:element:1".into(),
                text: "World".into(),
            }],
        };
        let json = serde_json::to_string(&m).expect("serialize mutation");
        let back: IrMutation = serde_json::from_str(&json).expect("deserialize mutation");
        assert_eq!(m, back);
    }

    #[test]
    fn serde_round_trip_http_get_replace_text_by_node_id() {
        let m = IrMutation::HttpGetReplaceTextByNodeId {
            url: "http://localhost:3000/api/hello".into(),
            node_id: "P5Q9LkD2".into(),
            field: Some("message".into()),
        };
        let json = serde_json::to_string(&m).expect("serialize mutation");
        let back: IrMutation = serde_json::from_str(&json).expect("deserialize mutation");
        assert_eq!(m, back);
    }
}
