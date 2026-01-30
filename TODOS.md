# giallo.kak Implementation Status

This project is **feature complete** and actively maintained. The TODOs below reflect what's been implemented vs remaining polish items.

## Completed

### Repo Structure
- [x] Cargo workspace with giallo Kakoune server (Rust binary)
- [x] `rc/giallo.kak` - Kakoune plugin script
- [x] `docs/config.example.toml` - Configuration examples

### Kakoune Integration
- [x] Buffer options: `giallo_lang`, `giallo_theme`, `giallo_hl_ranges`, `giallo_buf_fifo_path`, `giallo_buf_sentinel`
- [x] `ranges` highlighter integration
- [x] Hooks: BufOpen, BufReload, InsertChar, NormalIdle
- [x] Filetype-to-language mapping
- [x] Commands: `giallo-enable/disable`, `giallo-rehighlight`, `giallo-set-theme`

### Server & IPC
- [x] Request/response protocol (line-delimited)
- [x] Per-buffer FIFO creation
- [x] Server loop with highlight dispatch
- [x] Session init handshake
- [x] Global FIFO server from Kakoune

### Highlighting
- [x] `giallo::Registry` loading (builtin dump + grammar link)
- [x] Request parsing (language, theme, buffer text)
- [x] `Registry::highlight` integration
- [x] Token-to-Kakoune-ranges conversion
- [x] Style → face name caching per theme
- [x] Kakoune `set-face` command generation
- [x] Font style mapping (bold/italic/underline/strikethrough)

### Configuration
- [x] Config file: `~/.config/giallo.kak/config.toml`
- [x] Theme and language map overrides
- [x] Default theme fallback
- [x] Unknown language fallback (plain text)
- [x] Custom grammars and themes loading
- [x] CLI commands: `list-grammars`, `list-themes`

### Performance & Quality
- [x] Debounced updates (50-100ms)
- [x] Comprehensive logging (error/warn/info/debug/trace levels)
- [x] Error handling with graceful fallbacks
- [x] Multi-platform builds (Linux, macOS)
- [x] CI/CD with automated releases
- [x] Crates.io publishing (stable and prerelease)

### Documentation
- [x] README with installation, usage, configuration
- [x] CLI commands documented
- [x] Custom grammar/theme setup guide
- [x] Sample configuration file

## Remaining Enhancements

- [ ] Max buffer size guard (prevent OOM on huge files)
- [ ] Cache compiled themes/face sets across sessions
- [ ] Troubleshooting guide for common issues

## Current Version

See [Cargo.toml](./Cargo.toml) for version. Released versions available on:
- [crates.io](https://crates.io/crates/giallo-kak)
- [GitHub Releases](https://github.com/Yukaii/giallo.kak/releases)
