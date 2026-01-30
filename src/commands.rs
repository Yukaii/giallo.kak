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
        "catppuccin-frappe",
        "catppuccin-latte",
        "catppuccin-macchiato",
        "catppuccin-mocha",
        "dracula",
        "dracula-soft",
        "gruvbox-dark-hard",
        "gruvbox-dark-medium",
        "gruvbox-dark-soft",
        "gruvbox-light-hard",
        "gruvbox-light-medium",
        "gruvbox-light-soft",
        "kanagawa-dragon",
        "kanagawa-lotus",
        "kanagawa-wave",
        "tokyo-night",
        "github-dark",
        "github-dark-default",
        "github-dark-dimmed",
        "github-light",
        "github-light-default",
        "monokai",
        "nord",
        "one-dark-pro",
        "rose-pine",
        "rose-pine-dawn",
        "rose-pine-moon",
        "solarized-dark",
        "solarized-light",
        "ayu-dark",
        "ayu-mirage",
        "vscode-dark",
        "dark-plus",
        "light-plus",
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
