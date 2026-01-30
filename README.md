# giallo.kak

Kakoune integration for the [`giallo`](https://github.com/getzola/giallo) TextMate highlighter.

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
# 1. Clone this repository
git clone https://github.com/Yukaii/giallo.kak.git
cd giallo.kak

# 2. Clone giallo dependency (required for the builtin dump)
git clone https://github.com/getzola/giallo.git ../giallo

# 3. Generate the builtin dump (or use the provided one)
cd ../giallo
npm install
node scripts/extract-grammar-metadata.js
cargo run --release --bin=build-registry --features=tools

# 4. Copy dump back and build
cd ../giallo.kak
cp ../giallo/builtin.msgpack .
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

## Custom Grammars

giallo.kak supports dynamic loading of custom TextMate grammars without rebuilding. Simply place your grammar files in a directory and configure the path in your config.

### Quick Setup

1. **Create a grammars directory**:

```bash
mkdir -p ~/.config/giallo.kak/grammars
```

2. **Download grammar files** (.json or .plist) from:
   - VSCode extensions
   - [shikijs/textmate-grammars-themes](https://github.com/shikijs/textmate-grammars-themes)
   - Any TextMate/VSCode grammar repository

3. **Configure the grammars path** in `~/.config/giallo.kak/config.toml`:

```toml
# Path to your custom grammars directory
grammars_path = "~/.config/giallo.kak/grammars"

# Map Kakoune filetypes to grammar IDs
[language_map]
tf = "terraform"
hcl = "terraform"
```

4. **Restart Kakoune** - grammars are loaded automatically on startup

### Advanced: Custom Grammar Aliases

Grammar files can define aliases in their metadata. For example, a `terraform.json` grammar with `"aliases": ["tf", "hcl"]` will automatically be available for those filetypes. You can also manually map filetypes using `language_map` in config.

## Custom Themes

Just like grammars, giallo.kak supports dynamic loading of custom TextMate themes without rebuilding.

### Quick Setup

1. **Create a themes directory**:

```bash
mkdir -p ~/.config/giallo.kak/themes
```

2. **Download theme files** (.json) from:
   - VSCode extensions
   - [shikijs/textmate-grammars-themes](https://github.com/shikijs/textmate-grammars-themes)
   - Any TextMate/VSCode theme repository

3. **Configure the themes path** in `~/.config/giallo.kak/config.toml`:

```toml
# Path to your custom themes directory
themes_path = "~/.config/giallo.kak/themes"
```

4. **Use your custom theme**:

```kak
giallo-set-theme my-custom-theme
```

### Building Custom Registry (Advanced)

For maximum control, you can still build a custom registry dump:

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

## Config

Config file path:

- `$XDG_CONFIG_HOME/giallo.kak/config.toml`, or
- `~/.config/giallo.kak/config.toml`

Example config:

```toml
# Default theme
theme = "kanagawa-wave"

# Filetype mapping
[language_map]
sh = "shellscript"
js = "javascript"
```

Available themes: `kanagawa-wave`, `kanagawa-dragon`, `kanagawa-lotus`, `catppuccin-frappe`, `catppuccin-mocha`, `catppuccin-latte`, `tokyo-night`, `dracula`, `gruvbox-dark-medium`, and many more (55 total themes).

See `docs/config.example.toml` for a fuller template.

## CI/CD

This project uses GitHub Actions for automated builds and releases:

- **Every push to main**: Builds and tests on Linux, macOS, and Windows
- **Tag pushes (v\*)**: Creates GitHub releases with pre-built binaries

The CI process automatically:
1. Clones the upstream giallo repository
2. Generates the `builtin.msgpack` dump
3. Builds for multiple platforms
4. Creates releases with binaries attached

## Notes

- The [`giallo`](https://github.com/getzola/giallo) builtin dump (`builtin.msgpack`) is required for `Registry::builtin()`.
- The dump is generated in the giallo repository via:

```sh
cd ../giallo
just generate-dump
```

## Special Thanks

Special thanks to:

- **jdugan6240** from the Kakoune community Discord for sharing [giallo](https://github.com/getzola/giallo) and inspiring this Kakoune integration project
- [**kak-tree-sitter**](https://git.sr.ht/~hadronized/kak-tree-sitter) for the inspiration on the IPC architecture and FIFO-based communication pattern
- **OpenAI Codex 5.2** and **Kimi K2.5** for assistance in developing this project
