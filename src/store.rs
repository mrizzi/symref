use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::{Map, Value};

use crate::naming::{array_var_name, scalar_var_name};
use crate::types::{StoreOutput, VarRef, VarStore};

/// Extract a representative summary string from a JSON value.
pub fn extract_summary(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Object(map) => {
            for key in &["summary", "description", "text", "value"] {
                if let Some(Value::String(s)) = map.get(*key) {
                    return s.clone();
                }
            }
            // Fallback: first string field
            for v in map.values() {
                if let Value::String(s) = v {
                    return s.clone();
                }
            }
            serde_json::to_string(value).unwrap_or_default()
        }
        other => other.to_string(),
    }
}

/// Process input JSON and assign variable references.
fn assign_refs(prefix: &str, input: &Map<String, Value>) -> (VarStore, HashMap<String, VarRef>) {
    let mut store = VarStore::new();
    let mut refs = HashMap::new();

    for (field, value) in input {
        match value {
            Value::Array(items) => {
                for (i, item) in items.iter().enumerate() {
                    let var_name = array_var_name(prefix, field, i + 1);
                    let summary = extract_summary(item);
                    refs.insert(
                        var_name.clone(),
                        VarRef {
                            summary,
                            var_ref: var_name.clone(),
                        },
                    );
                    store.insert(var_name, item.clone());
                }
            }
            _ => {
                let var_name = scalar_var_name(prefix, field);
                let summary = extract_summary(value);
                refs.insert(
                    var_name.clone(),
                    VarRef {
                        summary,
                        var_ref: var_name.clone(),
                    },
                );
                store.insert(var_name, value.clone());
            }
        }
    }

    (store, refs)
}

/// Load an existing var store from disk, or return an empty one.
fn load_store(path: &Path) -> Result<VarStore> {
    if path.exists() {
        let data = fs::read_to_string(path).context("failed to read vars.json")?;
        let store: VarStore = serde_json::from_str(&data).context("failed to parse vars.json")?;
        Ok(store)
    } else {
        Ok(VarStore::new())
    }
}

/// Library API: assign symbolic references and persist to session store.
pub fn store(session: &Path, prefix: &str, input: &Map<String, Value>) -> Result<StoreOutput> {
    if !session.exists() {
        anyhow::bail!("session directory does not exist: {}", session.display());
    }

    let (new_entries, refs) = assign_refs(prefix, input);

    let store_path = session.join("vars.json");
    let mut var_store = load_store(&store_path)?;
    var_store.extend(new_entries);

    let store_json =
        serde_json::to_string_pretty(&var_store).context("failed to serialize var store")?;
    fs::write(&store_path, store_json).context("failed to write vars.json")?;

    Ok(StoreOutput { refs, store_path })
}

