use serde::{Deserialize, Serialize};

pub mod mutation;
pub use mutation::IrMutation;
pub mod diff;
pub use diff::IrDiffOp;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogicEngine {
    Wasm,
    Js,
}

impl Default for LogicEngine {
    fn default() -> Self {
        LogicEngine::Wasm
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogicModuleDescriptor {
    pub module: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Execution engine for this module. Defaults to `wasm` for packaged IR.
    #[serde(default)]
    pub engine: LogicEngine,
}
