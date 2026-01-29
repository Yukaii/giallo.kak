# giallo.kak

Kakoune integration for the `giallo` TextMate highlighter.

## Build

```sh
cargo build
```

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

2) Enable per-buffer:

```kak
giallo-enable
```

## Config

Config file path:

- `$XDG_CONFIG_HOME/giallo.kak/config.toml`, or
- `~/.config/giallo.kak/config.toml`

Example config:

```toml
# Default theme
# theme = "catppuccin-frappe"

# Filetype mapping
# [language_map]
# sh = "shellscript"
# js = "javascript"
```

See `docs/config.example.toml` for a fuller template.

## Notes

- The `giallo` builtin dump (`builtin.msgpack`) is required for `Registry::builtin()`.
- The dump is generated in `../giallo` via:

```sh
cd ../giallo
just generate-dump
```