/// CLI entry point: reads input from stdin/file, calls store(), prints result.
pub fn run(session: &Path, prefix: &str, input_path: Option<&Path>) -> Result<()> {
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

    let input: Map<String, Value> =
        serde_json::from_str(&input_text).context("input is not a valid JSON object")?;

    let output = store(session, prefix, &input)?;

    let output_json =
        serde_json::to_string_pretty(&output).context("failed to serialize output")?;
    println!("{}", output_json);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_summary_from_string() {
        assert_eq!(extract_summary(&json!("hello")), "hello");
    }

    #[test]
    fn extract_summary_prefers_summary_field() {
        let val = json!({"id": "REQ_1", "summary": "OAuth2 login flow", "priority": "high"});
        assert_eq!(extract_summary(&val), "OAuth2 login flow");
    }

    #[test]
    fn extract_summary_falls_back_to_description() {
        let val = json!({"id": "AC_1", "description": "Users can authenticate"});
        assert_eq!(extract_summary(&val), "Users can authenticate");
    }

    #[test]
    fn extract_summary_number() {
        assert_eq!(extract_summary(&json!(42)), "42");
    }

    #[test]
    fn assign_refs_array_field() {
        let input: Map<String, Value> = serde_json::from_value(json!({
            "requirements": [
                {"id": "REQ_1", "summary": "OAuth2 login flow"},
                {"id": "REQ_2", "summary": "Session persistence"}
            ]
        }))
        .unwrap();

        let (store, refs) = assign_refs("X7F", &input);

        assert!(store.contains_key("$X7F_REQ_1"));
        assert!(store.contains_key("$X7F_REQ_2"));
        assert_eq!(refs["$X7F_REQ_1"].summary, "OAuth2 login flow");
        assert_eq!(refs["$X7F_REQ_2"].summary, "Session persistence");
        assert_eq!(refs["$X7F_REQ_1"].var_ref, "$X7F_REQ_1");
    }

    #[test]
    fn assign_refs_scalar_field() {
        let input: Map<String, Value> = serde_json::from_value(json!({
            "background": "Implement user authentication"
        }))
        .unwrap();

        let (store, refs) = assign_refs("X7F", &input);

        assert!(store.contains_key("$X7F_BACKGROUND"));
        assert_eq!(
            refs["$X7F_BACKGROUND"].summary,
            "Implement user authentication"
        );
    }

    #[test]
    fn assign_refs_multi_word_array_field() {
        let input: Map<String, Value> = serde_json::from_value(json!({
            "acceptance_criteria": [
                {"id": "AC_1", "description": "Users can authenticate via OAuth2"}
            ]
        }))
        .unwrap();

        let (store, refs) = assign_refs("X7F", &input);

        assert!(store.contains_key("$X7F_AC_1"));
        assert_eq!(
            refs["$X7F_AC_1"].summary,
            "Users can authenticate via OAuth2"
        );
    }

    #[test]
    fn load_store_missing_file() {
        let store = load_store(Path::new("/nonexistent/vars.json")).unwrap();
        assert!(store.is_empty());
    }

    #[test]
    fn store_append_preserves_existing() {
        let dir = tempfile::tempdir().unwrap();
        let vars_path = dir.path().join("vars.json");
        fs::write(&vars_path, r#"{"$OLD_VAR": "existing value"}"#).unwrap();

        let mut store = load_store(&vars_path).unwrap();
        store.insert("$NEW_VAR".into(), json!("new value"));

        assert_eq!(store.len(), 2);
        assert_eq!(store["$OLD_VAR"], json!("existing value"));
        assert_eq!(store["$NEW_VAR"], json!("new value"));
    }

    #[test]
    fn extract_summary_object_fallback_first_string() {
        // No summary/description/text/value — should pick first string field
        let val = json!({"id": "X_1", "name": "some name"});
        let result = extract_summary(&val);
        // serde_json::Map iteration order isn't guaranteed, but one of the string fields
        assert!(result == "X_1" || result == "some name");
    }

    #[test]
    fn extract_summary_object_no_string_fields() {
        let val = json!({"count": 42, "active": true});
        let result = extract_summary(&val);
        // Falls back to JSON serialization
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["count"], 42);
    }

    #[test]
    fn extract_summary_null() {
        assert_eq!(extract_summary(&Value::Null), "null");
    }

    #[test]
    fn extract_summary_bool() {
        assert_eq!(extract_summary(&json!(true)), "true");
    }

    #[test]
    fn extract_summary_empty_object() {
        let val = json!({});
        let result = extract_summary(&val);
        assert_eq!(result, "{}");
    }

    #[test]
    fn extract_summary_empty_string() {
        assert_eq!(extract_summary(&json!("")), "");
    }

    #[test]
    fn assign_refs_empty_array() {
        let input: Map<String, Value> = serde_json::from_value(json!({"items": []})).unwrap();
        let (store, refs) = assign_refs("P", &input);
        assert!(store.is_empty());
        assert!(refs.is_empty());
    }

    #[test]
    fn assign_refs_mixed_array_and_scalar() {
        let input: Map<String, Value> = serde_json::from_value(json!({
            "tasks": [{"summary": "task one"}],
            "background": "some context"
        }))
        .unwrap();

        let (store, refs) = assign_refs("M1", &input);

        assert!(store.contains_key("$M1_TAS_1"));
        assert!(store.contains_key("$M1_BACKGROUND"));
        assert_eq!(refs["$M1_TAS_1"].summary, "task one");
        assert_eq!(refs["$M1_BACKGROUND"].summary, "some context");
    }

    #[test]
    fn assign_refs_nested_object_treated_as_scalar() {
        let input: Map<String, Value> = serde_json::from_value(json!({
            "metadata": {"author": "alice", "version": 2}
        }))
        .unwrap();

        let (store, refs) = assign_refs("N1", &input);

        assert!(store.contains_key("$N1_METADATA"));
        // The stored value is the full nested object
        assert_eq!(store["$N1_METADATA"]["author"], "alice");
        // Summary should find first string field
        assert!(!refs["$N1_METADATA"].summary.is_empty());
    }

    #[test]
    fn assign_refs_number_value() {
        let input: Map<String, Value> = serde_json::from_value(json!({"priority": 5})).unwrap();
        let (store, refs) = assign_refs("N", &input);

        assert_eq!(store["$N_PRIORITY"], json!(5));
        assert_eq!(refs["$N_PRIORITY"].summary, "5");
    }

    #[test]
    fn assign_refs_bool_value() {
        let input: Map<String, Value> = serde_json::from_value(json!({"active": true})).unwrap();
        let (store, refs) = assign_refs("B", &input);

        assert_eq!(store["$B_ACTIVE"], json!(true));
        assert_eq!(refs["$B_ACTIVE"].summary, "true");
    }

    #[test]
    fn assign_refs_null_value() {
        let input: Map<String, Value> = serde_json::from_value(json!({"notes": null})).unwrap();
        let (store, refs) = assign_refs("N", &input);

        assert_eq!(store["$N_NOTES"], Value::Null);
        assert_eq!(refs["$N_NOTES"].summary, "null");
    }

    #[test]
    fn assign_refs_multiple_array_items_sequential_indices() {
        let input: Map<String, Value> = serde_json::from_value(json!({
            "steps": [
                {"summary": "first"},
                {"summary": "second"},
                {"summary": "third"}
            ]
        }))
        .unwrap();

        let (store, refs) = assign_refs("S", &input);

        assert_eq!(refs["$S_STE_1"].summary, "first");
        assert_eq!(refs["$S_STE_2"].summary, "second");
        assert_eq!(refs["$S_STE_3"].summary, "third");
        assert_eq!(store.len(), 3);
    }

    #[test]
    fn assign_refs_empty_object_input() {
        let input: Map<String, Value> = serde_json::from_value(json!({})).unwrap();
        let (store, refs) = assign_refs("E", &input);
        assert!(store.is_empty());
        assert!(refs.is_empty());
    }

    #[test]
    fn store_roundtrip_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let input_path = dir.path().join("input.json");
        fs::write(
            &input_path,
            r#"{"items": [{"summary": "alpha"}, {"summary": "beta"}]}"#,
        )
        .unwrap();

        run(dir.path(), "RT", Some(input_path.as_path())).unwrap();

        let vars_path = dir.path().join("vars.json");
        assert!(vars_path.exists());

        let store: VarStore =
            serde_json::from_str(&fs::read_to_string(&vars_path).unwrap()).unwrap();
        assert_eq!(store.len(), 2);
        assert_eq!(store["$RT_ITE_1"]["summary"], "alpha");
        assert_eq!(store["$RT_ITE_2"]["summary"], "beta");
    }

    #[test]
    fn store_via_library_api() {
        let dir = tempfile::tempdir().unwrap();
        let input: Map<String, Value> = serde_json::from_value(json!({
            "requirements": [
                {"id": "REQ_1", "summary": "OAuth2 login flow"},
                {"id": "REQ_2", "summary": "Session persistence"}
            ],
            "background": "Implement user authentication"
        }))
        .unwrap();

        let output = store(dir.path(), "X7F", &input).unwrap();

        // Verify refs are returned
        assert_eq!(output.refs["$X7F_REQ_1"].summary, "OAuth2 login flow");
        assert_eq!(output.refs["$X7F_REQ_2"].summary, "Session persistence");
        assert_eq!(
            output.refs["$X7F_BACKGROUND"].summary,
            "Implement user authentication"
        );

        // Verify vars.json was written
        let vars_path = dir.path().join("vars.json");
        assert!(vars_path.exists());
        let store_data: VarStore =
            serde_json::from_str(&std::fs::read_to_string(&vars_path).unwrap()).unwrap();
        assert_eq!(store_data["$X7F_REQ_1"]["summary"], "OAuth2 login flow");
    }

    #[test]
    fn store_multiple_calls_merge() {
        let dir = tempfile::tempdir().unwrap();

        let input1 = dir.path().join("input1.json");
        fs::write(&input1, r#"{"a": [{"summary": "from a"}]}"#).unwrap();
        run(dir.path(), "P1", Some(input1.as_path())).unwrap();

        let input2 = dir.path().join("input2.json");
        fs::write(&input2, r#"{"b": [{"summary": "from b"}]}"#).unwrap();
        run(dir.path(), "P2", Some(input2.as_path())).unwrap();

        let store: VarStore =
            serde_json::from_str(&fs::read_to_string(dir.path().join("vars.json")).unwrap())
                .unwrap();
        assert_eq!(store.len(), 2);
        assert!(store.contains_key("$P1_A_1"));
        assert!(store.contains_key("$P2_B_1"));
    }
}
