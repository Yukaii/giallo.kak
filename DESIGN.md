# giallo.kak design

## Summary
Integrate the `giallo` TextMate highlighter into Kakoune by adding a small server that turns giallo token styling into Kakoune `ranges` highlighters. The design mirrors the session/FIFO approach used by `kak-tree-sitter` while keeping a simple Kakoune-side setup.

## Goals
- Provide TextMate-based syntax highlighting in Kakoune using `giallo`.
- Reuse giallo themes and grammars (builtin dump or user-provided).
- Use Kakoune `ranges` highlighter output, compatible with existing Kakoune runtime.
- Keep integration modular: a Kakoune script + a Rust server.
- Keep performance acceptable for typical file sizes.

## Non-goals
- Replace `kak-tree-sitter` or provide tree-sitter features (selections, indents, queries).
- Implement full incremental parsing in the first version.
- Rework Kakoune colorscheme logic; we use giallo themes directly.

## Key references
- `giallo` provides `Registry::builtin()` and `HighlightOptions` with `ThemeVariant` and a per-line token output (`HighlightedCode`).
- `kak-tree-sitter` already defines a working pattern for Kakoune integration: a server, FIFO updates, and `ranges` output.

## High-level architecture

### Components
1) **giallo-kak server (Rust binary)**
   - Owns a `giallo::Registry` and theme cache.
   - Receives buffer snapshots (full text) and language/theme choices.
   - Produces Kakoune `ranges` with face names and emits Kakoune commands.
   - Reuses the `ranges` update strategy from `kak-tree-sitter` (timestamped buffer option with ranges).

2) **Kakoune script (giallo.kak)**
   - Sets up highlighters and hooks for buffer updates.
   - Sends buffer content to the server (FIFO or similar IPC).
   - Holds buffer options like `giallo_lang`, `giallo_theme`, `giallo_hl_ranges`.

### Data flow
1) Kakoune detects a buffer that should be highlighted.
2) Kakoune script sends a snapshot (buffer content + filetype) to the server.
3) Server highlights via giallo and maps styles to Kakoune faces.
4) Server sends back Kakoune commands to:
   - define or update faces for the selected theme,
   - set buffer option `giallo_hl_ranges` with the encoded ranges.
5) Kakoune `ranges` highlighter displays the highlights.

## Kakoune integration details

### Highlighter setup
- A buffer-local ranges highlighter:
  - `add-highlighter -override buffer/giallo ranges giallo_hl_ranges`
- When a buffer is enabled, remove any default syntax highlighter if configured.

### Buffer options
- `giallo_lang` (string): giallo language id.
- `giallo_theme` (string): theme name.
- `giallo_hl_ranges` (string): the ranges highlighter payload.
- `giallo_buf_fifo_path` (string): FIFO path for buffer updates (if using FIFO).
- `giallo_buf_sentinel` (string): sentinel token for fifo framing (if needed).

### Hooks
- `BufOpen`/`BufReload`/`BufWritePost`/`InsertChar` (or `NormalIdle`) trigger updates.
- `BufSetOption filetype` maps Kakoune filetype to giallo language.
- `WinSetOption` to refresh on colorscheme change (optional).

## Server responsibilities

### Session and IPC
- Reuse `kak-tree-sitter`'s pattern: a per-buffer FIFO created by the server and injected into Kakoune.
- The client writes buffer snapshots to the FIFO; the server reads, highlights, and responds.

### Highlighting pipeline
1) Resolve language and theme.
2) `Registry::highlight(code, options)` returns `HighlightedCode` with per-line tokens.
3) Convert tokens into Kakoune `ranges`:
   - Track line and byte column offsets as tokens are concatenated.
   - Each token becomes a range with a face id.
   - Kakoune ranges are inclusive; the end column is `byte_end - 1`.

### Style to face mapping
- A giallo token is backed by a concrete `Style` (foreground, background, font style).
- Build a stable mapping: `Style -> face_name` for the active theme.
- Face names use a giallo prefix, for example `giallo_0001`.
- Generate Kakoune faces:
  - Foreground/background use `Style.foreground.as_hex()` and `Style.background.as_hex()`.
  - Font style maps to Kakoune attributes:
    - bold -> `+b`
    - italic -> `+i`
    - underline -> `+u`
    - strikethrough -> `+s`
  - Alpha from giallo colors is ignored (Kakoune has no alpha support).

### Face initialization
- On theme load, emit `set-face global` entries for every distinct style used by the theme.
- On theme switch, re-emit faces and re-highlight buffers.
- Cache per-theme face maps so a theme switch is O(1) after first use.

## Configuration

### Location
- `~/.config/giallo.kak/config.toml` (mirrors kak-tree-sitter convention).

### Suggested schema
- `theme = "catppuccin-frappe"`
- `language_map = { javascript = "javascript", typescript = "typescript", ... }`
- `dump_path = "..."` (optional)
- `prefer_builtin_dump = true` (default)
- `remove_default_highlighter = true`

## Compatibility with kak-tree-sitter
- This integration is independent of `kak-tree-sitter`, but follows its `ranges`-based approach.
- Users can enable either or both; the last-installed highlighter wins for a buffer.
- Face names use a distinct prefix to avoid collisions.

## Performance considerations
- Highlighting is full-buffer for v1; this is simplest and reliable.
- Use a debounce on updates (e.g., 50-100ms) to avoid work on rapid edits.
- Limit highlight to a maximum buffer size (configurable), fallback to no highlighting if exceeded.
- Cache `Registry` and compiled theme results across requests.

## Failure modes and fallbacks
- Unknown language: fallback to a plain grammar (`PLAIN_GRAMMAR_NAME` in giallo) or disable.
- Missing theme: fallback to a known default (configurable).
- IPC failure: log and disable highlights for the buffer.

## Milestones
1) **MVP**: giallo-kak server + Kakoune script, full-buffer highlights, built-in dump only.
2) **Config**: user config for theme/lang mapping and custom dump.
3) **Performance**: debounce, buffer size limit, caching.
4) **Quality**: diagnostics, sample themes, docs.
