use std::fs;
use std::process::Command;

use serde_json::{json, Value};

fn symref_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_symref"))
}

/// Helper: run store then deref in sequence.
fn store_then_deref(
    session: &std::path::Path,
    prefix: &str,
    store_input: &str,
    deref_input: &str,
) -> std::process::Output {
    let store_path = session.join("store_input.json");
    fs::write(&store_path, store_input).unwrap();

    let store_out = symref_bin()
        .args([
            "store",
            "--session",
            session.to_str().unwrap(),
            "--prefix",
            prefix,
            "--input",
            store_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        store_out.status.success(),
        "store failed: {}",
        String::from_utf8_lossy(&store_out.stderr)
    );

    let deref_path = session.join("deref_input.json");
    fs::write(&deref_path, deref_input).unwrap();

    symref_bin()
        .args([
            "deref",
            "--session",
            session.to_str().unwrap(),
            "--input",
            deref_path.to_str().unwrap(),
        ])
        .output()
        .unwrap()
}

#[test]
fn roundtrip_json_substitution() {
    let session = tempfile::tempdir().unwrap();

    let store_input = serde_json::to_string_pretty(&json!({
        "requirements": [
            {"id": "REQ_1", "summary": "OAuth2 login flow", "priority": "high"},
            {"id": "REQ_2", "summary": "Session persistence", "priority": "medium"}
        ],
        "acceptance_criteria": [
            {"id": "AC_1", "description": "Users can authenticate via OAuth2"}
        ]
    }))
    .unwrap();

    let deref_input = serde_json::to_string_pretty(&json!({
        "summary": "Implement OAuth2 login",
        "description": "## Acceptance Criteria\n- $X7F_AC_1\n\n## Requirements\n- $X7F_REQ_1"
    }))
    .unwrap();

    let output = store_then_deref(session.path(), "X7F", &store_input, &deref_input);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["summary"], "Implement OAuth2 login");

    let desc = result["description"].as_str().unwrap();
    assert!(
        desc.contains("Users can authenticate via OAuth2"),
        "expected AC_1 substitution in: {}",
        desc
    );
    assert!(
        desc.contains("OAuth2 login flow"),
        "expected REQ_1 substitution in: {}",
        desc
    );
}

