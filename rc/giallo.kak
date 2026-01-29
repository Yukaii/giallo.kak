# giallo.kak - Kakoune integration (skeleton)
# Usage: source this file from your kakrc.

# Buffer-local options
declare-option -hidden str giallo_lang
declare-option -hidden str giallo_theme
declare-option -hidden range-specs giallo_hl_ranges
declare-option -hidden str giallo_buf_fifo_path
declare-option -hidden str giallo_buf_sentinel
declare-option -hidden bool giallo_enabled false
declare-option -hidden int giallo_buf_update_timestamp -1
declare-option -hidden str giallo_server_req
declare-option -hidden str giallo_server_pid

define-command -docstring "Start giallo server with FIFO IPC" giallo-start-server %{
    evaluate-commands %sh{
        if [ -n "$kak_opt_giallo_server_req" ] && [ -p "$kak_opt_giallo_server_req" ]; then
            exit 0
        fi

        tmpdir="${TMPDIR:-/tmp}"
        dir="$(mktemp -d "$tmpdir/giallo-kak.XXXXXX")"
        req="$dir/req.fifo"
        mkfifo "$req"

        giallo-kak --fifo "$req" >/dev/null 2>&1 &
        pid=$!

        printf 'set-option global giallo_server_req %s\n' "$req"
        printf 'set-option global giallo_server_pid %s\n' "$pid"
    }
}

define-command -docstring "Stop giallo server" giallo-stop-server %{
    evaluate-commands %sh{
        if [ -n "$kak_opt_giallo_server_pid" ]; then
            kill "$kak_opt_giallo_server_pid" >/dev/null 2>&1 || true
        fi
        if [ -n "$kak_opt_giallo_server_req" ]; then
            rm -f "$kak_opt_giallo_server_req"
        fi
        printf 'set-option global giallo_server_req ""\n'
        printf 'set-option global giallo_server_pid ""\n'
    }
}

# Enable giallo highlighting for current buffer
# TODO: wire IPC and automatic updates.
define-command -docstring "Enable giallo highlighting for the current buffer" giallo-enable %{ 
    set-option buffer giallo_enabled true
    add-highlighter -override buffer/giallo ranges giallo_hl_ranges
    giallo-start-server
    giallo-init-buffer
}

# Disable giallo highlighting for current buffer
define-command -docstring "Disable giallo highlighting for the current buffer" giallo-disable %{ 
    set-option buffer giallo_enabled false
    remove-highlighter buffer/giallo
    set-option buffer giallo_hl_ranges ""
    set-option buffer giallo_buf_fifo_path ""
    set-option buffer giallo_buf_sentinel ""
    set-option buffer giallo_buf_update_timestamp -1
}

# Initialize per-buffer FIFO via server handshake.
define-command -docstring "Initialize per-buffer FIFO for giallo" giallo-init-buffer %{
    evaluate-commands -no-hooks %sh{
        fifo="$kak_opt_giallo_server_req"
        if [ -z "$fifo" ] || [ ! -p "$fifo" ]; then
            printf 'giallo-start-server\n'
            exit 0
        fi

        session="$kak_session"
        buffer="$kak_bufname"

        # Write to FIFO in background to avoid blocking UI if server isn't ready yet.
        sh -c "printf 'INIT %s %s %s\n' '$session' '$buffer' '$buffer' > '$fifo'" >/dev/null 2>&1 &
    }
}

# Send buffer content to the server.
define-command -hidden giallo-buffer-update %{
    evaluate-commands -no-hooks %sh{
        if [ -z "$kak_opt_giallo_buf_fifo_path" ]; then
            exit 0
        fi
        printf 'evaluate-commands -no-hooks %%{\n'
        printf 'echo -to-file "%%opt{giallo_buf_fifo_path}" -- "H %%opt{giallo_lang} %%opt{giallo_theme}"\n'
        printf 'write "%%opt{giallo_buf_fifo_path}"\n'
        printf 'echo -to-file "%%opt{giallo_buf_fifo_path}" -- "%%opt{giallo_buf_sentinel}"\n'
        printf '}\n'
    }
}

# Execute a command only if buffer timestamp changed.
define-command -hidden giallo-exec-if-changed -params 1 %{
    set-option -remove buffer giallo_buf_update_timestamp %val{timestamp}

    try %{
        evaluate-commands "giallo-exec-nop-%opt{giallo_buf_update_timestamp}"
        set-option buffer giallo_buf_update_timestamp %val{timestamp}
    } catch %{
        set-option buffer giallo_buf_update_timestamp %val{timestamp}
        evaluate-commands %arg{1}
    }
}

define-command -hidden giallo-exec-nop-0 nop

# Force a rehighlight
define-command -docstring "Rehighlight current buffer using giallo" giallo-rehighlight %{ 
    evaluate-commands -no-hooks %sh{
        if [ "$kak_opt_giallo_enabled" = "true" ]; then
            printf 'giallo-exec-if-changed giallo-buffer-update\n'
        fi
    }
}

# Set theme for current buffer (placeholder)
define-command -params 1 -docstring "Set giallo theme for current buffer" giallo-set-theme %{ 
    set-option buffer giallo_theme %arg{1}
    giallo-rehighlight
}

# Auto set giallo_lang from filetype unless explicitly set.
hook -group giallo global BufSetOption filetype=.* %{
    evaluate-commands %sh{
        if [ -z "$kak_opt_giallo_lang" ] && [ -n "$kak_opt_filetype" ]; then
            printf 'set-option buffer giallo_lang %s\n' "$kak_opt_filetype"
        fi
    }
}

# Auto refresh on idle edits; uses giallo_enabled guard inside giallo-rehighlight.
hook -group giallo global NormalIdle .* %{ giallo-rehighlight }

# Also refresh on major buffer lifecycle events.
hook -group giallo global BufReload .* %{ giallo-rehighlight }
hook -group giallo global BufWritePost .* %{ giallo-rehighlight }
hook -group giallo global InsertChar .* %{ giallo-rehighlight }
hook -group giallo global BufClose .* %{ giallo-disable }
