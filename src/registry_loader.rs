use std::io;
use std::path::Path;

use giallo::Registry;

use crate::config::expand_path;

#[derive(Deserialize)]
struct GrammarMeta {
    name: String,
    #[serde(default, rename = "fileTypes")]
    file_types: Vec<String>,
}

fn load_grammar_meta(path: &Path) -> Option<GrammarMeta> {
    if path
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
        .map_or(true, |ext| ext != "json")
    {
        return None;
    }

    let contents = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

fn file_stem_alias(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_string_lossy();
    let alias = stem.split('.').next()?.trim();
    if alias.is_empty() {
        None
    } else {
        Some(alias.to_lowercase())
    }
}

fn add_grammar_aliases(registry: &mut Registry, meta: &GrammarMeta, path: &Path) {
    let grammar_name = meta.name.trim();
    if grammar_name.is_empty() {
        return;
    }

    for file_type in &meta.file_types {
        let alias = file_type.trim();
        if !alias.is_empty() {
            registry.add_alias(grammar_name, alias);
        }
    }

    if let Some(alias) = file_stem_alias(path) {
        registry.add_alias(grammar_name, &alias);
    }
}

fn is_grammar_file(path: &Path) -> bool {
    path.extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
        .map_or(false, |ext| {
            matches!(ext.as_str(), "json" | "plist" | "tmlanguage")
        })
}

fn load_custom_grammars_in_dir(
    registry: &mut Registry,
    dir: &Path,
    loaded_count: &mut usize,
) -> io::Result<()> {
    let entries = std::fs::read_dir(dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            load_custom_grammars_in_dir(registry, &path, loaded_count)?;
            continue;
        }

        if !is_grammar_file(&path) {
            continue;
        }

        log::debug!("loading grammar from: {}", path.display());
        match registry.add_grammar_from_path(&path) {
            Ok(_) => {
                log::info!("loaded grammar: {}", path.display());
                *loaded_count += 1;
                if let Some(meta) = load_grammar_meta(&path) {
                    add_grammar_aliases(registry, &meta, &path);
                }
            }
            Err(err) => {
                log::error!("failed to load grammar {}: {}", path.display(), err);
            }
        }
    }

    Ok(())
}

pub fn load_custom_grammars(registry: &mut Registry, grammars_path: &str) -> io::Result<()> {
    let path = expand_path(grammars_path);
    let path_str = path.display().to_string();
    if !path.exists() {
        log::debug!("grammars path does not exist: {}", path_str);
        return Ok(());
    }

    let mut loaded_count = 0;
    load_custom_grammars_in_dir(registry, &path, &mut loaded_count)?;

    log::info!(
        "loaded {} custom grammars from {}",
        loaded_count,
        grammars_path
    );
    Ok(())
}

pub fn load_custom_themes(registry: &mut Registry, themes_path: &str) -> io::Result<()> {
    let path = expand_path(themes_path);
    let path_str = path.display().to_string();
    if !path.exists() {
        log::debug!("themes path does not exist: {}", path_str);
        return Ok(());
    }

    let mut loaded_count = 0;

    for entry in std::fs::read_dir(&path)? {
        let entry = entry?;
        let path = entry.path();

        if path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with('.'))
            .unwrap_or(true)
        {
            continue;
        }

        if !path.is_file() {
            continue;
        }

        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        match registry.add_theme_from_path(&path) {
            Ok(_) => {
                loaded_count += 1;
                log::debug!("loaded custom theme from {:?}", path);
            }
            Err(err) => {
                log::warn!("failed to load theme from {:?}: {}", path, err);
            }
        }
    }

    log::info!("loaded {} custom themes from {}", loaded_count, themes_path);
    Ok(())
}

use serde::Deserialize;
