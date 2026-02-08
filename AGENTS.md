# AGENTS.md - Coding Guidelines for giallo.kak

## Build, Test & Lint Commands

```bash
# Build the project (edition 2021, MSRV 1.74)
cargo build --release

# Run all tests (use --release for accurate perf benchmarks)
cargo test --release

# Run a specific test by name substring
cargo test --release <test_name>
cargo test --release fixture_tests         # Run fixture test suite
cargo test --release oneshot_terraform     # Run specific oneshot test
cargo test --release rust_keyword          # Run keyword tests matching pattern

# Cross-compile targets (CI matrix)
cargo build --release --target x86_64-unknown-linux-gnu
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin

# No separate lint command - rely on cargo build warnings
```

## Code Style Guidelines

### Naming Conventions
- **Types/Structs/Enums**: PascalCase (`BufferContext`, `StyleKey`, `FaceDef`, `Mode`)
- **Functions/Variables**: snake_case (`highlight_and_send`, `buffer_contexts`)
- **Constants**: SCREAMING_SNAKE_CASE (`DEFAULT_THEME`, `PLAIN_GRAMMAR_NAME`)
- **Modules**: snake_case, one file per module (`registry_loader`, `highlighting`)
- **Acronyms**: Treat as words (`HttpRequest` not `HTTPRequest`)

### Import Order
1. Standard library (`std::io`, `std::fs`, `std::sync`, `std::collections`)
2. Third-party crates (`giallo`, `log`, `serde`)
3. Internal modules (`crate::config::Config`, `crate::fifo`)

Separate each group with a blank line. Group related std imports on one line when short.

### Formatting
- No spaces inside `{}` in format strings: `format!("error: {err}")` not `format!("error: { err }")`
- Use `let-else` for early returns: `let Some(x) = val else { continue; };`
- Keep functions focused and under ~100 lines
- Prefer `?` operator for error propagation over manual match
- Use `Arc<Mutex<T>>` for thread-shared mutable state, `Arc<AtomicBool>` for flags
- Derive traits in order: `Clone, Debug, PartialEq, Eq, Hash`

### Error Handling
- Use `io::Error` as the primary error type; return `io::Result<T>`
- Log errors with context before propagating: `log::error!("action failed: {err}")`
- Use `eprintln!` for user-facing error messages
- Pattern:
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
- For non-critical failures, warn and fall back gracefully (e.g., fallback to plain grammar)

### Logging Levels
- `log::error!`: Unexpected failures affecting functionality
- `log::warn!`: Recoverable issues or fallbacks
- `log::info!`: Major lifecycle events (server start/stop, cleanup)
- `log::debug!`: Operation details (highlighting success, config loading)
- `log::trace!`: Detailed data dumps (full command payloads)

### Module Organization
Each module is a single file in `src/`. Keep related functionality together:
- `cli.rs`: Command-line argument parsing into `Mode` enum
- `config.rs`: TOML configuration loading and path resolution
- `commands.rs`: `list-grammars` / `list-themes` output formatting
- `fifo.rs`: Named pipe (FIFO) creation and buffer content reader threads
- `highlight.rs`: Highlighting orchestration and Kakoune command dispatch
- `highlighting.rs`: Style-to-face conversion and Kakoune command building
- `kakoune.rs`: Kakoune-specific utilities (shell quoting)
- `registry_loader.rs`: Custom grammar/theme loading from user config paths
- `server.rs`: Main server loop handling INIT, SET_THEME, PING, oneshot H commands
- `server_resources.rs`: Signal handling, graceful shutdown, temp dir cleanup (RAII)

### Key Patterns

**Thread-safe shared registry** (70-90MB, load once):
```rust
let registry = Arc::new(Registry::builtin().unwrap());
// Clone Arc for each thread, not the registry itself
let thread_registry = Arc::clone(&registry);
std::thread::spawn(move || { thread_registry.highlight(&text, &options); });
```

**Per-buffer mutable state** via `Arc<Mutex<T>>`:
```rust
pub theme: Arc<Mutex<String>>,
// Access: let theme = ctx.theme.lock().unwrap().clone();
```

**Graceful degradation** on highlight failure:
```rust
match registry.highlight(&text, &options) {
    Ok(h) => h,
    Err(err) => {
        log::warn!("failed for lang={lang}: {err}");
        // Retry with PLAIN_GRAMMAR_NAME as fallback
        registry.highlight(&text, &fallback_options)?
    }
}
```

**RAII cleanup** via Drop trait (see `server_resources.rs`):
```rust
impl Drop for ServerResources {
    fn drop(&mut self) { cleanup_base_dir(&self.base_dir); }
}
```

### Testing
- All tests are integration tests in `tests/` (no unit tests in `src/`)
- Tests use oneshot mode: spawn `giallo-kak --oneshot` as a subprocess
- Binary path via `env!("CARGO_BIN_EXE_giallo-kak")`
- Test helpers: `make_temp_dir()`, `write_config()`, `run_oneshot_highlight()`
- Performance tests have thresholds: small <500ms, medium <1s, large <5s
- E2E tests require a real Kakoune instance

### Dependencies
Check `Cargo.toml` before adding new crates. Current stack:
- `giallo` (with "dump" feature): Core TextMate highlighting engine
- `serde` + `serde_json` + `toml`: Serialization / config parsing
- `log` + `simple_logger`: Logging infrastructure
- `libc`: Low-level system calls (FIFO creation)
- `ctrlc`: Signal handling for graceful shutdown
- `which`: Executable path detection
- Dev: `sysinfo`, `tempfile`, `rand`

## Architecture

**Communication Flow:**
1. Kakoune sources `rc/giallo.kak` which defines hooks and commands
2. Server starts via `giallo-kak --fifo <path>` (background process)
3. `INIT session buffer token lang theme` creates a per-buffer FIFO + reader thread
4. Kakoune hooks write buffer content to the per-buffer FIFO (with sentinel delimiter)
5. Reader thread parses content, highlights via `giallo::Registry`
6. Results sent back to Kakoune as `set-face` / `set-option` commands via `kak -p session`

**Key Data Structures:**
- `Mode`: Enum of operating modes (Stdio, Oneshoot, Fifo, ListGrammars, etc.)
- `BufferContext`: Per-buffer state (session, buffer name, sentinel, lang/theme)
- `StyleKey`: Hashable style representation for face deduplication
- `Config`: TOML-loaded config with language mapping and custom grammar/theme paths
