# Performance Improvement Notes

Landed on `perf/quick-wins`:

- `[profile.release]` with thin LTO and `codegen-units = 1`
- Cached the `kak` binary PATH lookup (was re-scanned per send)
- Ranges built into a single pre-sized buffer; face names no longer cloned per token
- Direct socket send instead of spawning `kak -p` per update (see #1)
- giallo upgraded to 0.5.2 (bitcode+zstd dumps; registry now ~46ms / ~30MB RSS)
- Chunked delta sends + bounded-window incremental re-parsing (see #3/#4)

Remaining ideas, roughly ranked by expected impact.

## 1. ~~Persistent connection to the Kakoune session~~ (done, with a caveat)

A truly persistent connection is **not possible**: Kakoune's server closes the
socket immediately after executing each `Command` message by design
(`MessageType::Command` handler in `src/remote.cc`). What was implemented
instead: `send_to_kak` writes the framed wire protocol directly to the session
unix socket (`src/kakoune.rs:send_command_to_session`, same framing as
`kak -p`: `[type=2 u8][frame size u32][cmd len u32][cmd bytes]`), avoiding the
fork/exec of the kak binary per update. Falls back to spawning `kak -p` when
no reachable socket is found. Verified end-to-end against a live headless
Kakoune 2025.06.03 session.

## 2. ~~Lazy / partial grammar loading~~ (won't do - impact obsolete)

Re-evaluated after the giallo 0.5.2 upgrade (zstd+bitcode dumps, upstream
perf work). Measured on the current binary:

- registry load: ~46ms
- steady-state server RSS: ~30MB (peak ~41MB during load)

Partial loading is also effectively impossible without upstream changes:
the builtin dump is monolithic (`builtin.zst`) with no filter/subset/remove
APIs, and raw grammar sources are not shipped. Given that the FIFO-server
architecture loads once and amortizes across all buffers for the session,
the remaining win (~25MB, 46ms one-off) does not justify the complexity.
Revisit only if giallo exposes subset loading or a low-RAM target emerges.

## 3. ~~Incremental / dirty-region highlighting~~ (done)

Implemented server-side, no upstream giallo changes needed:

- **Chunked delta sends** (`src/highlight.rs`): output is split into 1000-line
  chunks mapped to buffer options `giallo_hl_ranges` (chunk 0, legacy name) and
  `giallo_hl_ranges_N`. Only chunks whose tokens changed are re-sent; identical
  updates skip the send entirely. Chunk highlighters are registered explicitly
  by the server - commands over the session socket run hook-less
  (`Context::EmptyContextFlag`), so BufSetOption hooks cannot be relied on.
- **Bounded-window re-parse**: the changed region is detected via common
  prefix/suffix against the cached text. Small edits that don't touch string/
  comment delimiters re-parse only `[dirty-200 .. dirty+50]` lines
  (`WARMUP_LINES`/`MARGIN_LINES`) and splice tokens into the per-buffer cache.
  Everything else (delimiter edits, oversized regions, near-whole-document
  windows) falls back to a full parse. Identical text skips parsing entirely.

Known limitation: a multiline construct opened more than ~200 lines above an
edit can mis-color lines in the spliced window until the next full parse
(which happens on any delimiter-touching edit). True incremental tokenization
would need giallo to expose parser-state snapshots (`Registry::tokenize` is
`pub(crate)`; `StateStack` unreachable).

## 4. ~~Cache face maps per (lang, theme) across requests~~ (done)

`FaceAllocator` persists per buffer for a given (lang, theme); styles keep
their face name across updates and only newly allocated faces are sent.

## 5. ~~Avoid allocating StyleKey before cache lookup~~ (done)

StyleKey is now a packed u64 (fg RGB24 / bg RGB24 / font-style flags); face-map
lookups allocate and hash nothing. The two `as_hex()` String allocations per
lookup remain until giallo exposes non-allocating color access.

`style_key()` allocates two Strings (normalized hex fg/bg) per token even on
cache hits. Consider hashing a packed representation (e.g. RGB u32s + font
style bits) or borrowing keys via `HashMap<StyleKey, _>` with a raw-entry-style
lookup to skip allocation on hits.

## 6. ~~Reduce shell-side process spawns in rc/giallo.kak~~ (mostly done)

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
For direct measurement run:

    cargo test --release --test highlight_bench -- --ignored --nocapture

Typical numbers on the reference machine (rust grammar, mid-file edit):

| lines | full ms | windowed ms | identical µs |
|------:|--------:|------------:|-------------:|
| 200   | 7.4     | 5.8         | 0.3          |
| 1000  | 37.0    | 10.2        | 1.3          |
| 5000  | 182.9   | 13.1        | 7.3          |

Windowed cost is ~flat in file size (bounded by warmup+margin), while full
parse grows linearly; identical updates cost only the byte-diff plan.
