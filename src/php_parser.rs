use crate::lines::is_countable;
use std::collections::HashMap;

pub const BODY_LIMIT: usize = 30;

#[derive(Debug)]
pub struct Violation {
    pub name: String,
    pub line: usize,
    pub body_lines: usize,
    pub old_body_lines: Option<usize>, // None = new function
}

/// Parse PHP source and check against legacy-aware limits.
pub fn check(source: &str, old_source: Option<&str>) -> Vec<Violation> {
    let fns = parse_functions(source);
    let old_fns = old_source.map(parse_functions).unwrap_or_default();
    collect_violations(fns, old_fns)
}

#[derive(Debug)]
struct FnInfo {
    line: usize,
    body_lines: usize,
}

fn collect_violations(
    fns: HashMap<String, FnInfo>,
    old_fns: HashMap<String, FnInfo>,
) -> Vec<Violation> {
    let mut violations = Vec::new();
    for (name, info) in &fns {
        let old_len = old_fns.get(name).map(|f| f.body_lines);
        let limit = old_len.map(|l| l.max(BODY_LIMIT)).unwrap_or(BODY_LIMIT);
        if info.body_lines > limit {
            violations.push(Violation {
                name: name.clone(),
                line: info.line,
                body_lines: info.body_lines,
                old_body_lines: old_len,
            });
        }
    }
    violations.sort_by_key(|v| v.line);
    violations
}

/// Parse all functions from PHP source, returning name -> FnInfo.
fn parse_functions(source: &str) -> HashMap<String, FnInfo> {
    let lines: Vec<&str> = source.lines().collect();
    let mut result = HashMap::new();
    let mut i = 0;
    let mut in_bc = false;

    while i < lines.len() {
        let line = lines[i];
        if let Some((name, fn_line)) = extract_php_fn_name(line, i, in_bc) {
            // Find opening brace
            if let Some(open_idx) = find_opening_brace(&lines, i) {
                let (body_lines, close_idx) = count_body(&lines, open_idx);
                result.insert(
                    name,
                    FnInfo {
                        line: fn_line,
                        body_lines,
                    },
                );
                i = close_idx + 1;
                continue;
            }
        }
        update_block_comment_state(line, &mut in_bc);
        i += 1;
    }
    result
}

/// Extract PHP function name from a line containing `function <name>`.
fn extract_php_fn_name(line: &str, line_idx: usize, in_bc: bool) -> Option<(String, usize)> {
    if in_bc {
        return None;
    }
    let trimmed = line.trim();
    if trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with("/*") {
        return None;
    }
    let pos = find_keyword_position(trimmed, "function ")?;
    let after = &trimmed[pos + 9..];
    // Skip `&` for reference returns
    let after = after.trim_start_matches('&').trim();
    let name_end = after
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(after.len());
    if name_end == 0 {
        return None;
    }
    Some((after[..name_end].to_string(), line_idx + 1))
}

fn find_keyword_position(line: &str, keyword: &str) -> Option<usize> {
    let keyword_pos = line.find(keyword)?;
    let stop_pos = ["//", "/*", "#", "\"", "'"]
        .into_iter()
        .filter_map(|marker| line.find(marker))
        .min()
        .unwrap_or(line.len());

    if keyword_pos < stop_pos {
        Some(keyword_pos)
    } else {
        None
    }
}

/// Find the `{` that opens a function body, scanning up to 10 lines forward.
fn find_opening_brace(lines: &[&str], start: usize) -> Option<usize> {
    (start..lines.len().min(start + 10)).find(|&i| lines[i].contains('{'))
}

/// Count countable body lines inside the brace block starting at `open_line`.
fn count_body(lines: &[&str], open_line: usize) -> (usize, usize) {
    let mut depth = 0i32;
    let mut body_count = 0usize;
    let mut in_bc = false;
    let mut in_string = false;
    let mut string_char = '"';

    for (offset, line) in lines[open_line..].iter().enumerate() {
        let idx = open_line + offset;
        update_depth(
            line,
            &mut depth,
            &mut in_bc,
            &mut in_string,
            &mut string_char,
        );

        if offset > 0 && depth > 0 {
            let mut bc_clone = in_bc;
            if is_countable(line, &mut bc_clone) {
                body_count += 1;
            }
        }

        if depth == 0 && offset > 0 {
            return (body_count, idx);
        }
    }
    (body_count, lines.len().saturating_sub(1))
}

fn update_depth(
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
            break;
        } else if chars[j] == '#' {
            break; // PHP single-line comment
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

fn update_block_comment_state(line: &str, in_bc: &mut bool) {
    let mut dummy = 0i32;
    let mut dummy_str = false;
    let mut dummy_char = '"';
    update_depth(line, &mut dummy, in_bc, &mut dummy_str, &mut dummy_char);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn php_function_with_counted_lines(name: &str, count: usize) -> String {
        let body = vec!["    $x++;"; count].join("\n");
        format!("function {}() {{\n{}\n}}", name, body)
    }

    #[test]
    fn allows_legacy_php_function_to_stay_same_size_over_limit() {
        let source = php_function_with_counted_lines("legacy_demo", BODY_LIMIT + 1);

        let violations = check(&source, Some(&source));

        assert!(violations.is_empty());
    }

    #[test]
    fn blocks_legacy_php_function_when_it_grows_past_previous_size() {
        let old_source = php_function_with_counted_lines("legacy_demo", BODY_LIMIT + 1);
        let new_source = php_function_with_counted_lines("legacy_demo", BODY_LIMIT + 2);

        let violations = check(&new_source, Some(&old_source));

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].name, "legacy_demo");
        assert_eq!(violations[0].old_body_lines, Some(BODY_LIMIT + 1));
    }

    #[test]
    fn ignores_function_keyword_inside_top_level_string_literal() {
        assert_eq!(
            extract_php_fn_name(r#"$msg = "function fake";"#, 0, false),
            None
        );
    }
}
