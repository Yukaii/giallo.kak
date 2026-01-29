# giallo.kak implementation TODOs

## 0) Repo scaffolding
- [ ] Add Cargo workspace or crate layout for the giallo Kakoune server (Rust binary).
- [ ] Add `rc/` directory with `giallo.kak` plugin script.
- [ ] Add `docs/` for user setup and configuration.

## 1) Kakoune script (MVP)
- [x] Define buffer options: `giallo_lang`, `giallo_theme`, `giallo_hl_ranges`, `giallo_buf_fifo_path`, `giallo_buf_sentinel`.
- [x] Add `ranges` highlighter: `add-highlighter -override buffer/giallo ranges giallo_hl_ranges`.
- [x] Add hooks for buffer setup and updates (BufOpen/BufReload/InsertChar/NormalIdle).
- [x] Add filetype-to-language mapping hook (`BufSetOption filetype`).
- [x] Add commands:
  - `giallo-enable` / `giallo-disable` (per buffer)
  - `giallo-rehighlight`
  - `giallo-set-theme <name>`

## 2) Server IPC
- [x] Define a simple request/response format (line-delimited or length-prefixed).
- [x] Implement per-buffer FIFO creation (similar to `kak-tree-sitter`).
- [x] Implement server loop to read buffer snapshots and dispatch highlight jobs.
- [x] Implement session init handshake: return FIFO path + sentinel to Kakoune.
- [x] Wire a global FIFO server from Kakoune (start/stop + request/response).

## 3) Highlight pipeline
- [ ] Load `giallo::Registry` (builtin dump + grammar link).
- [ ] Parse request: language, theme, buffer text.
- [ ] Run `Registry::highlight` to get `HighlightedCode`.
- [ ] Convert tokens to Kakoune ranges (inclusive end column).

## 4) Style → face mapping
- [ ] Implement `Style -> face_name` cache per theme.
- [ ] Emit Kakoune `set-face global` commands for each style.
- [ ] Map giallo font styles to Kakoune attrs (+b/+i/+u/+s).
- [ ] Ignore alpha (Kakoune doesn’t support it).

## 5) Config
- [x] Define config file path and schema (`~/.config/giallo.kak/config.toml`).
- [x] Implement theme/lang map overrides.
- [x] Add default theme fallback and unknown-language fallback.

## 6) Performance & safety
- [x] Debounce updates (50–100ms).
- [ ] Add max buffer size guard.
- [ ] Cache compiled themes / face sets.
- [ ] Add logging and error handling.

## 7) Docs & examples
- [ ] Basic install instructions.
- [ ] Sample config.
- [ ] Troubleshooting (missing theme/lang, IPC errors).
