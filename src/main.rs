mod lines;
mod php_parser;
mod rust_parser;

use serde::Deserialize;
use std::io::{self, Read};
use std::path::Path;
use std::process;

#[derive(Deserialize)]
struct HookInput {
    tool_input: ToolInput,
}

#[derive(Deserialize)]
struct ToolInput {
    file_path: String,
}

fn main() {
    let input = read_stdin_json();
    let file_path = &input.tool_input.file_path;

    let ext = Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "rs" => handle_rust(file_path),
        "php" => handle_php(file_path),
        _ => {}
    }
}

fn read_stdin_json() -> HookInput {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf).unwrap_or(0);
    serde_json::from_str(&buf).unwrap_or_else(|_| process::exit(0))
}

fn handle_rust(file_path: &str) {
    let source = match std::fs::read_to_string(file_path) {
        Ok(s) => s,
        Err(_) => return,
    };

    let result = rust_parser::check(&source);
    let mut messages: Vec<String> = Vec::new();

    format_fn_violations_rust(&result.fn_violations, file_path, &mut messages);

    if result.file_lines > rust_parser::FILE_LINE_LIMIT {
        messages.push(format!(
            "File is {} lines (max {}). Consider splitting it.",
            result.file_lines,
            rust_parser::FILE_LINE_LIMIT
        ));
    }

    if messages.is_empty() {
        return;
    }

    let short = Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(file_path);

    let detail = messages.join("\n  - ");
    let reason = format!(
        "Function body exceeds 30-line limit in {}:\n  - {}\nExtract logic into well-named helper functions.",
        short, detail
    );
    block(&reason);
}

fn format_fn_violations_rust(
    violations: &[rust_parser::Violation],
    _file_path: &str,
    messages: &mut Vec<String>,
) {
    for v in violations {
        messages.push(format!(
            "{} (line {}): {} body lines",
            v.name, v.line, v.body_lines
        ));
    }
}

fn handle_php(file_path: &str) {
    let source = match std::fs::read_to_string(file_path) {
        Ok(s) => s,
        Err(_) => return,
    };

    let old_source = fetch_committed_php(file_path);
    let violations = php_parser::check(&source, old_source.as_deref());

    if violations.is_empty() {
        return;
    }

    let short = Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(file_path);

    let details: Vec<String> = violations.iter().map(format_php_violation).collect();
    let detail = details.join("\n  - ");
    let reason = format!(
        "Function length violation in {}:\n  - {}\nNew functions: max 30 lines. Legacy functions: do not increase length.",
        short, detail
    );
    block(&reason);
}

fn format_php_violation(v: &php_parser::Violation) -> String {
    match v.old_body_lines {
        Some(old) if old > php_parser::BODY_LIMIT => format!(
            "{} (line {}): {} body lines (was {}, cannot grow legacy function)",
            v.name, v.line, v.body_lines, old
        ),
        _ => format!(
            "{} (line {}): {} body lines (max {})",
            v.name,
            v.line,
            v.body_lines,
            php_parser::BODY_LIMIT
        ),
    }
}

fn fetch_committed_php(file_path: &str) -> Option<String> {
    let repo_root = find_git_root(file_path)?;
    let rel = Path::new(file_path).strip_prefix(&repo_root).ok()?;
    let rel_str = rel.to_str()?;

    let output = process::Command::new("git")
        .args(["show", &format!("HEAD:{}", rel_str)])
        .current_dir(&repo_root)
        .output()
        .ok()?;

    if output.status.success() {
        String::from_utf8(output.stdout).ok()
    } else {
        None
    }
}

fn find_git_root(file_path: &str) -> Option<std::path::PathBuf> {
    let mut dir = Path::new(file_path).parent()?;
    loop {
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
}

fn block(reason: &str) {
    let output = serde_json::json!({
        "decision": "block",
        "reason": reason
    });
    println!("{}", output);
    process::exit(0);
}
