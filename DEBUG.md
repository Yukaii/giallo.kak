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
