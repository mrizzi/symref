use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The variable store: maps `$VAR` names to their JSON values.
pub type VarStore = HashMap<String, Value>;

/// A reference entry in the store command output.
#[derive(Debug, Serialize, Deserialize)]
pub struct VarRef {
    pub summary: String,
    #[serde(rename = "ref")]
    pub var_ref: String,
}

/// The output of the store command.
#[derive(Debug, Serialize, Deserialize)]
pub struct StoreOutput {
    pub refs: HashMap<String, VarRef>,
    pub store_path: PathBuf,
}
