use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_test_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("claude-fn-length-hook-{name}-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_file(path: &Path, content: &str) {
    fs::write(path, content).unwrap();
}

fn run_hook(payload: &serde_json::Value) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_claude-fn-length-hook"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(payload.to_string().as_bytes())
        .unwrap();

    child.wait_with_output().unwrap()
}

fn rust_function_with_counted_lines(name: &str, count: usize) -> String {
    let body = vec!["    let value = 1;"; count].join("\n");
    format!("fn {name}() {{\n{body}\n}}\n")
}

#[test]
fn write_payload_allows_small_rust_file_without_block_output() {
    let dir = temp_test_dir("write-allow");
    let file_path = dir.join("sample.rs");

    let payload = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {
            "file_path": file_path,
            "content": rust_function_with_counted_lines("ok_demo", 2)
        }
    });

    let output = run_hook(&payload);

    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout).unwrap().trim().is_empty());
    assert!(String::from_utf8(output.stderr).unwrap().trim().is_empty());
}

#[test]
fn write_payload_blocks_oversized_rust_function_with_json_output() {
    let dir = temp_test_dir("write-block");
    let file_path = dir.join("sample.rs");

    let payload = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {
            "file_path": file_path,
            "content": rust_function_with_counted_lines("too_long_demo", 31)
        }
    });

    let output = run_hook(&payload);
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(stdout.contains("\"decision\":\"block\""));
    assert!(stdout.contains("too_long_demo"));
}

#[test]
fn edit_payload_simulates_updated_content_before_writing_to_disk() {
    let dir = temp_test_dir("edit-block");
    let file_path = dir.join("sample.rs");
    let original = rust_function_with_counted_lines("edit_demo", 2);
    let replacement = rust_function_with_counted_lines("edit_demo", 31);
    write_file(&file_path, &original);

    let payload = serde_json::json!({
        "tool_name": "Edit",
        "tool_input": {
            "file_path": file_path,
            "old_string": original,
            "new_string": replacement
        }
    });

    let output = run_hook(&payload);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let on_disk = fs::read_to_string(&file_path).unwrap();

    assert!(output.status.success());
    assert!(stdout.contains("\"decision\":\"block\""));
    assert!(stdout.contains("edit_demo"));
    assert_eq!(on_disk, rust_function_with_counted_lines("edit_demo", 2));
}
