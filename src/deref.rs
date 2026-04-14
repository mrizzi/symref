use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::store::extract_summary;
use crate::types::VarStore;

/// A variable map that warns on stderr for unresolved references
/// and leaves them as-is in the output.
struct LenientVarMap {
    inner: HashMap<String, String>,
}

impl<'a> subst::VariableMap<'a> for LenientVarMap {
    type Value = String;

    fn get(&'a self, key: &str) -> Option<String> {
        match self.inner.get(key) {
            Some(v) => Some(v.clone()),
            None => {
                eprintln!("warning: unresolved variable reference: ${}", key);
                Some(format!("${}", key))
            }
        }
    }
}

/// Build a variable map from the store (keys stripped of `$` prefix,
/// values converted to representative strings).
fn build_var_map(store: &VarStore) -> LenientVarMap {
    let mut inner = HashMap::new();
    for (key, value) in store {
        let name = key.strip_prefix('$').unwrap_or(key);
        inner.insert(name.to_string(), extract_summary(value));
    }
    LenientVarMap { inner }
}

/// Load the variable store from the session directory.
fn load_var_store(session: &Path) -> Result<VarStore> {
    let store_path = session.join("vars.json");

    let store_data = match fs::read_to_string(&store_path) {
        Ok(data) => data,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            anyhow::bail!(
                "vars.json not found in session directory: {}",
                session.display()
            );
        }
        Err(e) => return Err(e).context("failed to read vars.json"),
    };

    serde_json::from_str(&store_data).context("failed to parse vars.json")
}

/// Library API: substitute `$VAR` references in a JSON value.
///
/// Loads `vars.json` from the session directory, builds a variable map,
/// and substitutes all `$VAR` references in string values within the input.
/// Unresolved references are left as-is with a warning on stderr.
pub fn deref(session: &Path, input: &Value) -> Result<Value> {
    let store = load_var_store(session)?;
    let var_map = build_var_map(&store);

    let mut json_value = input.clone();
    subst::json::substitute_string_values(&mut json_value, &var_map)
        .map_err(|e| anyhow::anyhow!("substitution error: {}", e))?;

    Ok(json_value)
}

