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
declare-option -hidden str giallo_server_log
declare-option -hidden bool giallo_remove_default_highlighter true

# Debug option: set to true to enable verbose server logging
declare-option -hidden bool giallo_debug false

define-command -docstring "Start giallo server with FIFO IPC" giallo-start-server %{
    evaluate-commands %sh{
        if [ -n "$kak_opt_giallo_server_req" ] && [ -p "$kak_opt_giallo_server_req" ]; then
            exit 0
        fi

        tmpdir="${TMPDIR:-/tmp}"
        dir="$(mktemp -d "$tmpdir/giallo-kak.XXXXXX")"
        req="$dir/req.fifo"
        mkfifo "$req"

        verbose_flag=""
        redirect=">/dev/null 2>&1"
        if [ "$kak_opt_giallo_debug" = "true" ]; then
            verbose_flag="--verbose"
            log_file="$dir/giallo.log"
            redirect=">>$log_file 2>&1"
            printf 'echo "giallo: debug logging to %s"\n' "$log_file"
        fi

        giallo_bin=$(command -v giallo-kak || true)
        eval "giallo-kak --fifo \"$req\" $verbose_flag $redirect &"
        pid=$!

        printf 'set-option global giallo_server_req %s\n' "$req"
        printf 'set-option global giallo_server_pid %s\n' "$pid"
        if [ "$kak_opt_giallo_debug" = "true" ]; then
            printf 'set-option global giallo_server_log %s\n' "$log_file"
            if [ -n "$log_file" ]; then
                printf 'echo "giallo: server pid=%s fifo=%s bin=%s" >>%s\n' "$pid" "$req" "$giallo_bin" "$log_file"
            fi
        fi
    }
}

# Show the log file path for debugging
define-command -docstring "Show giallo server log file path" giallo-log %{
    echo "giallo log: %opt{giallo_server_log}"
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
        printf 'set-option global giallo_server_log ""\n'
    }
}

define-command -hidden giallo-remove-default-highlighter %{
    evaluate-commands %sh{
        if [ "$kak_opt_giallo_remove_default_highlighter" != "true" ]; then
            exit 0
        fi
        if [ "$kak_opt_giallo_enabled" != "true" ]; then
            exit 0
        fi
        if [ -z "$kak_opt_filetype" ]; then
            exit 0
        fi
        printf 'try %%{ remove-highlighter window/%s }\n' "$kak_opt_filetype"
    }
}

# Enable giallo highlighting for current buffer
# TODO: wire IPC and automatic updates.
define-command -docstring "Enable giallo highlighting for the current buffer" giallo-enable %{ 
    evaluate-commands %sh{
        if [ "$kak_opt_giallo_debug" = "true" ] && [ -n "$kak_opt_giallo_server_log" ] && [ "$kak_buffile" = "$kak_opt_giallo_server_log" ]; then
            printf 'echo "giallo: skip enabling for server log buffer"\n'
            exit 0
        fi
    }
    set-option buffer giallo_enabled true
    
    # Auto-set language from filetype if not already set
    evaluate-commands %sh{
        if [ -z "$kak_opt_giallo_lang" ] && [ -n "$kak_opt_filetype" ]; then
            printf 'set-option buffer giallo_lang %s\n' "$kak_opt_filetype"
        fi
    }
    
    giallo-remove-default-highlighter
    try %{ remove-highlighter buffer/giallo }
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
        log_file="$kak_opt_giallo_server_log"

        # Write to FIFO in background to avoid blocking UI if server isn't ready yet.
        sh -c "printf 'INIT %s %s %s\n' '$session' '$buffer' '$buffer' > '$fifo'" >/dev/null 2>&1 &
        if [ "$kak_opt_giallo_debug" = "true" ] && [ -n "$log_file" ]; then
            printf 'giallo-init-buffer: session=%s buffer=%s fifo=%s\n' "$session" "$buffer" "$fifo" >>"$log_file"
        fi
    }
}

