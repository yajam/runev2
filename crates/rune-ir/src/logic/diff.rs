use serde::{Deserialize, Serialize};

/// Minimal IR diff operation set for initial integration.
///
/// External JSON uses an `op` tag in snake_case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum IrDiffOp {
    /// Replace text content for a target. Targets follow the same
    /// temporary rules as `IrMutation::ReplaceText`:
    /// - `widget:InputBox` | `widget:TextArea`
    /// - `widget:DocumentInputBox:<id>` | `widget:DocumentTextArea:<id>`
    /// - `node:element:<index>` (temporary numeric form)
    ReplaceText { target: String, text: String },
    /// Replace text content for a bound data node by its `node_id`.
    /// Only applies to text-bearing nodes (text and label variants).
    ReplaceTextByNodeId { node_id: String, text: String },
}

#[cfg(test)]
mod tests {
    use super::IrDiffOp;

    #[test]
    fn serde_round_trip_replace_text_op() {
        let op = IrDiffOp::ReplaceText {
            target: "node:element:0".to_string(),
            text: "Hi".to_string(),
        };
        let json = serde_json::to_string(&op).expect("serialize op");
        let back: IrDiffOp = serde_json::from_str(&json).expect("deserialize op");
        assert_eq!(op, back);
    }

    #[test]
    fn serde_round_trip_replace_text_by_node_id() {
        let op = IrDiffOp::ReplaceTextByNodeId {
            node_id: "P5Q9LkD2".into(),
            text: "Hello".into(),
        };
        let json = serde_json::to_string(&op).expect("serialize op");
        let back: IrDiffOp = serde_json::from_str(&json).expect("deserialize op");
        assert_eq!(op, back);
    }
}
