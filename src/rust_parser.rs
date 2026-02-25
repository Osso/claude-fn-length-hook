use crate::lines::is_countable;

pub const BODY_LIMIT: usize = 30;
pub const FILE_LINE_LIMIT: usize = 750;

#[derive(Debug)]
pub struct Violation {
    pub name: String,
    pub line: usize,
    pub body_lines: usize,
}

pub struct RustViolations {
    pub fn_violations: Vec<Violation>,
    pub file_lines: usize,
}

/// Parse a Rust source file and return function body violations.
pub fn check(source: &str) -> RustViolations {
    let lines: Vec<&str> = source.lines().collect();
    let file_lines = lines.len();
    let fn_violations = find_fn_violations(&lines);
    RustViolations {
        fn_violations,
        file_lines,
    }
}

/// Walk lines to find `fn` declarations and measure their body lengths.
fn find_fn_violations(lines: &[&str]) -> Vec<Violation> {
    let mut violations = Vec::new();
    let mut in_block_comment = false;
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        // Track block comments so we don't pick up `fn` inside them
        if let Some(viol) = try_parse_fn(lines, &mut i, &mut in_block_comment) {
            if viol.body_lines > BODY_LIMIT {
                violations.push(viol);
            }
        } else {
            update_block_comment_state(line, &mut in_block_comment);
            i += 1;
        }
    }
    violations
}

/// If the current line (at index `i`) contains a top-level `fn` declaration,
/// scan its body and return a Violation (even if within limit, caller filters).
/// Advances `i` past the closing brace.
fn try_parse_fn(lines: &[&str], i: &mut usize, in_block_comment: &mut bool) -> Option<Violation> {
    let line = lines[*i];

    if *in_block_comment {
        update_block_comment_state(line, in_block_comment);
        *i += 1;
        return None;
    }

    let (name, fn_line) = extract_fn_name(line, *i)?;

    // Find opening brace of function body (may be on same line or next few)
    let open_idx = find_opening_brace(lines, *i)?;

    let (body_lines, close_idx) = count_body(lines, open_idx);

    *i = close_idx + 1;
    Some(Violation {
        name,
        line: fn_line + 1,
        body_lines,
    })
}

/// Extract function name from a line containing `fn <name>`.
/// Returns (name, 0-based line index) or None if no match.
fn extract_fn_name(line: &str, line_idx: usize) -> Option<(String, usize)> {
    let trimmed = line.trim();
    // Skip comments
    if trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with("/*") {
        return None;
    }
    let pos = trimmed.find("fn ")?;
    let after = &trimmed[pos + 3..];
    let name_end = after
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(after.len());
    if name_end == 0 {
        return None;
    }
    Some((after[..name_end].to_string(), line_idx))
}

/// Scan forward from `start` to find the `{` that opens the function body.
fn find_opening_brace(lines: &[&str], start: usize) -> Option<usize> {
    (start..lines.len().min(start + 10)).find(|&i| lines[i].contains('{'))
}

/// Count body lines inside braces starting at `open_line`.
/// Returns (countable body lines, index of closing brace line).
fn count_body(lines: &[&str], open_line: usize) -> (usize, usize) {
    let mut depth = 0i32;
    let mut body_count = 0usize;
    let mut in_bc = false;
    let mut in_string = false;
    let mut string_char = '"';

    for (offset, line) in lines[open_line..].iter().enumerate() {
        let idx = open_line + offset;
        update_string_and_depth(
            line,
            &mut depth,
            &mut in_bc,
            &mut in_string,
            &mut string_char,
        );

        // Count lines that are inside the body (depth > 0 before this line opened)
        if offset > 0 && depth > 0 && is_countable(line, &mut in_bc.clone()) {
            body_count += 1;
        }

        if depth == 0 && offset > 0 {
            return (body_count, idx);
        }
    }
    (body_count, lines.len().saturating_sub(1))
}

/// Update brace depth and string/block-comment state for one line.
fn update_string_and_depth(
    line: &str,
    depth: &mut i32,
    in_bc: &mut bool,
    in_string: &mut bool,
    string_char: &mut char,
) {
    let chars: Vec<char> = line.chars().collect();
    let mut j = 0;
    while j < chars.len() {
        if *in_bc {
            if j + 1 < chars.len() && chars[j] == '*' && chars[j + 1] == '/' {
                *in_bc = false;
                j += 2;
                continue;
            }
        } else if *in_string {
            if chars[j] == '\\' {
                j += 2;
                continue;
            }
            if chars[j] == *string_char {
                *in_string = false;
            }
        } else if j + 1 < chars.len() && chars[j] == '/' && chars[j + 1] == '*' {
            *in_bc = true;
            j += 2;
            continue;
        } else if j + 1 < chars.len() && chars[j] == '/' && chars[j + 1] == '/' {
            break; // rest of line is comment
        } else if chars[j] == '"' || chars[j] == '\'' {
            *in_string = true;
            *string_char = chars[j];
        } else if chars[j] == '{' {
            *depth += 1;
        } else if chars[j] == '}' {
            *depth -= 1;
        }
        j += 1;
    }
}

/// Update block-comment state for a line (used outside fn parsing).
fn update_block_comment_state(line: &str, in_bc: &mut bool) {
    let mut dummy_depth = 0i32;
    let mut dummy_str = false;
    let mut dummy_char = '"';
    update_string_and_depth(
        line,
        &mut dummy_depth,
        in_bc,
        &mut dummy_str,
        &mut dummy_char,
    );
}
