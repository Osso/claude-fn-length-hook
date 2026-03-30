mod lines;
mod php_parser;
mod rust_parser;

use serde::Deserialize;
use std::io::{self, Read};
use std::path::Path;
use std::process;

#[derive(Deserialize)]
struct HookInput {
    tool_name: Option<String>,
    tool_input: serde_json::Value,
}

fn main() {
    let input = read_stdin_json();
    let file_path = match extract_file_path(&input) {
        Some(p) => p,
        None => return,
    };

    let ext = Path::new(&file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    if !matches!(ext, "rs" | "php") {
        return;
    }

    let (before, after) = match resolve_content(&input, &file_path) {
        Some(pair) => pair,
        None => return,
    };

    let messages = match ext {
        "rs" => check_rust(before.as_deref(), &after),
        "php" => check_php(before.as_deref(), &after, &file_path),
        _ => return,
    };

    if !messages.is_empty() {
        block(&format_block_message(&file_path, &messages));
    }
}

fn extract_file_path(input: &HookInput) -> Option<String> {
    input
        .tool_input
        .get("file_path")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Simulate the edit to get (before, after) content.
fn resolve_content(input: &HookInput, file_path: &str) -> Option<(Option<String>, String)> {
    let tool = input.tool_name.as_deref().unwrap_or("Edit");
    let before = std::fs::read_to_string(file_path).ok();

    match tool {
        "Edit" => resolve_edit(&input.tool_input, before),
        "Write" => resolve_write(&input.tool_input, before),
        _ => None,
    }
}

fn resolve_edit(
    tool_input: &serde_json::Value,
    before: Option<String>,
) -> Option<(Option<String>, String)> {
    let old_str = tool_input.get("old_string")?.as_str()?;
    let new_str = tool_input.get("new_string")?.as_str()?;
    let current = before.as_ref()?;
    let after = current.replacen(old_str, new_str, 1);
    Some((before, after))
}

fn resolve_write(
    tool_input: &serde_json::Value,
    before: Option<String>,
) -> Option<(Option<String>, String)> {
    let content = tool_input.get("content")?.as_str()?.to_string();
    Some((before, content))
}

fn check_rust(before: Option<&str>, after: &str) -> Vec<String> {
    let mut messages: Vec<String> = Vec::new();
    let result = rust_parser::check(after);

    for v in &result.fn_violations {
        let limit = if v.is_test { rust_parser::TEST_BODY_LIMIT } else { rust_parser::BODY_LIMIT };
        messages.push(format!(
            "{} (line {}): {} body lines (max {})",
            v.name, v.line, v.body_lines, limit
        ));
    }

    check_file_length(before, result.file_lines, &mut messages);
    messages
}

fn check_file_length(before: Option<&str>, after_lines: usize, messages: &mut Vec<String>) {
    if after_lines <= rust_parser::FILE_LINE_LIMIT {
        return;
    }
    let before_lines = before.map(|s| s.lines().count()).unwrap_or(0);
    if before_lines <= rust_parser::FILE_LINE_LIMIT || after_lines > before_lines {
        messages.push(format!(
            "File is {} lines (max {}). Consider splitting it.",
            after_lines,
            rust_parser::FILE_LINE_LIMIT
        ));
    }
}

fn check_php(before: Option<&str>, after: &str, file_path: &str) -> Vec<String> {
    let is_test_file = file_path.ends_with("Test.php") || file_path.ends_with("Cest.php");
    php_parser::check(after, before, is_test_file)
        .iter()
        .map(|v| format_php_violation(v, is_test_file))
        .collect()
}

fn format_php_violation(v: &php_parser::Violation, is_test_file: bool) -> String {
    let applicable_limit = if is_test_file { php_parser::TEST_BODY_LIMIT } else { php_parser::BODY_LIMIT };
    match v.old_body_lines {
        Some(old) if old > applicable_limit => format!(
            "{} (line {}): {} body lines (was {}, cannot grow legacy function)",
            v.name, v.line, v.body_lines, old
        ),
        _ => format!(
            "{} (line {}): {} body lines (max {})",
            v.name, v.line, v.body_lines, applicable_limit
        ),
    }
}

fn format_block_message(file_path: &str, messages: &[String]) -> String {
    let short = Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(file_path);
    let detail = messages.join("\n  - ");
    format!(
        "Function body exceeds limit in {}:\n  - {}\nExtract logic into well-named helper functions.",
        short, detail
    )
}

fn read_stdin_json() -> HookInput {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf).unwrap_or(0);
    serde_json::from_str(&buf).unwrap_or_else(|_| process::exit(0))
}

fn block(reason: &str) {
    let output = serde_json::json!({
        "decision": "block",
        "reason": reason
    });
    println!("{}", output);
    process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_edit_replaces_old_string_in_simulated_output() {
        let input = serde_json::json!({
            "old_string": "before",
            "new_string": "after"
        });

        let (before, after) = resolve_edit(&input, Some("before value".to_string())).unwrap();

        assert_eq!(before.as_deref(), Some("before value"));
        assert_eq!(after, "after value");
    }

    #[test]
    fn resolve_write_uses_full_replacement_content() {
        let input = serde_json::json!({
            "content": "new file contents"
        });

        let (before, after) = resolve_write(&input, Some("old file contents".to_string())).unwrap();

        assert_eq!(before.as_deref(), Some("old file contents"));
        assert_eq!(after, "new file contents");
    }

    #[test]
    fn check_file_length_allows_legacy_oversized_file_without_growth() {
        let before = vec!["x"; rust_parser::FILE_LINE_LIMIT + 5].join("\n");
        let mut messages = Vec::new();

        check_file_length(Some(&before), rust_parser::FILE_LINE_LIMIT + 5, &mut messages);

        assert!(messages.is_empty());
    }

    #[test]
    fn check_file_length_blocks_legacy_oversized_file_growth() {
        let before = vec!["x"; rust_parser::FILE_LINE_LIMIT + 5].join("\n");
        let mut messages = Vec::new();

        check_file_length(Some(&before), rust_parser::FILE_LINE_LIMIT + 6, &mut messages);

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("max 750"));
    }
}