/// Run the deref command.
pub fn run(session: &Path, input_path: Option<&Path>) -> Result<()> {
    let input_text = match input_path {
        Some(path) => {
            fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
        }
        None => {
            let mut buf = String::new();
            io::stdin()
                .read_to_string(&mut buf)
                .context("failed to read from stdin")?;
            Ok(buf)
        }
    }?;

    // Try JSON first (delegates to library API); fall back to plain text.
    let output = if let Ok(json_value) = serde_json::from_str::<Value>(&input_text) {
        let result = deref(session, &json_value)?;
        serde_json::to_string_pretty(&result).context("failed to serialize output")?
    } else {
        let store = load_var_store(session)?;
        let var_map = build_var_map(&store);
        subst::substitute(&input_text, &var_map)
            .map_err(|e| anyhow::anyhow!("substitution error: {}", e))?
    };

    io::stdout()
        .write_all(output.as_bytes())
        .context("failed to write output")?;
    if !output.ends_with('\n') {
        println!();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn build_var_map_strips_dollar_prefix() {
        let mut store = VarStore::new();
        store.insert("$X7F_REQ_1".into(), json!("OAuth2 login flow"));
        let map = build_var_map(&store);
        assert_eq!(map.inner.get("X7F_REQ_1").unwrap(), "OAuth2 login flow");
    }

    #[test]
    fn build_var_map_extracts_summary_from_object() {
        let mut store = VarStore::new();
        store.insert(
            "$X7F_REQ_1".into(),
            json!({"id": "REQ_1", "summary": "OAuth2 login flow", "priority": "high"}),
        );
        let map = build_var_map(&store);
        assert_eq!(map.inner.get("X7F_REQ_1").unwrap(), "OAuth2 login flow");
    }

    #[test]
    fn build_var_map_number_value() {
        let mut store = VarStore::new();
        store.insert("$X_COUNT".into(), json!(42));
        let map = build_var_map(&store);
        assert_eq!(map.inner.get("X_COUNT").unwrap(), "42");
    }

    #[test]
    fn build_var_map_bool_value() {
        let mut store = VarStore::new();
        store.insert("$X_ACTIVE".into(), json!(true));
        let map = build_var_map(&store);
        assert_eq!(map.inner.get("X_ACTIVE").unwrap(), "true");
    }

    #[test]
    fn build_var_map_null_value() {
        let mut store = VarStore::new();
        store.insert("$X_NOTES".into(), Value::Null);
        let map = build_var_map(&store);
        assert_eq!(map.inner.get("X_NOTES").unwrap(), "null");
    }

    #[test]
    fn build_var_map_key_without_dollar() {
        let mut store = VarStore::new();
        store.insert("NO_DOLLAR".into(), json!("value"));
        let map = build_var_map(&store);
        assert_eq!(map.inner.get("NO_DOLLAR").unwrap(), "value");
    }

    #[test]
    fn lenient_var_map_returns_known_value() {
        let mut inner = HashMap::new();
        inner.insert("FOO".to_string(), "bar".to_string());
        let map = LenientVarMap { inner };
        assert_eq!(
            <LenientVarMap as subst::VariableMap>::get(&map, "FOO").unwrap(),
            "bar"
        );
    }

    #[test]
    fn lenient_var_map_returns_dollar_ref_for_unknown() {
        let map = LenientVarMap {
            inner: HashMap::new(),
        };
        assert_eq!(
            <LenientVarMap as subst::VariableMap>::get(&map, "UNKNOWN").unwrap(),
            "$UNKNOWN"
        );
    }

    #[test]
    fn substitute_plain_text() {
        let mut store = VarStore::new();
        store.insert("$A_B_1".into(), json!("hello"));
        let map = build_var_map(&store);

        let result = subst::substitute("say $A_B_1 world", &map).unwrap();
        assert_eq!(result, "say hello world");
    }

    #[test]
    fn substitute_braces_syntax() {
        let mut store = VarStore::new();
        store.insert("$A_B_1".into(), json!("hello"));
        let map = build_var_map(&store);

        let result = subst::substitute("say ${A_B_1} world", &map).unwrap();
        assert_eq!(result, "say hello world");
    }

    #[test]
    fn substitute_no_recursion() {
        // Value itself contains a $VAR reference — should NOT be expanded further
        let mut store = VarStore::new();
        store.insert("$X_A_1".into(), json!("see $X_B_1"));
        store.insert("$X_B_1".into(), json!("deep"));
        let map = build_var_map(&store);

        let result = subst::substitute("val=$X_A_1", &map).unwrap();
        // Should be "val=see $X_B_1", NOT "val=see deep"
        assert_eq!(result, "val=see $X_B_1");
    }

    #[test]
    fn substitute_multiple_vars_in_one_string() {
        let mut store = VarStore::new();
        store.insert("$P_A_1".into(), json!("alpha"));
        store.insert("$P_A_2".into(), json!("beta"));
        store.insert("$P_A_3".into(), json!("gamma"));
        let map = build_var_map(&store);

        let result = subst::substitute("$P_A_1 $P_A_2 $P_A_3", &map).unwrap();
        assert_eq!(result, "alpha beta gamma");
    }

    #[test]
    fn substitute_adjacent_vars() {
        let mut store = VarStore::new();
        store.insert("$P_A_1".into(), json!("ab"));
        store.insert("$P_A_2".into(), json!("cd"));
        let map = build_var_map(&store);

        let result = subst::substitute("${P_A_1}${P_A_2}", &map).unwrap();
        assert_eq!(result, "abcd");
    }

    #[test]
    fn substitute_var_at_start_and_end() {
        let mut store = VarStore::new();
        store.insert("$V_S_1".into(), json!("start"));
        store.insert("$V_E_1".into(), json!("end"));
        let map = build_var_map(&store);

        let result = subst::substitute("$V_S_1 middle $V_E_1", &map).unwrap();
        assert_eq!(result, "start middle end");
    }

    #[test]
    fn substitute_empty_input() {
        let map = build_var_map(&VarStore::new());
        let result = subst::substitute("", &map).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn substitute_no_vars_in_input() {
        let map = build_var_map(&VarStore::new());
        let result = subst::substitute("no variables here", &map).unwrap();
        assert_eq!(result, "no variables here");
    }

    #[test]
    fn json_substitute_nested_structure() {
        let mut store = VarStore::new();
        store.insert("$X_A_1".into(), json!("resolved"));
        let map = build_var_map(&store);

        let mut value = json!({
            "outer": {
                "inner": "$X_A_1"
            },
            "list": ["$X_A_1", "literal"]
        });

        subst::json::substitute_string_values(&mut value, &map).unwrap();
        assert_eq!(value["outer"]["inner"], "resolved");
        assert_eq!(value["list"][0], "resolved");
        assert_eq!(value["list"][1], "literal");
    }

    #[test]
    fn json_substitute_preserves_non_string_values() {
        let mut store = VarStore::new();
        store.insert("$X_A_1".into(), json!("val"));
        let map = build_var_map(&store);

        let mut value = json!({
            "name": "$X_A_1",
            "count": 42,
            "active": true,
            "nothing": null
        });

        subst::json::substitute_string_values(&mut value, &map).unwrap();
        assert_eq!(value["name"], "val");
        assert_eq!(value["count"], 42);
        assert_eq!(value["active"], true);
        assert!(value["nothing"].is_null());
    }

    #[test]
    fn deref_via_library_api() {
        let dir = tempfile::tempdir().unwrap();

        // Create vars.json with test data
        let mut var_store = VarStore::new();
        var_store.insert("$X7F_REQ_1".into(), json!("OAuth2 login flow"));
        var_store.insert("$X7F_BACKGROUND".into(), json!("Implement auth"));
        let vars_json = serde_json::to_string_pretty(&var_store).unwrap();
        fs::write(dir.path().join("vars.json"), &vars_json).unwrap();

        let input = json!({
            "issueKey": "TC-42",
            "description": "Implementing $X7F_REQ_1 with $X7F_BACKGROUND"
        });

        let result = deref(dir.path(), &input).unwrap();

        assert_eq!(result["issueKey"], "TC-42");
        assert_eq!(
            result["description"],
            "Implementing OAuth2 login flow with Implement auth"
        );
    }
}
