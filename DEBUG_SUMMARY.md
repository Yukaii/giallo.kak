## What was the issue?

The user is testing giallo.kak highlighting but no updates are reaching the server. The INIT commands work (FIFOs are created, threads spawn) but buffer content is never sent.

## What I found

1. **giallo-rehighlight was broken**: It generated a command but never executed it as Kakoune code
2. **giallo-buffer-update runs as %sh**: It writes directly to the FIFO from shell
3. **giallo-exec-if-changed uses timestamp**: It tries to detect buffer changes but may not work initially
4. **No debug output from the hooks**: Can't see if they're firing

## What I fixed

1. Fixed `giallo-rehighlight` to actually execute the command
2. In debug mode, it now bypasses the timestamp check and always calls `giallo-buffer-update`
3. Added stderr logging to see hook execution

## Test command

```kak
# Stop and restart
:giallo-stop-server
:giallo-enable

# Or manually force
:giallo-force-update

# Check debug
:giallo-debug
```

In debug mode, you should now see in the server log:
```
giallo-rehighlight: enabled=true fifo=/tmp/...
giallo-buffer-update: buffer=main.rs fifo=/tmp/... lang=rust theme=
giallo-buffer-update: sending header
giallo-buffer-update: sending content
giallo-buffer-update: sending sentinel
[DEBUG giallo_kak] buffer FIFO: got header lang=rust theme=
...
```
