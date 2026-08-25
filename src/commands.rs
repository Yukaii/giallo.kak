use giallo::Registry;
use std::fs;

use crate::config::{expand_path, Config};

pub fn list_grammars(registry: &Registry, config: &Config, plain: bool) {
    if !plain {
        println!("Available grammars:");
        println!();
    }

    let common_grammars = vec![
        "rust",
        "python",
        "javascript",
        "typescript",
        "json",
        "yaml",
        "toml",
        "markdown",
        "bash",
        "go",
        "cpp",
        "c",
        "java",
        "ruby",
        "php",
        "html",
        "css",
        "scss",
        "xml",
        "sql",
        "docker",
        "terraform",
        "hcl",
        "shellscript",
        "lua",
        "vim",
        "regex",
        "make",
        "cmake",
        "ini",
        "diff",
        "git-commit",
        "git-rebase",
        "graphql",
        "proto",
        "swift",
        "kotlin",
        "scala",
        "clojure",
        "erlang",
        "elixir",
        "haskell",
        "ocaml",
        "fsharp",
        "r",
        "matlab",
        "julia",
        "perl",
        "vue",
        "svelte",
        "astro",
        "zig",
        "nix",
        "dart",
        "typst",
        "prisma",
        "csharp",
        "powershell",
        "fish",
        "nushell",
        "latex",
        "less",
        "stylus",
        "postcss",
        "gdscript",
        "solidity",
        "groovy",
        "nim",
        "odin",
        "crystal",
        "elisp",
        "emacs-lisp",
        "lisp",
        "scheme",
        "racket",
        "raku",
        "perl6",
        "cobol",
        "awk",
        "tcl",
        "rst",
        "asciidoc",
        "bibtex",
        "bicep",
        "blade",
        "glsl",
        "hlsl",
        "wgsl",
        "handlebars",
        "jinja",
        "json5",
        "jsonc",
        "kdl",
        "liquid",
        "mdx",
        "mermaid",
        "moonbit",
        "objc",
        "objective-c",
        "plsql",
        "pug",
        "ron",
        "sass",
        "surrealql",
        "templ",
        "twig",
        "wasm",
        "wit",
        "wolfram",
    ];

    let mut found_grammars = Vec::new();
    for grammar in &common_grammars {
        if registry.contains_grammar(grammar) {
            found_grammars.push(*grammar);
        }
    }

    let mut custom_grammars: Vec<String> = Vec::new();
    if let Some(ref grammars_path) = config.grammars_path {
        let path = expand_path(grammars_path);
        if path.exists() {
            if let Ok(entries) = fs::read_dir(&path) {
                custom_grammars = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        let name = e.file_name();
                        let name_str = name.to_string_lossy();
                        !name_str.starts_with('.') && e.path().is_file()
                    })
                    .filter_map(|e| {
                        let path = e.path();
                        let ext = path.extension().and_then(|e| e.to_str());
                        if ext == Some("json") || ext == Some("plist") {
                            path.file_stem().map(|s| s.to_string_lossy().to_string())
                        } else {
                            None
                        }
                    })
                    .collect();
                custom_grammars.sort();
            }
        }
    }

    if plain {
        for grammar in &found_grammars {
            println!("{}", grammar);
        }
        for grammar in &custom_grammars {
            println!("{}", grammar);
        }
    } else {
        if !found_grammars.is_empty() {
            println!("Builtin grammars ({}):", found_grammars.len());
            for grammar in &found_grammars {
                println!("  {}", grammar);
            }
            println!();
        }

        if !custom_grammars.is_empty() {
            if let Some(ref grammars_path) = config.grammars_path {
                println!(
                    "Custom grammars from {} ({}):",
                    grammars_path,
                    custom_grammars.len()
                );
                for grammar in &custom_grammars {
                    println!("  {} (custom)", grammar);
                }
                println!();
            }
        }

        if found_grammars.is_empty() && custom_grammars.is_empty() {
            println!("  No grammars found.");
        }

        println!("Use in config.toml:");
        println!("  [language_map]");
        println!("  <filetype> = \"<grammar_id>\"");
        println!();
        println!("Or in Kakoune:");
        println!("  set-option buffer giallo_lang <grammar_id>");
    }
}

