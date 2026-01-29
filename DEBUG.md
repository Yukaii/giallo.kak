## Summary: giallo.kak debugging

### Current Status

The server starts correctly, creates per-buffer FIFOs, and spawns threads. However, buffer updates are not reaching the server - the FIFO writes are failing silently.

### Log Analysis

```
[INFO giallo_kak] starting giallo-kak server
[DEBUG giallo_kak] INIT: created buffer FIFO at /tmp/.../47db79e303771863.req.fifo
[DEBUG giallo_kak] buffer FIFO: starting for buffer=README.md sentinel=giallo-47db79e303771863
# <-- No "got header" message means update never received
```

### Key Observations

1. **INIT works**: Server creates FIFO, Kakoune receives path
2. **Force update completes**: Kakoune command finishes without error
3. **Server receives nothing**: Buffer thread waits but never gets data

### Potential Issues

1. **FIFO blocking**: The buffer thread opens FIFO for reading but no writer yet
2. **Shell script errors**: Writes failing silently
3. **FIFO path mismatch**: Different path stored vs used

### Test Steps

1. After rebuild, reload script: `evaluate-commands %sh{ giallo-kak --print-rc }`
2. Stop/start server
3. Enable on buffer
4. Run `giallo-force-update`
5. Check for new error messages in Kakoune and server log

---

## Memory Optimization (Jan 2026)

### Problem
Each buffer spawn created a new thread that loaded its own `Registry::builtin()`, consuming 70-90MB per buffer. With multiple buffers open, memory usage grew linearly.

### Solution
Wrapped the registry in `Arc<Registry>` to share it across all buffer handler threads:

**Changes in `main.rs`:**
1. Registry loaded once in main and wrapped in `Arc::new(registry)`
2. `Arc::clone(&registry)` passed to each buffer thread instead of loading new registry
3. Updated function signatures to accept `Arc<Registry>` or `&Registry`
4. Removed per-thread `Registry::builtin()` call

### Result
- **Before**: ~70-90MB per buffer
- **After**: ~1-2MB per buffer (shared registry + thread overhead)

### Technical Details
- `giallo::Registry` implements both `Send` and `Sync`, making it safe to share across threads
- `Arc` (Atomically Reference Counted) allows multiple owners of the same data
- Registry is read-only after initialization, so no synchronization primitives needed
- `highlight()` method only requires `&self`, enabling concurrent read access
