# giallo.kak - Kakoune integration (skeleton)
# Usage: source this file from your kakrc.

# Buffer-local options
declare-option -hidden str giallo_lang
declare-option -hidden str giallo_theme
declare-option -hidden str giallo_hl_ranges
declare-option -hidden str giallo_buf_fifo_path
declare-option -hidden str giallo_buf_sentinel
declare-option -hidden bool giallo_enabled false

# Enable giallo highlighting for current buffer
# TODO: wire IPC and automatic updates.
define-command -docstring "Enable giallo highlighting for the current buffer" giallo-enable %{ 
    set-option buffer giallo_enabled true
    add-highlighter -override buffer/giallo ranges giallo_hl_ranges
    giallo-rehighlight
}

# Disable giallo highlighting for current buffer
define-command -docstring "Disable giallo highlighting for the current buffer" giallo-disable %{ 
    set-option buffer giallo_enabled false
    remove-highlighter buffer/giallo
    set-option buffer giallo_hl_ranges ""
}

# Force a rehighlight (placeholder)
define-command -docstring "Rehighlight current buffer using giallo" giallo-rehighlight %{ 
    evaluate-commands %sh{
        if [ "$kak_opt_giallo_enabled" != "true" ]; then
            exit 0
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

        {
            printf 'H %s %s %s\n' "$lang" "$theme" "$len"
            printf %s "$content"
        } | giallo-kak --oneshot
    }
}

# Set theme for current buffer (placeholder)
define-command -params 1 -docstring "Set giallo theme for current buffer" giallo-set-theme %{ 
    set-option buffer giallo_theme %arg{1}
    giallo-rehighlight
}

# Auto refresh on idle edits; uses giallo_enabled guard inside giallo-rehighlight.
hook -group giallo global NormalIdle .* %{ giallo-rehighlight }
