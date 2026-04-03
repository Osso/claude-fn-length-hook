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
    fs::create_dir_all(dir.join(".git")).unwrap();
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

fn read_plan(dir: &Path) -> String {
    fs::read_to_string(dir.join("PLAN.md")).unwrap_or_default()
}

fn rust_function_with_counted_lines(name: &str, count: usize) -> String {
    let body = vec!["    let value = 1;"; count].join("\n");
    format!("fn {name}() {{\n{body}\n}}\n")
}

fn rust_test_function_with_counted_lines(name: &str, count: usize) -> String {
    let body = vec!["    let value = 1;"; count].join("\n");
    format!("#[test]\nfn {name}() {{\n{body}\n}}\n")
}

fn php_function_with_counted_lines(name: &str, count: usize) -> String {
    let body = vec!["    $x++;"; count].join("\n");
    format!("<?php\nfunction {name}() {{\n{body}\n}}\n")
}

#[test]
fn write_payload_allows_small_rust_file_without_plan_entry() {
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
    assert!(read_plan(&dir).is_empty());
}

#[test]
fn write_payload_adds_plan_todo_for_oversized_rust_function() {
    let dir = temp_test_dir("write-plan");
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
    let plan = read_plan(&dir);

    assert!(output.status.success());
    assert!(stdout.trim().is_empty(), "should not block");
    assert!(plan.contains("- [ ] Refactor"));
    assert!(plan.contains("too_long_demo"));
}

#[test]
fn edit_payload_adds_plan_todo_without_modifying_source_file() {
    let dir = temp_test_dir("edit-plan");
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
    let plan = read_plan(&dir);
    let on_disk = fs::read_to_string(&file_path).unwrap();

    assert!(output.status.success());
    assert!(stdout.trim().is_empty(), "should not block");
    assert!(plan.contains("edit_demo"));
    assert_eq!(on_disk, rust_function_with_counted_lines("edit_demo", 2));
}

#[test]
fn rust_test_fn_with_50_body_lines_is_allowed() {
    let dir = temp_test_dir("rust-test-allow");
    let file_path = dir.join("sample.rs");

    let payload = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {
            "file_path": file_path,
            "content": rust_test_function_with_counted_lines("my_test", 50)
        }
    });

    let output = run_hook(&payload);

    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout).unwrap().trim().is_empty());
    assert!(read_plan(&dir).is_empty());
}

#[test]
fn rust_test_fn_with_201_body_lines_adds_plan_todo() {
    let dir = temp_test_dir("rust-test-plan");
    let file_path = dir.join("sample.rs");

    let payload = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {
            "file_path": file_path,
            "content": rust_test_function_with_counted_lines("my_test", 201)
        }
    });

    let output = run_hook(&payload);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let plan = read_plan(&dir);

    assert!(output.status.success());
    assert!(stdout.trim().is_empty(), "should not block");
    assert!(plan.contains("my_test"));
    assert!(plan.contains("max 200"));
}

#[test]
fn php_test_file_with_50_line_function_is_allowed() {
    let dir = temp_test_dir("php-test-allow");
    let file_path = dir.join("FooTest.php");

    let payload = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {
            "file_path": file_path,
            "content": php_function_with_counted_lines("test_something", 50)
        }
    });

    let output = run_hook(&payload);

    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout).unwrap().trim().is_empty());
    assert!(read_plan(&dir).is_empty());
}

#[test]
fn duplicate_plan_todos_are_not_added() {
    let dir = temp_test_dir("dedup");
    let file_path = dir.join("sample.rs");

    let payload = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {
            "file_path": file_path,
            "content": rust_function_with_counted_lines("dup_fn", 31)
        }
    });

    run_hook(&payload);
    run_hook(&payload);

    let plan = read_plan(&dir);
    let count = plan.matches("dup_fn").count();
    assert_eq!(count, 1, "should not duplicate TODO entries");
}

#[test]
fn same_function_different_line_count_does_not_duplicate() {
    let dir = temp_test_dir("dedup-lines");
    let file_path = dir.join("sample.rs");

    // First write: 35 body lines
    let payload1 = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {
            "file_path": file_path,
            "content": rust_function_with_counted_lines("growing_fn", 35)
        }
    });
    run_hook(&payload1);

    // Second write: 40 body lines (same function, different count)
    let payload2 = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {
            "file_path": file_path,
            "content": rust_function_with_counted_lines("growing_fn", 40)
        }
    });
    run_hook(&payload2);

    let plan = read_plan(&dir);
    let count = plan.matches("growing_fn").count();
    assert_eq!(count, 1, "same function with different line count should not duplicate");
}