pub fn list_themes(registry: &Registry, config: &Config, plain: bool) {
    if !plain {
        println!("Available themes:");
        println!();
    }

    let common_themes = vec![
        "andromeeda",
        "aurora-x",
        "ayu-dark",
        "ayu-light",
        "ayu-mirage",
        "catppuccin-frappe",
        "catppuccin-latte",
        "catppuccin-macchiato",
        "catppuccin-mocha",
        "dark-plus",
        "dracula",
        "dracula-soft",
        "everforest-dark",
        "everforest-light",
        "github-dark",
        "github-dark-default",
        "github-dark-dimmed",
        "github-dark-high-contrast",
        "github-light",
        "github-light-default",
        "github-light-high-contrast",
        "gruvbox-dark-hard",
        "gruvbox-dark-medium",
        "gruvbox-dark-soft",
        "gruvbox-light-hard",
        "gruvbox-light-medium",
        "gruvbox-light-soft",
        "horizon",
        "houston",
        "kanagawa-dragon",
        "kanagawa-lotus",
        "kanagawa-wave",
        "laserwave",
        "light-plus",
        "material-theme",
        "material-theme-darker",
        "material-theme-lighter",
        "material-theme-ocean",
        "material-theme-palenight",
        "min-dark",
        "min-light",
        "monokai",
        "night-owl",
        "night-owl-light",
        "nord",
        "one-dark-pro",
        "one-light",
        "plastic",
        "poimandres",
        "red",
        "rose-pine",
        "rose-pine-dawn",
        "rose-pine-moon",
        "slack-dark",
        "slack-ochin",
        "snazzy-light",
        "solarized-dark",
        "solarized-light",
        "synthwave-84",
        "tokyo-night",
        "vesper",
        "vitesse-black",
        "vitesse-dark",
        "vitesse-light",
    ];

    let mut found_themes = Vec::new();
    for theme in &common_themes {
        if registry.contains_theme(theme) {
            found_themes.push(*theme);
        }
    }

    let mut custom_themes: Vec<String> = Vec::new();
    if let Some(ref themes_path) = config.themes_path {
        let path = expand_path(themes_path);
        if path.exists() {
            if let Ok(entries) = fs::read_dir(&path) {
                custom_themes = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        let name = e.file_name();
                        let name_str = name.to_string_lossy();
                        !name_str.starts_with('.') && e.path().is_file()
                    })
                    .filter_map(|e| {
                        let path = e.path();
                        let ext = path.extension().and_then(|e| e.to_str());
                        if ext == Some("json") {
                            path.file_stem().map(|s| s.to_string_lossy().to_string())
                        } else {
                            None
                        }
                    })
                    .collect();
                custom_themes.sort();
            }
        }
    }

    if plain {
        for theme in &found_themes {
            println!("{}", theme);
        }
        for theme in &custom_themes {
            println!("{}", theme);
        }
    } else {
        if !found_themes.is_empty() {
            println!("Builtin themes ({}):", found_themes.len());
            for theme in &found_themes {
                println!("  {}", theme);
            }
            println!();
        }

        if !custom_themes.is_empty() {
            if let Some(ref themes_path) = config.themes_path {
                println!(
                    "Custom themes from {} ({}):",
                    themes_path,
                    custom_themes.len()
                );
                for theme in &custom_themes {
                    println!("  {} (custom)", theme);
                }
                println!();
            }
        }

        if found_themes.is_empty() && custom_themes.is_empty() {
            println!("  No themes found.");
        }

        println!("Use in config.toml:");
        println!("  theme = \"<theme_name>\"");
        println!();
        println!("Or in Kakoune:");
        println!("  giallo-set-theme <theme_name>");
    }
}
