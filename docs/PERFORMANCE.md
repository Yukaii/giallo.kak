# Performance Improvement Notes

Quick wins already landed on `perf/quick-wins`:

- `[profile.release]` with thin LTO and `codegen-units = 1`
- Cached the `kak` binary PATH lookup (was re-scanned per send)
- Ranges built into a single pre-sized buffer; face names no longer cloned per token

Remaining ideas, roughly ranked by expected impact.

## 1. Persistent connection to the Kakoune session

Every highlight spawns a `kak -p <session>` subprocess (`src/highlight.rs`),
paying fork/exec + session lookup each update (~10-50ms). Replace with writes
to the session's Unix socket directly (a socket connection is already used for
liveness checks in `src/server_resources.rs`). Keep `kak -p` as fallback.

## 2. Lazy / partial grammar loading

The full builtin registry (~70-90MB) is loaded eagerly at startup
(`src/main.rs:59`) for every mode, including `--oneshot`, `list-grammars`,
and `list-themes`. If giallo's API allows, load grammars per language on first
use, and skip registry construction entirely for list modes. Check whether
giallo 0.4.0 offers lazy loading or mmap-backed dumps; upgrading may help both
memory and startup time.

## 3. Incremental / dirty-region highlighting

Full buffer content is re-highlighted on every keystroke batch (rate-limited
to 50ms shell-side). For large files this dominates cost. Options:
dirty-line tracking between Kakoune and the server, or an upstream giallo
incremental API if one exists.

## 4. Cache face maps per (lang, theme) across requests

Face dedup map is rebuilt from scratch on every highlight
(`src/highlighting.rs:build_kakoune_commands`) even though it depends only on
the theme. DESIGN.md:94 already plans caching per-theme face maps; reuse them
across requests and only emit new `set-face` commands when faces change.

## 5. Avoid allocating StyleKey before cache lookup

`style_key()` allocates two Strings (normalized hex fg/bg) per token even on
cache hits. Consider hashing a packed representation (e.g. RGB u32s + font
style bits) or borrowing keys via `HashMap<StyleKey, _>` with a raw-entry-style
lookup to skip allocation on hits.

## 6. Reduce shell-side process spawns in rc/giallo.kak

The rate limiter around FIFO writes shells out to `date +%s%3N`, `kill -0`,
and `ps -p` per edit event (`rc/giallo.kak:216-260`). Kakoune's `%val{...}`
timestamp can replace the `date` call; liveness checks can be folded into the
server instead of shell probes.

## 7. Trim per-buffer thread stacks

Two threads are spawned per buffer (`src/fifo.rs:89`). Default stacks are 8MB
virtual each; use `thread::Builder::stack_size(...)` since the work is shallow.
Minor RSS/virtual-footprint win, mostly cosmetic.

## 8. Reader-thread scan efficiency

The FIFO reader restarts sentinel search from position 0 after each drain
(`src/fifo.rs:151-154`) - O(n^2)-ish when many messages accumulate. Track the
last scanned offset. Only matters under bursty multi-buffer load.

## Measurement caveat

`tests/performance.rs` measures wall time including process spawn + registry
load, so it guards regressions but does not isolate highlight-path costs.
When pursuing items above, benchmark the highlight path directly (e.g. an
in-process benchmark against `Registry::highlight` + command building).
