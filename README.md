# giallo.kak

Kakoune integration for the [`giallo`](https://github.com/getzola/giallo) TextMate highlighter.

[![Crates.io](https://img.shields.io/crates/v/giallo-kak.svg)](https://crates.io/crates/giallo-kak)
[![GitHub Release](https://img.shields.io/github/v/release/Yukaii/giallo.kak)](https://github.com/Yukaii/giallo.kak/releases)
[![License](https://img.shields.io/crates/l/giallo-kak.svg)](LICENSE)

## Demo

[![asciicast](https://asciinema.org/a/776697.svg)](https://asciinema.org/a/776697)

## Installation

### From crates.io

Install using cargo (requires Rust toolchain):

```bash
cargo install giallo-kak
```

This will compile and install the binary to `~/.cargo/bin/giallo-kak`. Make sure `~/.cargo/bin` is in your PATH.

To update to the latest version:

```bash
cargo install giallo-kak --force
```

### From GitHub Releases (Pre-built binaries)

Download pre-built binaries from the [Releases](https://github.com/Yukaii/giallo.kak/releases) page:

```bash
# Linux x86_64
curl -L -o giallo-kak https://github.com/Yukaii/giallo.kak/releases/latest/download/giallo-kak-x86_64-unknown-linux-gnu
cp giallo-kak ~/.local/bin/

# macOS
curl -L -o giallo-kak https://github.com/Yukaii/giallo.kak/releases/latest/download/giallo-kak-x86_64-apple-darwin
cp giallo-kak ~/.local/bin/

# Make executable
chmod +x ~/.local/bin/giallo-kak
```

### Build from Source

```sh
git clone https://github.com/Yukaii/giallo.kak.git
cd giallo.kak
cargo build --release
```

The binary will be at `target/release/giallo-kak`.

## Usage

Add to your Kakoune `kakrc`:

```kak
evaluate-commands %sh{
  giallo-kak init
}
```

Or source the script directly:

```kak
source /path/to/giallo.kak/rc/giallo.kak
```

2) Enable per-buffer (or it auto-enables for buffers with filetype):

```kak
giallo-enable
```

3) Set a theme:

```kak
giallo-set-theme kanagawa-wave
```

## CLI Commands

### List Available Themes

See all available themes (built-in and custom):

```bash
giallo-kak list-themes
```

For scripting (outputs one item per line):

```bash
giallo-kak list-themes --plain
```

### List Available Grammars

See all available grammars with their filetype mappings:

```bash
giallo-kak list-grammars
```

List grammar names only:

```bash
giallo-kak list-grammars --plain
```

This is useful for finding the correct grammar ID when configuring `language_map` in your config.

## Configuration

All configuration files live under `~/.config/giallo.kak/` (or `$XDG_CONFIG_HOME/giallo.kak/`):

```
~/.config/giallo.kak/
├── config.toml          # Main configuration file
├── grammars/            # Custom TextMate grammars
│   ├── terraform.json
│   └── my-language.json
└── themes/              # Custom TextMate themes
    └── my-theme.json
```

### Config File

The main configuration is in `config.toml`:

```toml
# Default theme
theme = "kanagawa-wave"

# Optional: paths to custom grammars and themes
grammars_path = "~/.config/giallo.kak/grammars"
themes_path = "~/.config/giallo.kak/themes"

# Filetype mapping
[language_map]
sh = "shellscript"
js = "javascript"
tf = "terraform"
hcl = "terraform"
```

Run `giallo-kak list-themes` to see all 55+ built-in themes and any custom themes you've added.

### Custom Grammars

giallo.kak supports dynamic loading of custom TextMate grammars without rebuilding.

**Quick Setup:**

1. **Create the grammars directory:**

```bash
mkdir -p ~/.config/giallo.kak/grammars
```

2. **Download grammar files** (.json or .plist) from:
   - VSCode extensions
   - [shikijs/textmate-grammars-themes](https://github.com/shikijs/textmate-grammars-themes)
   - Any TextMate/VSCode grammar repository

3. **Configure the grammars path** in your `config.toml` (see example above)

4. **Restart Kakoune** - grammars are loaded automatically on startup

**Grammar Aliases:**

Grammar files can define aliases in their metadata. For example, a `terraform.json` grammar with `"aliases": ["tf", "hcl"]` will automatically be available for those filetypes. You can also manually map filetypes using `language_map` in config.

### Custom Themes

Just like grammars, giallo.kak supports dynamic loading of custom TextMate themes without rebuilding.

**Quick Setup:**

1. **Create the themes directory:**

```bash
mkdir -p ~/.config/giallo.kak/themes
```

2. **Download theme files** (.json) from:
   - VSCode extensions
   - [shikijs/textmate-grammars-themes](https://github.com/shikijs/textmate-grammars-themes)
   - Any TextMate/VSCode theme repository

3. **Configure the themes path** in your `config.toml` (see example above)

4. **Use your custom theme:**

```kak
giallo-set-theme my-custom-theme
```

### Building Custom Registry (Advanced)

For maximum control, you can build a custom registry dump:

```rust
use giallo::Registry;

fn main() {
    let mut registry = Registry::default();

    // Load the builtin dump first (optional, for base grammars)
    let builtin = Registry::builtin().unwrap();
    // Or start fresh with Registry::default()

    // Add custom grammars from files
    for entry in std::fs::read_dir("/path/to/grammars").unwrap() {
        let path = entry.unwrap().path();
        registry.add_grammar_from_path(&path).unwrap();
    }

    // IMPORTANT: Link grammars to resolve dependencies (include/embed patterns)
    registry.link_grammars();

    // Add custom themes the same way
    registry.add_theme_from_path("/path/to/theme.json").unwrap();

    // Save the registry dump
    registry.save_to_file("custom.msgpack").unwrap();
}
```

Then replace the builtin dump and rebuild:

```bash
cp custom.msgpack /path/to/giallo.kak/builtin.msgpack
cargo build --release
```

The registry API provides these methods:
- `Registry::add_grammar_from_path(path)` - Add a grammar from a JSON/plist file
- `Registry::link_grammars()` - Resolve grammar dependencies (required after adding grammars)
- `Registry::add_theme_from_path(path)` - Add a theme from a JSON file
- `Registry::save_to_file(path)` - Save the compiled registry to a msgpack file

## Testing

### Running Tests

```bash
# All tests
cargo test --release

# E2E tests (requires Kakoune installed)
cargo test --release e2e

# Performance tests
cargo test --release performance
```

### E2E Tests

E2E tests launch real Kakoune instances to verify full integration:
- Buffer highlighting setup and teardown
- Theme changes and re-highlighting
- Server restart and reconnection
- Multiple buffer handling

Requirements:
- Kakoune must be installed (`which kak`)
- Tests create temporary sessions that are cleaned up automatically

### Performance Tests

Performance benchmarks track highlighting metrics:
- **Latency**: Time to highlight files of various sizes
  - Small (<100 lines): <300ms
  - Medium (1000 lines): <1000ms
  - Large (10000 lines): <5000ms
- **Memory usage**: Delta during highlighting (<150MB for large files)
- **Throughput**: Updates per second (>10 highlights/sec)
- **Theme/language comparison**: Benchmark different configurations

Generate performance test fixtures:
```bash
cargo run --bin generate_perf_fixtures
```

### Stress Tests

Stress tests simulate realistic multi-buffer editing scenarios with resource monitoring:

```bash
# Run stress tests (requires Kakoune, takes 2-3 minutes)
cargo test --release e2e_stress -- --test-threads=1
```

**Stress Test Scenarios:**

| Test | Description | Duration |
|------|-------------|----------|
| `stress_many_buffers` | Open 20 buffers simultaneously | ~15s |
| `stress_rapid_editing` | 100 rapid edits with rehighlighting | ~10s |
| `stress_continuous_updates` | Simulate typing session (300 updates) | ~30s |
| `stress_memory_stability` | Monitor memory over 60 seconds | ~60s |
| `stress_concurrent_typing` | Type in 5 buffers simultaneously | ~10s |
| `stress_large_file_editing` | Edit a 1000-line file | ~15s |

**Metrics Monitored:**
- CPU usage over time
- Memory consumption and growth
- Highlight latency under load
- Throughput (edits per second)

Example output:
```
=== Resource Usage Report ===
Duration: 60.0s
Samples: 60
Memory Baseline: 45.23 MB
Memory Average: 52.15 MB
Memory Max: 58.92 MB
Memory Growth: 30.3%
CPU Average: 8.2%
CPU Max: 15.6%
===========================
```

### CI/CD

This project uses GitHub Actions for automated builds and releases. Every push to main builds and tests on Linux, macOS, and Windows. Tag pushes (v\*) create GitHub releases with pre-built binaries and publish to crates.io.

The `builtin.msgpack` dump file is generated automatically during CI from the upstream [giallo](https://github.com/getzola/giallo) repository.

#### Performance Regression Detection

On every pull request, the CI runs performance tests and posts results as a comment:

```
## Performance Test Results

| Metric | Value | Threshold |
|--------|-------|-----------|
| Small file (100 lines) | 156ms | 300ms |
| Medium file (1000 lines) | 523ms | 1000ms |
| Large file (10000 lines) | 2451ms | 5000ms |
| Memory delta | 12MB | 150MB |

Status: ✅ Performance within acceptable thresholds
```

If performance regressions are detected, the comment will include:
- ⚠️ Warning indicators
- Which specific thresholds were exceeded
- Request for investigation

Performance thresholds are defined in `.github/perf-baseline.json`.

## Special Thanks

Special thanks to:

- **jdugan6240** from the Kakoune community Discord for sharing [giallo](https://github.com/getzola/giallo) and inspiring this Kakoune integration project
- [**kak-tree-sitter**](https://git.sr.ht/~hadronized/kak-tree-sitter) for the inspiration on the IPC architecture and FIFO-based communication pattern
- **OpenAI Codex 5.2** and **Kimi K2.5** for assistance in developing this project