# Send buffer content to the server.
define-command -hidden giallo-buffer-update %{
    evaluate-commands %sh{
        if [ "$kak_opt_giallo_debug" = "true" ] && [ -n "$kak_opt_giallo_server_log" ] && [ "$kak_buffile" = "$kak_opt_giallo_server_log" ]; then
            echo "giallo-buffer-update: skip server log buffer" >&2
            exit 0
        fi
        if [ -z "$kak_opt_giallo_buf_fifo_path" ]; then
            if [ "$kak_opt_giallo_debug" = "true" ]; then
                echo "giallo-buffer-update: ERROR - no fifo path" >&2
            fi
            exit 0
        fi
        
        fifo="$kak_opt_giallo_buf_fifo_path"
        sentinel="$kak_opt_giallo_buf_sentinel"
        lang="$kak_opt_giallo_lang"
        theme="$kak_opt_giallo_theme"
        log_file="$kak_opt_giallo_server_log"
        
        # Debug output - always log to stderr for shell debugging
        if [ "$kak_opt_giallo_debug" = "true" ]; then
            echo "giallo-buffer-update: START buffer=$kak_bufname" >&2
            echo "giallo-buffer-update: fifo=$fifo" >&2
            echo "giallo-buffer-update: lang=$lang theme=$theme" >&2
            echo "giallo-buffer-update: sentinel=$sentinel" >&2
            echo "giallo-buffer-update: buffile=$kak_buffile" >&2
            if [ -n "$log_file" ]; then
                printf 'giallo-buffer-update: buffer=%s filetype=%s lang=%s theme=%s fifo=%s sentinel=%s buffile=%s\n' \
                    "$kak_bufname" "$kak_opt_filetype" "$lang" "$theme" "$fifo" "$sentinel" "$kak_buffile" >>"$log_file"
            fi
        fi
        
        # Check if FIFO exists
        if [ ! -p "$fifo" ]; then
            if [ "$kak_opt_giallo_debug" = "true" ]; then
                echo "giallo-buffer-update: ERROR - not a fifo: $fifo" >&2
                ls -la "$fifo" 2>&1 >&2
                if [ -n "$log_file" ]; then
                    printf 'giallo-buffer-update: ERROR - not a fifo: %s\n' "$fifo" >>"$log_file"
                fi
            fi
            printf 'echo "giallo: ERROR - buffer fifo not found"\n'
            exit 1
        fi
        
        # Send header with language and theme
        if [ "$kak_opt_giallo_debug" = "true" ]; then
            echo "giallo-buffer-update: writing header..." >&2
            if [ -n "$log_file" ]; then
                printf 'giallo-buffer-update: writing header lang=%s theme=%s\n' "$lang" "$theme" >>"$log_file"
            fi
        fi
        
        {
            printf 'H %s %s\n' "$lang" "$theme"
            cat "$kak_buffile"
            printf '%s\n' "$sentinel"
        } > "$fifo" 2>&1
        
        result=$?
        if [ "$kak_opt_giallo_debug" = "true" ]; then
            echo "giallo-buffer-update: write result=$result" >&2
            if [ -n "$log_file" ]; then
                printf 'giallo-buffer-update: write result=%s\n' "$result" >>"$log_file"
            fi
        fi
        
        if [ $result -ne 0 ]; then
            if [ "$kak_opt_giallo_debug" = "true" ]; then
                printf 'echo "giallo: ERROR - failed to write to fifo (exit %d)"\n' "$result"
                if [ -n "$log_file" ]; then
                    printf 'giallo-buffer-update: ERROR - write failed exit=%s\n' "$result" >>"$log_file"
                fi
            fi
            exit 1
        fi
        
        if [ "$kak_opt_giallo_debug" = "true" ]; then
            echo "giallo-buffer-update: SUCCESS" >&2
            if [ -n "$log_file" ]; then
                printf 'giallo-buffer-update: SUCCESS\n' >>"$log_file"
            fi
        fi
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
        %arg{1}
    }
}

define-command -hidden giallo-exec-nop-0 nop

# Show debug information about current buffer
define-command -docstring "Show giallo debug info for current buffer" giallo-debug %{
    echo "giallo debug: enabled=%opt{giallo_enabled} lang=%opt{giallo_lang} theme=%opt{giallo_theme} fifo_path=%opt{giallo_buf_fifo_path}"
}

