# giallo.kak

Kakoune integration for the [`giallo`](https://github.com/getzola/giallo) TextMate highlighter.

## Installation

### From GitHub Releases (Recommended)

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

1) Source the Kakoune script:

```kak
source /path/to/giallo.kak/rc/giallo.kak
```

Or print the embedded rc from the binary:

```kak
# From a shell, then source in Kakoune:
# giallo-kak --print-rc
```

2) Enable per-buffer (or it auto-enables for buffers with filetype):

```kak
giallo-enable
```

3) Set a theme:

```kak
giallo-set-theme kanagawa-wave
```

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