#[test]
fn roundtrip_plain_text_substitution() {
    let session = tempfile::tempdir().unwrap();

    let store_input = r#"{"tasks": [{"summary": "Write tests"}]}"#;
    let deref_input = "Please implement $T1_TAS_1";

    let deref_path = session.path().join("deref_input.txt");

    // Store first
    let store_path = session.path().join("store_input.json");
    fs::write(&store_path, store_input).unwrap();
    let store_out = symref_bin()
        .args([
            "store",
            "--session",
            session.path().to_str().unwrap(),
            "--prefix",
            "T1",
            "--input",
            store_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(store_out.status.success());

    // Deref
    fs::write(&deref_path, deref_input).unwrap();
    let output = symref_bin()
        .args([
            "deref",
            "--session",
            session.path().to_str().unwrap(),
            "--input",
            deref_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let result = String::from_utf8(output.stdout).unwrap();
    assert!(
        result.contains("Write tests"),
        "expected substitution in: {}",
        result
    );
}

#[test]
fn deref_unresolved_var_warns_and_preserves() {
    let session = tempfile::tempdir().unwrap();

    // Create a minimal store
    let store_input = r#"{"items": [{"summary": "known"}]}"#;
    let store_path = session.path().join("store_input.json");
    fs::write(&store_path, store_input).unwrap();
    let store_out = symref_bin()
        .args([
            "store",
            "--session",
            session.path().to_str().unwrap(),
            "--prefix",
            "K",
            "--input",
            store_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(store_out.status.success());

    // Deref with an unknown variable
    let deref_path = session.path().join("template.txt");
    fs::write(&deref_path, "known=$K_ITE_1 unknown=$NOPE").unwrap();

    let output = symref_bin()
        .args([
            "deref",
            "--session",
            session.path().to_str().unwrap(),
            "--input",
            deref_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("known=known"),
        "expected resolved var in: {}",
        stdout
    );
    assert!(
        stdout.contains("$NOPE"),
        "expected unresolved var preserved in: {}",
        stdout
    );

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("NOPE"),
        "expected warning on stderr: {}",
        stderr
    );
}

#[test]
fn deref_missing_vars_json_fails() {
    let session = tempfile::tempdir().unwrap();

    let output = symref_bin()
        .args([
            "deref",
            "--session",
            session.path().to_str().unwrap(),
            "--input",
            "/dev/null",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("vars.json"),
        "expected helpful error: {}",
        stderr
    );
}

#[test]
fn multiple_prefixes_coexist() {
    let session = tempfile::tempdir().unwrap();

    // Store with prefix A
    let input_a = session.path().join("a.json");
    fs::write(&input_a, r#"{"items": [{"summary": "from A"}]}"#).unwrap();
    let out_a = symref_bin()
        .args([
            "store",
            "--session",
            session.path().to_str().unwrap(),
            "--prefix",
            "A",
            "--input",
            input_a.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out_a.status.success());

    // Store with prefix B
    let input_b = session.path().join("b.json");
    fs::write(&input_b, r#"{"items": [{"summary": "from B"}]}"#).unwrap();
    let out_b = symref_bin()
        .args([
            "store",
            "--session",
            session.path().to_str().unwrap(),
            "--prefix",
            "B",
            "--input",
            input_b.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out_b.status.success());

    // Deref both
    let template = session.path().join("template.txt");
    fs::write(&template, "A=$A_ITE_1 B=$B_ITE_1").unwrap();
    let output = symref_bin()
        .args([
            "deref",
            "--session",
            session.path().to_str().unwrap(),
            "--input",
            template.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let result = String::from_utf8(output.stdout).unwrap();
    assert!(result.contains("from A"), "result: {}", result);
    assert!(result.contains("from B"), "result: {}", result);
}

#[test]
fn deref_braces_syntax() {
    let session = tempfile::tempdir().unwrap();

    let store_input = r#"{"items": [{"summary": "braces work"}]}"#;
    let deref_input = "test ${B_ITE_1} done";

    let store_path = session.path().join("store_input.json");
    fs::write(&store_path, store_input).unwrap();
    let store_out = symref_bin()
        .args([
            "store",
            "--session",
            session.path().to_str().unwrap(),
            "--prefix",
            "B",
            "--input",
            store_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(store_out.status.success());

    let deref_path = session.path().join("template.txt");
    fs::write(&deref_path, deref_input).unwrap();
    let output = symref_bin()
        .args([
            "deref",
            "--session",
            session.path().to_str().unwrap(),
            "--input",
            deref_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let result = String::from_utf8(output.stdout).unwrap();
    assert!(
        result.contains("braces work"),
        "expected braces substitution in: {}",
        result
    );
}

#[test]
fn deref_no_recursion() {
    let session = tempfile::tempdir().unwrap();

    // Store two vars where the first's value contains a reference to the second
    let vars = json!({
        "$NR_A_1": "see $NR_B_1",
        "$NR_B_1": "deep value"
    });
    fs::write(
        session.path().join("vars.json"),
        serde_json::to_string(&vars).unwrap(),
    )
    .unwrap();

    let deref_path = session.path().join("template.txt");
    fs::write(&deref_path, "result=$NR_A_1").unwrap();

    let output = symref_bin()
        .args([
            "deref",
            "--session",
            session.path().to_str().unwrap(),
            "--input",
            deref_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let result = String::from_utf8(output.stdout).unwrap();
    // Should NOT recurse: the substituted value contains $NR_B_1, which should remain literal
    assert!(
        result.contains("see $NR_B_1"),
        "substitution should not recurse: {}",
        result
    );
    assert!(
        !result.contains("deep value"),
        "should not have recursed into $NR_B_1: {}",
        result
    );
}

#[test]
fn deref_multiple_vars_in_one_json_string() {
    let session = tempfile::tempdir().unwrap();

    let store_input = serde_json::to_string(&json!({
        "requirements": [
            {"summary": "first"},
            {"summary": "second"}
        ]
    }))
    .unwrap();
    let deref_input = serde_json::to_string(&json!({
        "combined": "$TC_REQ_1 and $TC_REQ_2"
    }))
    .unwrap();

    let output = store_then_deref(session.path(), "TC", &store_input, &deref_input);
    assert!(output.status.success());

    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["combined"], "first and second");
}

#[test]
fn deref_json_nested_structure() {
    let session = tempfile::tempdir().unwrap();

    let store_input = r#"{"items": [{"summary": "nested-val"}]}"#;
    let deref_input = serde_json::to_string(&json!({
        "outer": {
            "inner": {
                "deep": "$D_ITE_1"
            }
        }
    }))
    .unwrap();

    let output = store_then_deref(session.path(), "D", store_input, &deref_input);
    assert!(output.status.success());

    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["outer"]["inner"]["deep"], "nested-val");
}

#[test]
fn deref_json_array_values() {
    let session = tempfile::tempdir().unwrap();

    let store_input = r#"{"items": [{"summary": "alpha"}, {"summary": "beta"}]}"#;
    let deref_input = serde_json::to_string(&json!({
        "list": ["$A_ITE_1", "$A_ITE_2", "literal"]
    }))
    .unwrap();

    let output = store_then_deref(session.path(), "A", store_input, &deref_input);
    assert!(output.status.success());

    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["list"][0], "alpha");
    assert_eq!(result["list"][1], "beta");
    assert_eq!(result["list"][2], "literal");
}

#[test]
fn deref_preserves_non_string_json_values() {
    let session = tempfile::tempdir().unwrap();

    let store_input = r#"{"items": [{"summary": "val"}]}"#;
    let deref_input = serde_json::to_string(&json!({
        "name": "$P_ITE_1",
        "count": 42,
        "active": true,
        "data": null
    }))
    .unwrap();

    let output = store_then_deref(session.path(), "P", store_input, &deref_input);
    assert!(output.status.success());

    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["name"], "val");
    assert_eq!(result["count"], 42);
    assert_eq!(result["active"], true);
    assert!(result["data"].is_null());
}

#[test]
fn deref_from_stdin() {
    let session = tempfile::tempdir().unwrap();

    // Store first
    let store_path = session.path().join("input.json");
    fs::write(&store_path, r#"{"items": [{"summary": "stdin-val"}]}"#).unwrap();
    let store_out = symref_bin()
        .args([
            "store",
            "--session",
            session.path().to_str().unwrap(),
            "--prefix",
            "SI",
            "--input",
            store_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(store_out.status.success());

    // Deref from stdin
    let output = symref_bin()
        .args(["deref", "--session", session.path().to_str().unwrap()])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap()
        .wait_with_output_with_stdin("result=$SI_ITE_1");

    assert!(output.status.success());
    let result = String::from_utf8(output.stdout).unwrap();
    assert!(
        result.contains("stdin-val"),
        "expected substitution: {}",
        result
    );
}

#[test]
fn deref_unresolved_var_in_json() {
    let session = tempfile::tempdir().unwrap();

    let store_input = r#"{"items": [{"summary": "known"}]}"#;
    let deref_input = serde_json::to_string(&json!({
        "a": "$J_ITE_1",
        "b": "$MISSING_VAR"
    }))
    .unwrap();

    let output = store_then_deref(session.path(), "J", store_input, &deref_input);
    assert!(output.status.success());

    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["a"], "known");
    assert_eq!(result["b"], "$MISSING_VAR");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("MISSING_VAR"),
        "expected warning: {}",
        stderr
    );
}

#[test]
fn deref_object_value_extracts_description_field() {
    let session = tempfile::tempdir().unwrap();

    // Manually create vars.json with an object that has description but no summary
    let vars = json!({
        "$D_AC_1": {"id": "AC_1", "description": "Must support OAuth2"}
    });
    fs::write(
        session.path().join("vars.json"),
        serde_json::to_string(&vars).unwrap(),
    )
    .unwrap();

    let deref_path = session.path().join("template.txt");
    fs::write(&deref_path, "criteria: $D_AC_1").unwrap();

    let output = symref_bin()
        .args([
            "deref",
            "--session",
            session.path().to_str().unwrap(),
            "--input",
            deref_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let result = String::from_utf8(output.stdout).unwrap();
    assert!(
        result.contains("Must support OAuth2"),
        "expected description extraction: {}",
        result
    );
}

#[test]
fn large_store_hundreds_of_variables() {
    let session = tempfile::tempdir().unwrap();

    // Build input with 200 array items
    let items: Vec<Value> = (1..=200)
        .map(|i| json!({"summary": format!("item number {}", i)}))
        .collect();
    let input = json!({"entries": items});

    let input_path = session.path().join("input.json");
    fs::write(&input_path, serde_json::to_string(&input).unwrap()).unwrap();

    let store_out = symref_bin()
        .args([
            "store",
            "--session",
            session.path().to_str().unwrap(),
            "--prefix",
            "BIG",
            "--input",
            input_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        store_out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&store_out.stderr)
    );

    let result: Value = serde_json::from_slice(&store_out.stdout).unwrap();
    assert_eq!(result["refs"].as_object().unwrap().len(), 200);

    // Deref the first and last
    let deref_path = session.path().join("template.txt");
    fs::write(&deref_path, "first=$BIG_ENT_1 last=$BIG_ENT_200").unwrap();

    let deref_out = symref_bin()
        .args([
            "deref",
            "--session",
            session.path().to_str().unwrap(),
            "--input",
            deref_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(deref_out.status.success());

    let result = String::from_utf8(deref_out.stdout).unwrap();
    assert!(result.contains("item number 1"), "result: {}", result);
    assert!(result.contains("item number 200"), "result: {}", result);
}

#[test]
fn full_pipeline_store_fixtures_then_deref_fixtures() {
    // Uses the test fixture files from tests/fixtures/
    let session = tempfile::tempdir().unwrap();

    let store_out = symref_bin()
        .args([
            "store",
            "--session",
            session.path().to_str().unwrap(),
            "--prefix",
            "X7F",
            "--input",
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/validated.json"),
        ])
        .output()
        .unwrap();
    assert!(
        store_out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&store_out.stderr)
    );

    let deref_out = symref_bin()
        .args([
            "deref",
            "--session",
            session.path().to_str().unwrap(),
            "--input",
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/template.json"),
        ])
        .output()
        .unwrap();
    assert!(
        deref_out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&deref_out.stderr)
    );

    let result: Value = serde_json::from_slice(&deref_out.stdout).unwrap();
    assert_eq!(result["summary"], "Implement OAuth2 login");
    let desc = result["description"].as_str().unwrap();
    assert!(desc.contains("Users can authenticate via OAuth2"));
    assert!(desc.contains("OAuth2 login flow"));
}

trait WaitWithStdin {
    fn wait_with_output_with_stdin(self, input: &str) -> std::process::Output;
}

impl WaitWithStdin for std::process::Child {
    fn wait_with_output_with_stdin(mut self, input: &str) -> std::process::Output {
        use std::io::Write;
        if let Some(ref mut stdin) = self.stdin {
            stdin.write_all(input.as_bytes()).unwrap();
        }
        self.wait_with_output().unwrap()
    }
}
