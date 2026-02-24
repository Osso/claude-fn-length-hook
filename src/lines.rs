//! Classifies lines for body-line counting purposes.
//! A line counts toward the body if it is not blank, comment-only, or brace-only.

/// Returns true if the line is considered countable (not blank/comment/brace).
/// `in_block_comment` is updated in place as we traverse.
pub fn is_countable(line: &str, in_block_comment: &mut bool) -> bool {
    let trimmed = line.trim();

    if *in_block_comment {
        if let Some(end) = trimmed.find("*/") {
            *in_block_comment = false;
            let after = trimmed[end + 2..].trim();
            return is_countable_simple(after);
        }
        return false;
    }

    // Strip inline block comments before checking
    let effective = strip_inline_block_comment(trimmed, in_block_comment);
    is_countable_simple(&effective)
}

/// Strip a block comment that opens and potentially closes on the same line.
/// Sets `in_block_comment` if the comment does not close on this line.
fn strip_inline_block_comment<'a>(
    line: &'a str,
    in_block_comment: &mut bool,
) -> std::borrow::Cow<'a, str> {
    if let Some(start) = line.find("/*") {
        let before = &line[..start];
        let rest = &line[start + 2..];
        if let Some(end) = rest.find("*/") {
            // Comment opens and closes on the same line; keep what's before + after
            let after = &rest[end + 2..];
            let combined = format!("{} {}", before.trim(), after.trim());
            return std::borrow::Cow::Owned(combined.trim().to_string());
        } else {
            *in_block_comment = true;
            return std::borrow::Cow::Owned(before.trim().to_string());
        }
    }
    std::borrow::Cow::Borrowed(line)
}

/// Check a pre-processed line (no open block comments) for countability.
fn is_countable_simple(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return false;
    }
    // Comment-only lines
    if t.starts_with("//") || t.starts_with('#') || t.starts_with('*') {
        return false;
    }
    // Brace-only lines: `{`, `}`, `};`, `},`
    if matches!(t, "{" | "}" | "};" | "}," | "});" | "})") {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(lines: &[&str]) -> usize {
        let mut in_bc = false;
        lines.iter().filter(|l| is_countable(l, &mut in_bc)).count()
    }

    #[test]
    fn blank_lines_excluded() {
        assert_eq!(count(&["", "  ", "\t"]), 0);
    }

    #[test]
    fn comment_lines_excluded() {
        assert_eq!(
            count(&["// comment", "  // indented", "* doc", "# hash"]),
            0
        );
    }

    #[test]
    fn brace_lines_excluded() {
        assert_eq!(count(&["{", "}", "};"]), 0);
    }

    #[test]
    fn code_lines_counted() {
        assert_eq!(count(&["let x = 1;", "return x;"]), 2);
    }

    #[test]
    fn block_comment_excluded() {
        assert_eq!(count(&["/* start", "middle", "end */"]), 0);
    }

    #[test]
    fn inline_block_comment_counted() {
        // Line has code before block comment
        assert_eq!(count(&["let x = /* comment */ 1;"]), 1);
    }
}
