# AGENTS.md - Coding Guidelines for giallo.kak

## Build, Test & Lint Commands

```bash
# Build the project
cargo build --release

# Run all tests
cargo test --release

# Run a specific test (example patterns)
cargo test --release <test_name>           # Run single test by name
cargo test --release fixture_tests         # Run fixture test suite
cargo test --release oneshot_terraform     # Run specific oneshot test
cargo test --release rust_keyword          # Run keyword tests matching pattern

# Build for specific target (CI)
cargo build --release --target x86_64-unknown-linux-gnu
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin

# There is no lint command configured - rely on cargo build warnings
```

## Code Style Guidelines

### Naming Conventions
- **Types**: PascalCase (`BufferContext`, `StyleKey`)
- **Functions/Variables**: snake_case (`highlight_and_send`, `buffer_contexts`)
- **Constants**: SCREAMING_SNAKE_CASE (`DEFAULT_THEME`)
- **Modules**: snake_case (`registry_loader`, `highlighting`)
- **Acronyms**: Treat as words (`HttpRequest` not `HTTPRequest`)

### Import Order
1. Standard library imports grouped by category (std::io, std::fs, std::sync)
2. Third-party crate imports
3. Internal module imports (`crate::config::Config`)

### Formatting
- No spaces after `{` in format strings: `format!("error: {err}")` not `format!("error: { err }")`
- Use `let-else` syntax for early returns when possible
- Keep functions focused and under ~100 lines
- Prefer `?` operator for error propagation
- Use `Arc<Mutex<T>>` for thread-shared mutable state

### Error Handling
- Use `io::Error` for most error types
- Log errors with context: `log::error!("action failed: {err}")`
- Use `eprintln!` for user-facing errors
- Pattern for error logging:
  ```rust
  match operation() {
      Ok(result) => result,
      Err(err) => {
          log::error!("operation failed: {err}");
          eprintln!("operation failed: {err}");
          return Err(err);
      }
  }
  ```

### Logging Levels
- `log::error!`: Unexpected failures that affect functionality
- `log::warn!`: Recoverable issues or fallbacks (e.g., grammar fallback to plain)
- `log::info!`: Major lifecycle events (server start/stop)
- `log::debug!`: Operation details (highlighting success, config loading)
- `log::trace!`: Detailed data dumps (command payloads)

### Module Organization
Keep related functionality in dedicated modules:
- `cli.rs`: Command-line parsing and help text
- `config.rs`: Configuration loading and path handling
- `fifo.rs`: FIFO file operations and buffer processing
- `highlight.rs`: Highlighting orchestration and Kakoune communication
- `highlighting.rs`: Style-to-face conversion logic
- `registry_loader.rs`: Custom grammar/theme loading
- `server.rs`: Main server loop and command handling
- `kakoune.rs`: Kakoune-specific utilities (quoting)

### Key Patterns

**Configuration Resolution:**
```rust
pub fn resolve_theme<'a>(&'a self, theme: &'a str) -> &'a str {
    if theme.is_empty() {
        self.theme.as_deref().unwrap_or(DEFAULT_THEME)
    } else {
        theme
    }
}
```

**Thread-Safe Context Updates:**
```rust
pub theme: Arc<Mutex<String>>,
// Access: let theme = ctx.theme.lock().unwrap().clone();
```

**Graceful Degradation:**
```rust
match registry.highlight(text, &options) {
    Ok(h) => h,
    Err(err) => {
        log::warn!("primary failed, trying fallback: {err}");
        // Fallback logic
    }
}
```

### Testing
- Use `cargo test --release` for accurate performance
- Integration tests in `tests/` directory
- Fixture tests for end-to-end validation
- Test helper functions for common setup (see `tests/fixture_tests.rs`)

### Dependencies
Check `Cargo.toml` before adding new crates. Current stack:
- `giallo`: Core TextMate highlighting engine
- `serde` + `serde_json` + `toml`: Serialization
- `log` + `simple_logger`: Logging
- `libc`: Low-level system calls
- `ctrlc`: Signal handling
- `which`: Executable detection

### Architecture Notes

**Main Components:**
1. **CLI Parser** (`cli.rs`): Parses arguments into `Mode` enum variants
2. **Server** (`server.rs`): Main command loop handling INIT, SET_THEME, PING, and oneshot H commands
3. **FIFO Handler** (`fifo.rs`): Channel-based reader/processor for buffer content via named pipes
4. **Highlighting Pipeline**:
   - `highlight.rs`: Orchestrates highlighting and sends results to Kakoune
   - `highlighting.rs`: Converts giallo styles to Kakoune face specifications
5. **Registry Loader** (`registry_loader.rs`): Loads custom grammars/themes from user config paths

**Communication Flow:**
1. Kakoune sends INIT command with session/buffer/token
2. Server creates FIFO per buffer and spawns reader thread
3. Kakoune writes buffer content to FIFO (with sentinel delimiter)
4. Reader thread sends complete content via channel to processor
5. Processor highlights text and sends Kakoune commands back via `kak -p`

**Key Data Structures:**
- `BufferContext`: Per-buffer state (session, buffer name, sentinel, lang/theme Arc<Mutex>)
- `StyleKey`: Hashable representation of text style for face deduplication
- `Config`: TOML-loaded configuration with language mapping and paths
