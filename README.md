# claude-fn-length-hook

A small Rust hook binary that blocks oversized Rust and PHP functions before a file write is accepted.

## What it enforces

- Rust function bodies: maximum 30 countable lines
- Rust files: maximum 750 lines
- PHP function bodies: maximum 30 countable lines for new functions
- PHP legacy functions already over the limit: allowed only if they do not grow

For Rust file length, legacy files already over 750 lines are allowed only when an edit does not increase the file length.

## Supported hook payloads

The binary reads JSON from stdin and currently supports these tool payloads:

- `Edit`
- `Write`

Expected fields:

```json
{
  "tool_name": "Edit",
  "tool_input": {
    "file_path": "src/main.rs",
    "old_string": "old text",
    "new_string": "new text"
  }
}
```

```json
{
  "tool_name": "Write",
  "tool_input": {
    "file_path": "src/main.rs",
    "content": "full replacement file contents"
  }
}
```

If `tool_name` is omitted, the hook treats the payload as `Edit`.

## Output

When the change is allowed, the program exits without printing a blocking decision.

When the change violates a limit, it prints JSON like this:

```json
{
  "decision": "block",
  "reason": "Function body exceeds 30-line limit in main.rs:\n  - foo (line 12): 34 body lines\nExtract logic into well-named helper functions."
}
```

## Build

```bash
cargo build --release
```

The binary will be available at:

```text
target/release/claude-fn-length-hook
```

## Quick test

```bash
printf '%s\n' '{"tool_name":"Write","tool_input":{"file_path":"src/main.rs","content":"fn main() {\n    println!(\"ok\");\n}\n"}}' | cargo run --quiet
```

## Notes

- Only `.rs` and `.php` files are checked
- The hook simulates the post-edit file content from the incoming payload instead of assuming the file was already written to disk
- Rust and PHP parsers count only body lines considered meaningful by the project rules
