use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const DEFAULT_THEME: &str = "catppuccin-frappe";

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Config {
    pub theme: Option<String>,
    #[serde(default)]
    pub language_map: HashMap<String, String>,
    #[serde(default)]
    pub highlighter_map: HashMap<String, String>,
    #[serde(default)]
    pub grammars_path: Option<String>,
    #[serde(default)]
    pub themes_path: Option<String>,
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        let Ok(contents) = fs::read_to_string(&path) else {
            return Self::default();
        };
        match toml::from_str::<Config>(&contents) {
            Ok(config) => config,
            Err(err) => {
                eprintln!("config parse error ({}): {err}", path.display());
                Self::default()
            }
        }
    }

    pub fn resolve_lang(&self, lang: &str) -> String {
        self.language_map
            .get(lang)
            .cloned()
            .unwrap_or_else(|| lang.to_string())
    }

    pub fn resolve_highlighter(&self, lang: &str) -> String {
        self.highlighter_map
            .get(lang)
            .cloned()
            .unwrap_or_else(|| lang.to_string())
    }

    pub fn resolve_theme<'a>(&'a self, theme: &'a str) -> &'a str {
        if theme.is_empty() {
            self.theme.as_deref().unwrap_or(DEFAULT_THEME)
        } else {
            theme
        }
    }
}

pub fn config_path() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(dir).join("giallo.kak/config.toml")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config/giallo.kak/config.toml")
    } else {
        PathBuf::from("giallo.kak.toml")
    }
}

pub fn expand_path(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(&path[2..]);
        }
    }
    PathBuf::from(path)
}
