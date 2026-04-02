use std::fs;
use std::process::Command;

use serde_json::{json, Value};

fn symref_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_symref"))
}

#[test]
fn store_from_file_creates_vars_and_outputs_refs() {
    let session = tempfile::tempdir().unwrap();
    let input_path = session.path().join("input.json");

    let input = json!({
        "requirements": [
            {"id": "REQ_1", "summary": "OAuth2 login flow", "priority": "high"},
            {"id": "REQ_2", "summary": "Session persistence", "priority": "medium"}
        ],
        "acceptance_criteria": [
            {"id": "AC_1", "description": "Users can authenticate via OAuth2"}
        ]
    });
    fs::write(&input_path, serde_json::to_string_pretty(&input).unwrap()).unwrap();

    let output = symref_bin()
        .args([
            "store",
            "--session",
            session.path().to_str().unwrap(),
            "--prefix",
            "X7F",
            "--input",
            input_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let result: Value = serde_json::from_slice(&output.stdout).unwrap();

    // Check refs in output
    assert_eq!(result["refs"]["$X7F_REQ_1"]["summary"], "OAuth2 login flow");
    assert_eq!(
        result["refs"]["$X7F_REQ_2"]["summary"],
        "Session persistence"
    );
    assert_eq!(
        result["refs"]["$X7F_AC_1"]["summary"],
        "Users can authenticate via OAuth2"
    );
    assert_eq!(result["refs"]["$X7F_REQ_1"]["ref"], "$X7F_REQ_1");

    // Check vars.json was created with correct content
    let vars_path = session.path().join("vars.json");
    assert!(vars_path.exists());
    let vars: Value = serde_json::from_str(&fs::read_to_string(&vars_path).unwrap()).unwrap();
    assert_eq!(vars["$X7F_REQ_1"]["summary"], "OAuth2 login flow");
    assert_eq!(
        vars["$X7F_AC_1"]["description"],
        "Users can authenticate via OAuth2"
    );
}

#[test]
fn store_scalar_field() {
    let session = tempfile::tempdir().unwrap();
    let input_path = session.path().join("input.json");

    fs::write(
        &input_path,
        r#"{"background": "Implement user authentication for the platform"}"#,
    )
    .unwrap();

    let output = symref_bin()
        .args([
            "store",
            "--session",
            session.path().to_str().unwrap(),
            "--prefix",
            "X7F",
            "--input",
            input_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());

    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        result["refs"]["$X7F_BACKGROUND"]["summary"],
        "Implement user authentication for the platform"
    );

    let vars: Value =
        serde_json::from_str(&fs::read_to_string(session.path().join("vars.json")).unwrap())
            .unwrap();
    assert_eq!(
        vars["$X7F_BACKGROUND"],
        "Implement user authentication for the platform"
    );
}

#[test]
fn store_appends_to_existing_vars() {
    let session = tempfile::tempdir().unwrap();
    let vars_path = session.path().join("vars.json");

    // Pre-populate with an existing variable
    fs::write(&vars_path, r#"{"$OLD_VAR": "old value"}"#).unwrap();

    let input_path = session.path().join("input.json");
    fs::write(&input_path, r#"{"items": [{"summary": "new item"}]}"#).unwrap();

    let output = symref_bin()
        .args([
            "store",
            "--session",
            session.path().to_str().unwrap(),
            "--prefix",
            "X1",
            "--input",
            input_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());

    let vars: Value = serde_json::from_str(&fs::read_to_string(&vars_path).unwrap()).unwrap();
    assert_eq!(vars["$OLD_VAR"], "old value");
    assert!(vars["$X1_ITE_1"].is_object());
}

#[test]
fn store_from_stdin() {
    let session = tempfile::tempdir().unwrap();

    let output = symref_bin()
        .args([
            "store",
            "--session",
            session.path().to_str().unwrap(),
            "--prefix",
            "S1",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap()
        .wait_with_output_with_stdin(r#"{"notes": [{"text": "hello"}]}"#);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["refs"]["$S1_NOT_1"]["summary"], "hello");
}

#[test]
fn store_invalid_json_fails() {
    let session = tempfile::tempdir().unwrap();
    let input_path = session.path().join("input.json");
    fs::write(&input_path, "not json").unwrap();

    let output = symref_bin()
        .args([
            "store",
            "--session",
            session.path().to_str().unwrap(),
            "--prefix",
            "X",
            "--input",
            input_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
}

#[test]
fn store_missing_session_fails() {
    let output = symref_bin()
        .args([
            "store",
            "--session",
            "/nonexistent/session",
            "--prefix",
            "X",
            "--input",
            "/dev/null",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
}

#[test]
fn store_empty_json_object() {
    let session = tempfile::tempdir().unwrap();
    let input_path = session.path().join("input.json");
    fs::write(&input_path, "{}").unwrap();

    let output = symref_bin()
        .args([
            "store",
            "--session",
            session.path().to_str().unwrap(),
            "--prefix",
            "E",
            "--input",
            input_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(result["refs"].as_object().unwrap().is_empty());
}

#[test]
fn store_output_has_store_path() {
    let session = tempfile::tempdir().unwrap();
    let input_path = session.path().join("input.json");
    fs::write(&input_path, r#"{"x": "y"}"#).unwrap();

    let output = symref_bin()
        .args([
            "store",
            "--session",
            session.path().to_str().unwrap(),
            "--prefix",
            "P",
            "--input",
            input_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    let store_path = result["store_path"].as_str().unwrap();
    assert!(
        store_path.ends_with("vars.json"),
        "store_path should end with vars.json: {}",
        store_path
    );
}

#[test]
fn store_mixed_arrays_and_scalars() {
    let session = tempfile::tempdir().unwrap();
    let input_path = session.path().join("input.json");
    let input = json!({
        "requirements": [
            {"id": "REQ_1", "summary": "Login"},
            {"id": "REQ_2", "summary": "Logout"}
        ],
        "background": "Auth system redesign",
        "priority": 1
    });
    fs::write(&input_path, serde_json::to_string(&input).unwrap()).unwrap();

    let output = symref_bin()
        .args([
            "store",
            "--session",
            session.path().to_str().unwrap(),
            "--prefix",
            "MIX",
            "--input",
            input_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());

    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    let refs = result["refs"].as_object().unwrap();

    assert_eq!(refs["$MIX_REQ_1"]["summary"], "Login");
    assert_eq!(refs["$MIX_REQ_2"]["summary"], "Logout");
    assert_eq!(refs["$MIX_BACKGROUND"]["summary"], "Auth system redesign");
    assert_eq!(refs["$MIX_PRIORITY"]["summary"], "1");
}

#[test]
fn store_empty_array_field() {
    let session = tempfile::tempdir().unwrap();
    let input_path = session.path().join("input.json");
    fs::write(&input_path, r#"{"items": [], "name": "test"}"#).unwrap();

    let output = symref_bin()
        .args([
            "store",
            "--session",
            session.path().to_str().unwrap(),
            "--prefix",
            "EA",
            "--input",
            input_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    let refs = result["refs"].as_object().unwrap();
    // Only the scalar field should produce a ref
    assert_eq!(refs.len(), 1);
    assert!(refs.contains_key("$EA_NAME"));
}

#[test]
fn store_json_array_not_object_fails() {
    let session = tempfile::tempdir().unwrap();
    let input_path = session.path().join("input.json");
    fs::write(&input_path, r#"[1, 2, 3]"#).unwrap();

    let output = symref_bin()
        .args([
            "store",
            "--session",
            session.path().to_str().unwrap(),
            "--prefix",
            "X",
            "--input",
            input_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
}

#[test]
fn store_nested_object_value() {
    let session = tempfile::tempdir().unwrap();
    let input_path = session.path().join("input.json");
    fs::write(&input_path, r#"{"config": {"timeout": 30, "retries": 3}}"#).unwrap();

    let output = symref_bin()
        .args([
            "store",
            "--session",
            session.path().to_str().unwrap(),
            "--prefix",
            "C",
            "--input",
            input_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());

    let vars: Value =
        serde_json::from_str(&fs::read_to_string(session.path().join("vars.json")).unwrap())
            .unwrap();
    assert_eq!(vars["$C_CONFIG"]["timeout"], 30);
    assert_eq!(vars["$C_CONFIG"]["retries"], 3);
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