# Manual test command
define-command -docstring "Force send buffer to server (debug)" giallo-force-update %{
    echo "giallo: force update starting..."
    evaluate-commands %sh{
        if [ -z "$kak_opt_giallo_buf_fifo_path" ]; then
            printf 'echo "giallo: ERROR - no fifo path set"\n'
            exit 1
        fi
        if [ ! -p "$kak_opt_giallo_buf_fifo_path" ]; then
            printf 'echo "giallo: ERROR - fifo does not exist: %s"\n' "$kak_opt_giallo_buf_fifo_path"
            exit 1
        fi
        printf 'echo "giallo: fifo ok, calling update..."\n'
    }
    giallo-buffer-update
    echo "giallo: force update complete"
}

# Force a rehighlight
define-command -docstring "Rehighlight current buffer using giallo" giallo-rehighlight %{ 
    # In debug mode, always update regardless of timestamp
    evaluate-commands %sh{
        if [ "$kak_opt_giallo_debug" = "true" ] && [ -n "$kak_opt_giallo_server_log" ] && [ "$kak_buffile" = "$kak_opt_giallo_server_log" ]; then
            exit 0
        fi
        if [ "$kak_opt_giallo_debug" = "true" ] && [ -n "$kak_opt_giallo_server_log" ] && [ "$kak_bufname" = "$kak_opt_giallo_server_log" ]; then
            exit 0
        fi
        if [ "$kak_opt_giallo_enabled" = "true" ]; then
            printf 'try %%{ remove-highlighter buffer/giallo }\n'
            printf 'add-highlighter -override buffer/giallo ranges giallo_hl_ranges\n'
        fi
        if [ "$kak_opt_giallo_debug" = "true" ] && [ "$kak_opt_giallo_enabled" = "true" ]; then
            echo "giallo-rehighlight: enabled=$kak_opt_giallo_enabled fifo=$kak_opt_giallo_buf_fifo_path lang=$kak_opt_giallo_lang" >&2
            if [ "$kak_opt_giallo_enabled" = "true" ] && [ -n "$kak_opt_giallo_buf_fifo_path" ]; then
                printf 'giallo-buffer-update\n'
            fi
        else
            if [ "$kak_opt_giallo_enabled" = "true" ]; then
                printf 'giallo-exec-if-changed giallo-buffer-update\n'
            fi
        fi
    }
}

# Set theme for current buffer (placeholder)
define-command -params 1 -docstring "Set giallo theme for current buffer" giallo-set-theme %{ 
    set-option buffer giallo_theme %arg{1}
    giallo-rehighlight
}

# Global option to auto-enable giallo on buffers with filetype
declare-option -hidden bool giallo_auto_enable true

# Auto set giallo_lang from filetype unless explicitly set.
hook -group giallo global BufSetOption filetype=.* %{
    evaluate-commands %sh{
        if [ -z "$kak_opt_giallo_lang" ] && [ -n "$kak_opt_filetype" ]; then
            printf 'set-option buffer giallo_lang %s\n' "$kak_opt_filetype"
            if [ "$kak_opt_giallo_debug" = "true" ] && [ -n "$kak_opt_giallo_server_log" ]; then
                printf 'giallo: set giallo_lang=%s from filetype\n' "$kak_opt_filetype" >>"$kak_opt_giallo_server_log"
            fi
        fi
    }
    giallo-remove-default-highlighter
}

# Auto-enable giallo on open buffers that have a filetype
hook -group giallo global BufCreate .* %{
    evaluate-commands %sh{
        if [ "$kak_opt_giallo_auto_enable" = "true" ] && [ -n "$kak_opt_filetype" ]; then
            printf 'giallo-enable\n'
        fi
    }
}

# Auto-enable giallo when filetype is set (for files that detect language after creation)
hook -group giallo global BufSetOption filetype=.* %{
    evaluate-commands %sh{
        if [ "$kak_opt_giallo_auto_enable" = "true" ] && [ -n "$kak_opt_filetype" ] && [ "$kak_opt_giallo_enabled" != "true" ]; then
            printf 'giallo-enable\n'
        fi
    }
}

hook -group giallo global WinDisplay .* %{ giallo-remove-default-highlighter }

# Auto refresh on idle edits; uses giallo_enabled guard inside giallo-rehighlight.
hook -group giallo global NormalIdle .* %{ giallo-rehighlight }

# Also refresh on major buffer lifecycle events.
hook -group giallo global BufReload .* %{ giallo-rehighlight }
hook -group giallo global BufWritePost .* %{ giallo-rehighlight }
hook -group giallo global InsertChar .* %{ giallo-rehighlight }
hook -group giallo global BufClose .* %{ giallo-disable }
