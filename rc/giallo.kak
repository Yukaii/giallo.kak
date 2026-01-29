# giallo.kak - Kakoune integration (skeleton)
# Usage: source this file from your kakrc.

# Buffer-local options
declare-option -hidden str giallo_lang
declare-option -hidden str giallo_theme
declare-option -hidden str giallo_hl_ranges
declare-option -hidden str giallo_buf_fifo_path
declare-option -hidden str giallo_buf_resp_path
declare-option -hidden str giallo_buf_sentinel
declare-option -hidden bool giallo_enabled false
declare-option -hidden str giallo_server_req
declare-option -hidden str giallo_server_resp
declare-option -hidden str giallo_server_pid

define-command -docstring "Start giallo server with FIFO IPC" giallo-start-server %{
    evaluate-commands %sh{
        if [ -n "$kak_opt_giallo_server_req" ] && [ -p "$kak_opt_giallo_server_req" ]; then
            exit 0
        fi

        tmpdir="${TMPDIR:-/tmp}"
        dir="$(mktemp -d "$tmpdir/giallo-kak.XXXXXX")"
        req="$dir/req.fifo"
        resp="$dir/resp.fifo"
        mkfifo "$req"
        mkfifo "$resp"

        giallo-kak --fifo "$req" --resp "$resp" >/dev/null 2>&1 &
        pid=$!

        printf 'set-option global giallo_server_req %s\n' "$req"
        printf 'set-option global giallo_server_resp %s\n' "$resp"
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
        if [ -n "$kak_opt_giallo_server_resp" ]; then
            rm -f "$kak_opt_giallo_server_resp"
        fi

        printf 'set-option global giallo_server_req ""\n'
        printf 'set-option global giallo_server_resp ""\n'
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
    giallo-rehighlight
}

# Disable giallo highlighting for current buffer
define-command -docstring "Disable giallo highlighting for the current buffer" giallo-disable %{ 
    set-option buffer giallo_enabled false
    remove-highlighter buffer/giallo
    set-option buffer giallo_hl_ranges ""
    set-option buffer giallo_buf_fifo_path ""
    set-option buffer giallo_buf_resp_path ""
    set-option buffer giallo_buf_sentinel ""
}

# Initialize per-buffer FIFO via server handshake.
define-command -docstring "Initialize per-buffer FIFO for giallo" giallo-init-buffer %{
    evaluate-commands %sh{
        if [ -z "$kak_opt_giallo_server_req" ] || [ ! -p "$kak_opt_giallo_server_req" ]; then
            printf 'giallo-start-server\n'
        fi
        if [ -z "$kak_opt_giallo_buf_fifo_path" ] || [ ! -p "$kak_opt_giallo_buf_fifo_path" ]; then
            printf 'giallo-init-buffer\n'
        fi

        token="${kak_bufname:-buffer}"

        {
            printf 'INIT %s\n' "$token"
        } > "$kak_opt_giallo_server_req"

        IFS= read -r header < "$kak_opt_giallo_server_resp"
        set -- $header
        if [ "$1" = "OK" ] && [ -n "$2" ]; then
            dd bs=1 count="$2" < "$kak_opt_giallo_server_resp" 2>/dev/null
        fi
    }
}

# Force a rehighlight (placeholder)
define-command -docstring "Rehighlight current buffer using giallo" giallo-rehighlight %{ 
    evaluate-commands %sh{
        if [ "$kak_opt_giallo_enabled" != "true" ]; then
            exit 0
        fi

        if [ -z "$kak_opt_giallo_server_req" ] || [ ! -p "$kak_opt_giallo_server_req" ]; then
            printf 'giallo-start-server\n'
        fi

        lang="$kak_opt_giallo_lang"
        theme="$kak_opt_giallo_theme"

        if [ -z "$lang" ]; then
            lang="$kak_opt_filetype"
        fi
        if [ -z "$theme" ]; then
            theme="catppuccin-frappe"
        fi

        content="$kak_bufstr"
        len=$(printf %s "$content" | wc -c | tr -d '[:space:]')

        if [ -n "$kak_opt_giallo_buf_fifo_path" ] && [ -p "$kak_opt_giallo_buf_fifo_path" ] && [ -n "$kak_opt_giallo_buf_resp_path" ] && [ -p "$kak_opt_giallo_buf_resp_path" ]; then
            {
                printf 'H %s %s %s\n' "$lang" "$theme" "$len"
                printf %s "$content"
            } > "$kak_opt_giallo_buf_fifo_path"

            IFS= read -r header < "$kak_opt_giallo_buf_resp_path"
            set -- $header
            if [ "$1" = "OK" ] && [ -n "$2" ]; then
                dd bs=1 count="$2" < "$kak_opt_giallo_buf_resp_path" 2>/dev/null
            fi
        else
            {
                printf 'H %s %s %s\n' "$lang" "$theme" "$len"
                printf %s "$content"
            } | giallo-kak --oneshot
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
        if [ -z "$kak_opt_giallo_lang" ]; then
            printf 'set-option buffer giallo_lang %s\n' "$kak_opt_filetype"
        fi
    }
}

# Auto refresh on idle edits; uses giallo_enabled guard inside giallo-rehighlight.
hook -group giallo global NormalIdle .* %{ giallo-rehighlight }

# Also refresh on major buffer lifecycle events.
hook -group giallo global BufOpen .* %{ giallo-rehighlight }
hook -group giallo global BufReload .* %{ giallo-rehighlight }
hook -group giallo global BufWritePost .* %{ giallo-rehighlight }
hook -group giallo global InsertChar .* %{ giallo-rehighlight }
hook -group giallo global BufClose .* %{ giallo-disable }
