# Integration and Fixture Tests

This directory contains integration tests and fixtures for giallo.kak highlighting functionality.

## Test Structure

### Integration Tests

- **`string_keyword_highlighting.rs`**: Tests for string and keyword highlighting across multiple languages
  - Tests for Rust, JavaScript, Python, Go, and TypeScript
  - Tests for various string types (single-quoted, double-quoted, template literals, raw strings, etc.)
  - Tests for keywords in different contexts (control flow, loops, functions, classes, etc.)
  - Tests for edge cases (empty strings, escaped characters, multiline strings)

- **`fixture_tests.rs`**: Tests that use sample code fixtures
  - Loads full sample files from `tests/fixtures/`
  - Tests highlighting quality and quantity
  - Tests multiple themes
  - Tests specific string and keyword highlighting in context

- **`terraform_oneshot.rs`**: Tests for custom grammar support (Terraform example)

### Fixtures

The `fixtures/` directory contains sample code files for testing:

- **`rust_sample.rs`**: Comprehensive Rust code sample
  - String literals (regular, multiline, raw)
  - Keywords (fn, let, if, else, return, for, in, match, break, continue, struct, impl, async, await)
  - Comments

- **`javascript_sample.js`**: Comprehensive JavaScript code sample
  - String types (double-quoted, single-quoted, template literals)
  - Keywords (const, let, var, if, else, for, while, break, continue, function, async, await, class, export, import)
  - Comments

- **`python_sample.py`**: Comprehensive Python code sample
  - String types (regular, multiline, raw, f-strings)
  - Keywords (if, elif, else, for, while, break, continue, def, async, await, class, try, except, finally, with, import)
  - Comments

- **`go_sample.go`**: Comprehensive Go code sample
  - String types (regular, raw/multiline, runes)
  - Keywords (package, import, func, if, else, for, switch, case, default, break, continue, defer, go, select, struct, interface, const, var, type, map, range)
  - Comments

## Running Tests

Run all tests:
```bash
cargo test
```

Run only integration tests:
```bash
cargo test --test string_keyword_highlighting
cargo test --test fixture_tests
```

Run specific test:
```bash
cargo test rust_string_highlighting
cargo test fixture_rust_sample
```

Run with output:
```bash
cargo test -- --nocapture
```

## Test Coverage

The tests verify:

1. **Highlighting Output Format**
   - Face definitions are generated (`set-face global`)
   - Highlight ranges are generated (`set-option buffer giallo_hl_ranges`)
   - Face definitions use correct format (rgb colors)

2. **String Highlighting**
   - Single-quoted strings
   - Double-quoted strings
   - Template literals / f-strings
   - Raw strings
   - Multiline strings
   - Empty strings
   - Strings with escape sequences
   - Nested quotes in strings

3. **Keyword Highlighting**
   - Declaration keywords (let, const, var, fn, def, func)
   - Control flow keywords (if, else, elif, switch, case)
   - Loop keywords (for, while, loop, in, range)
   - Jump keywords (break, continue, return)
   - Async keywords (async, await)
   - Class/struct keywords (class, struct, interface, impl)
   - Import/export keywords
   - Exception handling keywords (try, catch, except, finally)
   - Other language-specific keywords

4. **Multi-Language Support**
   - Rust
   - JavaScript
   - TypeScript
   - Python
   - Go

5. **Multi-Theme Support**
   - catppuccin-frappe
   - catppuccin-mocha
   - tokyo-night
   - dracula
   - kanagawa-wave

## Known Issues

If you're experiencing issues with strings or keywords not being highlighted:

1. Check that the grammar is loaded correctly in the registry
2. Verify the language name matches the grammar name (use language_map in config)
3. Check that `registry.link_grammars()` is called after adding grammars
4. Run tests with `--verbose` flag to see debug output

## Adding New Tests

To add a new test:

1. **For integration tests**: Add a new test function to `string_keyword_highlighting.rs` or `fixture_tests.rs`
2. **For new fixtures**:
   - Create a new sample file in `tests/fixtures/`
   - Add a corresponding test in `fixture_tests.rs` to load and verify it

Example test:
```rust
#[test]
fn my_new_test() {
    let code = r#"your code here"#;
    let output = run_oneshot_highlight("language", "theme", code);
    assert_valid_highlighting(&output, "test_name");
    assert_has_ranges(&output);
}
```
