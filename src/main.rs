use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use giallo::{HighlightOptions, Registry, ThemeVariant, PLAIN_GRAMMAR_NAME};
use log;
use serde::Deserialize;

mod server_resources;
use server_resources::ServerResources;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct StyleKey {
    fg: String,
    bg: String,
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
}

#[derive(Clone, Debug)]
struct FaceDef {
    name: String,
    spec: String,
}

#[derive(Clone, Debug)]
struct BufferContext {
    session: String,
    buffer: String,
    sentinel: String,
    lang: Arc<Mutex<String>>,
    theme: Arc<Mutex<String>>,
}

impl BufferContext {
    fn new(session: String, buffer: String, sentinel: String, lang: String, theme: String) -> Self {
        Self {
            session,
            buffer,
            sentinel,
            lang: Arc::new(Mutex::new(lang)),
            theme: Arc::new(Mutex::new(theme)),
        }
    }
}

fn normalize_hex(hex: &str) -> String {
    if hex.len() == 9 {
        hex[..7].to_string()
    } else {
        hex.to_string()
    }
}

fn style_key(style: &giallo::Style) -> StyleKey {
    StyleKey {
        fg: normalize_hex(&style.foreground.as_hex()),
        bg: normalize_hex(&style.background.as_hex()),
        bold: style.font_style.contains(giallo::FontStyle::BOLD),
        italic: style.font_style.contains(giallo::FontStyle::ITALIC),
        underline: style.font_style.contains(giallo::FontStyle::UNDERLINE),
        strike: style.font_style.contains(giallo::FontStyle::STRIKETHROUGH),
    }
}

fn strip_hash(hex: &str) -> &str {
    if hex.starts_with('#') {
        &hex[1..]
    } else {
        hex
    }
}

fn style_to_face_spec(style: &giallo::Style, default_bg: Option<&str>) -> String {
    let mut attrs = String::new();
    if style.font_style.contains(giallo::FontStyle::BOLD) {
        attrs.push('b');
    }
    if style.font_style.contains(giallo::FontStyle::ITALIC) {
        attrs.push('i');
    }
    if style.font_style.contains(giallo::FontStyle::UNDERLINE) {
        attrs.push('u');
    }
    if style.font_style.contains(giallo::FontStyle::STRIKETHROUGH) {
        attrs.push('s');
    }

    let fg_hex = normalize_hex(&style.foreground.as_hex());
    let bg_hex = normalize_hex(&style.background.as_hex());
    let fg = strip_hash(&fg_hex);
    let bg = strip_hash(&bg_hex);

    // If background matches default theme background, use "default" to preserve terminal transparency
    let bg_spec = if let Some(default_bg_hex) = default_bg {
        if strip_hash(default_bg_hex) == bg {
            String::from("default")
        } else {
            format!("rgb:{bg}")
        }
    } else {
        format!("rgb:{bg}")
    };

    if attrs.is_empty() {
        format!("rgb:{fg},{bg_spec}")
    } else {
        format!("rgb:{fg},{bg_spec}+{attrs}")
    }
}

fn build_kakoune_commands(highlighted: &giallo::HighlightedCode<'_>) -> (Vec<FaceDef>, String) {
    let theme = match highlighted.theme {
        ThemeVariant::Single(theme) => theme,
        ThemeVariant::Dual { light, .. } => light,
    };

    let default_style = theme.default_style;
    let default_bg = default_style.background.as_hex();

    let mut faces: Vec<FaceDef> = Vec::new();
    let mut face_map: HashMap<StyleKey, String> = HashMap::new();
    let mut face_counter = 0usize;

    let mut ranges: Vec<String> = Vec::new();

    for (line_idx, line_tokens) in highlighted.tokens.iter().enumerate() {
        let mut col = 0usize;
        for token in line_tokens {
            if token.text.is_empty() {
                continue;
            }

            let bytes = token.text.as_bytes().len();
            let start = col;
            let end_excl = col + bytes;
            col = end_excl;

            let ThemeVariant::Single(style) = token.style else {
                continue;
            };

            let face_name = if style == default_style {
                "default".to_string()
            } else {
                let key = style_key(&style);
                if let Some(name) = face_map.get(&key) {
                    name.clone()
                } else {
                    face_counter += 1;
                    let name = format!("giallo_{face_counter:04}");
                    let spec = style_to_face_spec(&style, Some(&default_bg));
                    faces.push(FaceDef {
                        name: name.clone(),
                        spec,
                    });
                    face_map.insert(key, name.clone());
                    name
                }
            };

            let line = line_idx + 1;
            let col_start = start + 1;
            let col_end = end_excl.max(1);

            ranges.push(format!("{line}.{col_start},{line}.{col_end}|{face_name}"));
        }
    }

    let ranges_str = if ranges.is_empty() {
        String::new()
    } else {
        ranges.join(" ")
    };

    (faces, ranges_str)
}

fn build_commands(faces: &[FaceDef], ranges: &str) -> String {
    let mut commands = String::new();
    for face in faces {
        // Quote the face spec to handle # characters in hex colors
        commands.push_str("set-face global ");
        commands.push_str(&face.name);
        commands.push_str(" %{");
        commands.push_str(&face.spec);
        commands.push_str("}\n");
    }

    commands.push_str("set-option buffer giallo_hl_ranges %val{timestamp}");
    if !ranges.is_empty() {
        commands.push(' ');
        commands.push_str(ranges);
    }
    commands.push('\n');

    commands
}

#[allow(dead_code)]
fn write_response(mut out: impl Write, commands: &str) -> io::Result<()> {
    let len = commands.as_bytes().len();
    writeln!(out, "OK {len}")?;
    out.write_all(commands.as_bytes())?;
    out.flush()
}

#[allow(dead_code)]
fn read_exact_bytes(reader: &mut impl Read, len: usize) -> io::Result<Vec<u8>> {
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

fn kak_quote(input: &str) -> String {
    input.replace('\'', "''")
}

fn send_to_kak(session: &str, buffer: &str, payload: &str) -> io::Result<()> {
    let mut cmd = String::new();
    cmd.push_str("evaluate-commands -no-hooks -buffer '");
    cmd.push_str(&kak_quote(buffer));
    cmd.push_str("' -- %[ ");
    cmd.push_str(payload);
    cmd.push_str(" ]\n");

    log::trace!(
        "send_to_kak: sending {} bytes to kak -p {}",
        cmd.len(),
        session
    );

    // Log the full payload for debugging
    let preview_len = cmd.len().min(500);
    log::trace!("send_to_kak: command: {}", &cmd[..preview_len]);

    // Write commands to debug file if GIALLO_DEBUG_FILE env var is set
    if let Ok(debug_file) = std::env::var("GIALLO_DEBUG_FILE") {
        let debug_path = std::path::Path::new(&debug_file);
        let debug_dir = debug_path.parent().unwrap_or(std::path::Path::new("."));
        if let Err(e) = std::fs::create_dir_all(debug_dir) {
            log::warn!("Failed to create debug directory: {}", e);
        }
        if let Err(e) = std::fs::write(debug_path, &cmd) {
            log::warn!("Failed to write debug file: {}", e);
        } else {
            log::debug!("Wrote commands to debug file: {}", debug_file);
        }
    }

    // Check if kak is available
    if which::which("kak").is_err() {
        log::error!("send_to_kak: kak command not found in PATH");
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "kak command not found",
        ));
    }

    let mut child = Command::new("kak")
        .arg("-p")
        .arg(session)
        .stdin(Stdio::piped())
        .spawn()?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(cmd.as_bytes())?;
    }
    let status = child.wait()?;
    if !status.success() {
        log::warn!("send_to_kak: kak -p returned exit code {:?}", status.code());
    }
    Ok(())
}

fn highlight_and_send(
    text: &str,
    lang: &str,
    theme: &str,
    registry: &Registry,
    config: &Config,
    ctx: &BufferContext,
) {
    let resolved_lang = config.resolve_lang(lang);
    let resolved_theme = config.resolve_theme(theme);

    log::debug!(
        "highlight: buffer={} lang={} (resolved={}) theme={} (resolved={}) text_len={}",
        ctx.buffer,
        lang,
        resolved_lang,
        theme,
        resolved_theme,
        text.len()
    );

    let options = HighlightOptions::new(&resolved_lang, ThemeVariant::Single(resolved_theme));
    let highlighted = match registry.highlight(text, &options) {
        Ok(h) => {
            log::debug!("highlight: success for {} tokens", h.tokens.len());
            h
        }
        Err(err) => {
            log::warn!(
                "highlight: failed for lang={} theme={}: {}",
                resolved_lang,
                resolved_theme,
                err
            );
            log::warn!(
                "highlight: failed with lang={}, trying plain: {}",
                resolved_lang,
                err
            );
            let fallback =
                HighlightOptions::new(PLAIN_GRAMMAR_NAME, ThemeVariant::Single(resolved_theme));
            match registry.highlight(text, &fallback) {
                Ok(h) => {
                    log::debug!("highlight: fallback success for {} tokens", h.tokens.len());
                    h
                }
                Err(err) => {
                    log::error!("highlight: fallback also failed: {}", err);
                    eprintln!("highlight error: {err}");
                    return;
                }
            }
        }
    };

    let (faces, ranges) = build_kakoune_commands(&highlighted);
    log::debug!(
        "highlight: built {} faces and {} ranges",
        faces.len(),
        if ranges.is_empty() {
            0
        } else {
            ranges.split_whitespace().count()
        }
    );

    let commands = build_commands(&faces, &ranges);
    log::trace!("highlight: sending commands:\n{}", commands);

    if let Err(err) = send_to_kak(&ctx.session, &ctx.buffer, &commands) {
        log::error!("highlight: failed to send to kak: {}", err);
        eprintln!("failed to send highlights to kak: {err}");
    } else {
        log::debug!("highlight: sent highlights to kak successfully");
    }
}

fn run_buffer_fifo(
    req_path: &Path,
    registry: &Registry,
    config: &Config,
    ctx: BufferContext,
    quit_flag: Option<&Arc<AtomicBool>>,
) -> io::Result<()> {
    log::debug!(
        "buffer FIFO: starting for buffer={} sentinel={}",
        ctx.buffer,
        ctx.sentinel
    );

    // Create a channel to decouple reading from processing
    let (tx, rx): (Sender<String>, Receiver<String>) = channel();

    // Clone context and quit flag for the reader thread
    let ctx_clone = ctx.clone();
    let quit_flag_clone = quit_flag.map(|f| f.clone());
    let req_path_owned = req_path.to_path_buf();

    // Spawn reader thread - continuously reads from FIFO
    let reader_handle = thread::spawn(move || {
        let mut buf = String::new();
        let sentinel = ctx_clone.sentinel.clone(); // Clone sentinel to avoid borrow issues

        // Open the FIFO read-only in non-blocking mode
        let mut file = match open_fifo_nonblocking(&req_path_owned) {
            Ok(f) => f,
            Err(err) => {
                log::error!("reader: failed to open FIFO: {}", err);
                return;
            }
        };

        loop {
            // Check quit signal
            if let Some(ref flag) = quit_flag_clone {
                if flag.load(Ordering::Relaxed) {
                    break;
                }
            }

            // Try to read data from FIFO
            let mut read_buf = String::new();
            match file.read_to_string(&mut read_buf) {
                Ok(0) => {
                    // EOF - writer closed, wait a bit for next write
                    thread::sleep(std::time::Duration::from_millis(5));
                    continue;
                }
                Ok(_) => {
                    buf.push_str(&read_buf);
                }
                Err(err) => {
                    if err.kind() == io::ErrorKind::WouldBlock {
                        thread::sleep(std::time::Duration::from_millis(5));
                        continue;
                    } else {
                        log::warn!("reader: read error: {}", err);
                        thread::sleep(std::time::Duration::from_millis(50));
                        continue;
                    }
                }
            }

            // Check for complete messages (sentinel found)
            while let Some(index) = buf.find(&sentinel) {
                let content = buf[..index].to_string();
                let end_index = index + sentinel.len();
                buf.drain(..end_index);

                // Send complete message to processing thread
                if tx.send(content).is_err() {
                    log::debug!("reader: channel closed, exiting");
                    return;
                }
            }
        }
    });

    // Processing loop - receives messages and processes highlights
    loop {
        // Check quit signal
        if let Some(flag) = quit_flag {
            if flag.load(Ordering::Relaxed) {
                // Drop the receiver to close the channel, which will cause
                // the reader thread to get an error on send and exit
                drop(rx);
                let _ = reader_handle.join();
                break;
            }
        }

        // Try to receive a message with timeout
        match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(content) => {
                let lang = ctx.lang.lock().unwrap().clone();
                let theme = ctx.theme.lock().unwrap().clone();

                log::debug!(
                    "processor: received buffer (lang={} theme={} len={})",
                    lang,
                    theme,
                    content.len()
                );

                if !lang.is_empty() {
                    highlight_and_send(&content, &lang, &theme, registry, config, &ctx);
                } else {
                    log::warn!(
                        "processor: empty language, skipping highlight for buffer={}",
                        ctx.buffer
                    );
                }
            }
            Err(_) => {
                // Timeout - check quit signal and continue
                continue;
            }
        }
    }

    log::debug!("buffer FIFO: exiting for buffer={}", ctx.buffer);
    Ok(())
}

enum Mode {
    Stdio,
    Oneshoot,
    Fifo { req: String, resp: Option<String> },
    KakouneRc,
    ListGrammars,
    ListThemes,
}

fn parse_args() -> (Mode, bool) {
    let mut oneshot = false;
    let mut fifo_req: Option<String> = None;
    let mut fifo_resp: Option<String> = None;
    let mut kakoune_rc = false;
    let mut verbose = false;
    let mut list_grammars = false;
    let mut list_themes = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" => {
                let commit = option_env!("GIT_COMMIT").unwrap_or("unknown");
                println!("giallo-kak {} ({})", env!("CARGO_PKG_VERSION"), commit);
                process::exit(0);
            }
            "--verbose" | "-v" => verbose = true,
            "--oneshot" => oneshot = true,
            "init" | "--kakoune" | "--print-rc" => kakoune_rc = true,
            "list-grammars" | "--list-grammars" => list_grammars = true,
            "list-themes" | "--list-themes" => list_themes = true,
            "--fifo" => {
                if let Some(path) = args.next() {
                    fifo_req = Some(path);
                }
            }
            "--resp" => {
                if let Some(path) = args.next() {
                    fifo_resp = Some(path);
                }
            }
            _ => {}
        }
    }

    let mode = if list_grammars {
        Mode::ListGrammars
    } else if list_themes {
        Mode::ListThemes
    } else if let Some(req) = fifo_req {
        Mode::Fifo {
            req,
            resp: fifo_resp,
        }
    } else if kakoune_rc {
        Mode::KakouneRc
    } else if oneshot {
        Mode::Oneshoot
    } else {
        Mode::Stdio
    };

    (mode, verbose)
}

fn token_hash(token: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    token.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

#[derive(Clone, Debug, Default, Deserialize)]
struct Config {
    theme: Option<String>,
    #[serde(default)]
    language_map: HashMap<String, String>,
    #[serde(default)]
    grammars_path: Option<String>,
    #[serde(default)]
    themes_path: Option<String>,
}

impl Config {
    fn load() -> Self {
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

    fn resolve_lang(&self, lang: &str) -> String {
        self.language_map
            .get(lang)
            .cloned()
            .unwrap_or_else(|| lang.to_string())
    }

    fn resolve_theme<'a>(&'a self, theme: &'a str) -> &'a str {
        if theme.is_empty() {
            self.theme.as_deref().unwrap_or(DEFAULT_THEME)
        } else {
            theme
        }
    }
}

const DEFAULT_THEME: &str = "catppuccin-frappe";

/// Expand ~ to home directory in path
fn expand_path(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(&path[2..]);
        }
    }
    PathBuf::from(path)
}

/// Load custom grammars from the given directory path
fn is_grammar_file(path: &Path) -> bool {
    path.extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
        .map_or(false, |ext| {
            matches!(ext.as_str(), "json" | "plist" | "tmlanguage")
        })
}

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

    let contents = fs::read_to_string(path).ok()?;
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

fn load_custom_grammars_in_dir(
    registry: &mut Registry,
    dir: &Path,
    loaded_count: &mut usize,
) -> io::Result<()> {
    let entries = fs::read_dir(dir)?;

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

fn load_custom_grammars(registry: &mut Registry, grammars_path: &str) -> io::Result<()> {
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

/// Load custom themes from the given directory path
fn load_custom_themes(registry: &mut Registry, themes_path: &str) -> io::Result<()> {
    let path = expand_path(themes_path);
    let path_str = path.display().to_string();
    if !path.exists() {
        log::debug!("themes path does not exist: {}", path_str);
        return Ok(());
    }

    let mut loaded_count = 0;

    for entry in fs::read_dir(&path)? {
        let entry = entry?;
        let path = entry.path();

        // Skip hidden files and non-files
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

        // Check if it's a JSON file
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

/// List all available grammars (builtin + custom)
fn list_grammars(registry: &Registry, config: &Config) {
    println!("Available grammars:");
    println!();

    // Get builtin grammars from the registry
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

    if !found_grammars.is_empty() {
        println!("Builtin grammars ({}):", found_grammars.len());
        for grammar in &found_grammars {
            println!("  {}", grammar);
        }
        println!();
    }

    // List custom grammars from directory
    if let Some(ref grammars_path) = config.grammars_path {
        let path = expand_path(grammars_path);
        if path.exists() {
            let mut custom_count = 0;
            if let Ok(entries) = fs::read_dir(&path) {
                let mut custom_grammars: Vec<String> = entries
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
                custom_count = custom_grammars.len();

                if custom_count > 0 {
                    println!("Custom grammars from {} ({}):", grammars_path, custom_count);
                    for grammar in custom_grammars {
                        println!("  {} (custom)", grammar);
                    }
                    println!();
                }
            }

            if custom_count == 0 && found_grammars.is_empty() {
                println!("  No grammars found.");
            }
        } else {
            if found_grammars.is_empty() {
                println!("  No grammars found.");
            }
            println!(
                "Custom grammars directory does not exist: {}",
                grammars_path
            );
        }
    } else if found_grammars.is_empty() {
        println!("  No grammars found.");
    }

    println!("Use in config.toml:");
    println!("  [language_map]");
    println!("  <filetype> = \"<grammar_id>\"");
    println!();
    println!("Or in Kakoune:");
    println!("  set-option buffer giallo_lang <grammar_id>");
}

/// List all available themes (builtin + custom)
fn list_themes(registry: &Registry, config: &Config) {
    println!("Available themes:");
    println!();

    // Common builtin themes
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

    if !found_themes.is_empty() {
        println!("Builtin themes ({}):", found_themes.len());
        for theme in &found_themes {
            println!("  {}", theme);
        }
        println!();
    }

    // List custom themes from directory
    if let Some(ref themes_path) = config.themes_path {
        let path = expand_path(themes_path);
        if path.exists() {
            let mut custom_count = 0;
            if let Ok(entries) = fs::read_dir(&path) {
                let mut custom_themes: Vec<String> = entries
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
                custom_count = custom_themes.len();

                if custom_count > 0 {
                    println!("Custom themes from {} ({}):", themes_path, custom_count);
                    for theme in custom_themes {
                        println!("  {} (custom)", theme);
                    }
                    println!();
                }
            }

            if custom_count == 0 && found_themes.is_empty() {
                println!("  No themes found.");
            }
        } else {
            if found_themes.is_empty() {
                println!("  No themes found.");
            }
            println!("Custom themes directory does not exist: {}", themes_path);
        }
    } else if found_themes.is_empty() {
        println!("  No themes found.");
    }

    println!("Use in config.toml:");
    println!("  theme = \"<theme_name>\"");
    println!();
    println!("Or in Kakoune:");
    println!("  giallo-set-theme <theme_name>");
}

fn config_path() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(dir).join("giallo.kak/config.toml")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config/giallo.kak/config.toml")
    } else {
        PathBuf::from("giallo.kak.toml")
    }
}

fn create_fifo(path: &Path) -> io::Result<()> {
    let c_path = std::ffi::CString::new(path.as_os_str().to_string_lossy().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid fifo path"))?;
    let ret = unsafe { libc::mkfifo(c_path.as_ptr(), 0o644) };
    if ret != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Open a FIFO for reading without blocking.
///
/// Opening a FIFO for reading normally blocks until a writer opens it.
/// We use O_NONBLOCK to open it immediately, then clear the non-blocking flag
/// so subsequent reads block normally (which is what we want for reading data).
fn open_fifo_nonblocking(path: &Path) -> io::Result<std::fs::File> {
    use std::os::fd::FromRawFd;

    let c_path = std::ffi::CString::new(path.as_os_str().to_string_lossy().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid fifo path"))?;

    // Open with O_NONBLOCK to prevent blocking on open()
    let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDONLY | libc::O_NONBLOCK, 0o644) };

    if fd < 0 {
        return Err(io::Error::last_os_error());
    }

    // Clear the O_NONBLOCK flag so reads block normally
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL, 0) };
    if flags < 0 {
        unsafe { libc::close(fd) };
        return Err(io::Error::last_os_error());
    }

    let ret = unsafe { libc::fcntl(fd, libc::F_SETFL, flags & !libc::O_NONBLOCK) };
    if ret < 0 {
        unsafe { libc::close(fd) };
        return Err(io::Error::last_os_error());
    }

    // Convert to std::fs::File
    let file = unsafe { std::fs::File::from_raw_fd(fd) };
    Ok(file)
}

fn handle_init(token: &str, base_dir: &Path) -> io::Result<(PathBuf, String)> {
    fs::create_dir_all(base_dir)?;
    let hash = token_hash(token);
    let req = base_dir.join(format!("{hash}.req.fifo"));
    let sentinel = format!("giallo-{hash}");

    if !req.exists() {
        create_fifo(&req)?;
    }

    Ok((req, sentinel))
}

#[allow(unused_variables)]
fn run_server<R: BufRead, W: Write>(
    mut reader: R,
    mut writer: W,
    registry: Arc<Registry>,
    config: &Config,
    oneshot: bool,
    base_dir: Option<&Path>,
    ctx: Option<BufferContext>,
    resources: &ServerResources,
) -> io::Result<()> {
    let mut line = String::new();
    let mut buffer_contexts: std::collections::HashMap<String, BufferContext> =
        std::collections::HashMap::new();
    loop {
        // Check quit signal
        if resources.should_quit() {
            log::info!("Quit signal received, exiting server loop");
            break;
        }

        line.clear();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            break;
        }

        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }

        log::trace!("received: {}", line);

        if line == "PING" {
            log::trace!("responding with PONG");
            writeln!(writer, "PONG").ok();
            writer.flush().ok();
            continue;
        }

        let mut parts = line.split_whitespace();
        let cmd = parts.next().unwrap_or("");

        if cmd == "INIT" {
            let session = match parts.next() {
                Some(v) => v.to_string(),
                None => {
                    log::error!("INIT: missing session");
                    eprintln!("missing session");
                    continue;
                }
            };
            let buffer = match parts.next() {
                Some(v) => v.to_string(),
                None => {
                    log::error!("INIT: missing buffer");
                    eprintln!("missing buffer");
                    continue;
                }
            };
            let token = match parts.next() {
                Some(v) => v.to_string(),
                None => {
                    log::error!("INIT: missing token");
                    eprintln!("missing token");
                    continue;
                }
            };
            let lang = parts.next().unwrap_or("").to_string();
            let theme = parts.next().unwrap_or("").to_string();
            log::debug!(
                "INIT: session={} buffer={} token={} lang={} theme={}",
                session,
                buffer,
                token,
                lang,
                theme
            );

            let Some(base_dir) = base_dir else {
                log::error!("INIT: init not supported in this mode");
                eprintln!("init not supported in this mode");
                continue;
            };

            let (req, sentinel) = match handle_init(&token, base_dir) {
                Ok(v) => v,
                Err(err) => {
                    log::error!("INIT: error creating FIFO: {}", err);
                    eprintln!("init error: {err}");
                    continue;
                }
            };
            log::debug!(
                "INIT: created buffer FIFO at {} with sentinel {}",
                req.display(),
                sentinel
            );

            let commands = format!(
                "set-option buffer giallo_buf_fifo_path {req}\nset-option buffer giallo_buf_sentinel {sentinel}\n",
                req = req.display(),
                sentinel = sentinel
            );

            let req_path = req.clone();
            let token_clone = token.clone();
            let config_clone = config.clone();
            let ctx = BufferContext::new(
                session.clone(),
                buffer.clone(),
                sentinel.clone(),
                lang.clone(),
                theme.clone(),
            );
            // Clone for storage in map (before moving to thread)
            let ctx_for_map = ctx.clone();
            log::debug!("INIT: spawning buffer handler thread");
            let thread_quit_flag = resources.quit_flag();
            let thread_registry = registry.clone();
            thread::spawn(move || {
                log::debug!("buffer thread: starting for {}", token_clone);
                log::debug!("buffer thread: using shared registry for {}", token_clone);

                match run_buffer_fifo(
                    &req_path,
                    &thread_registry,
                    &config_clone,
                    ctx,
                    Some(&thread_quit_flag),
                ) {
                    Ok(_) => log::debug!("buffer thread: completed normally for {}", token_clone),
                    Err(err) => log::error!("buffer thread: error for {}: {}", token_clone, err),
                }

                let _ = fs::remove_file(&req_path);
                log::debug!("buffer thread: exiting for {}", token_clone);
            });

            // Store context in map for later updates
            buffer_contexts.insert(buffer.clone(), ctx_for_map);

            if let Err(err) = send_to_kak(&session, &buffer, &commands) {
                log::error!("INIT: failed to send init to kak: {}", err);
                eprintln!("failed to send init to kak: {err}");
            } else {
                log::debug!("INIT: sent buffer options to kak");
            }
            continue;
        }

        if cmd == "SET_THEME" {
            let buffer = match parts.next() {
                Some(v) => v.to_string(),
                None => {
                    log::error!("SET_THEME: missing buffer");
                    continue;
                }
            };
            let theme = match parts.next() {
                Some(v) => v.to_string(),
                None => {
                    log::error!("SET_THEME: missing theme");
                    continue;
                }
            };

            if let Some(ctx) = buffer_contexts.get(&buffer) {
                let mut ctx_theme = ctx.theme.lock().unwrap();
                *ctx_theme = theme.clone();
                log::debug!("SET_THEME: updated buffer={} theme={}", buffer, theme);
            } else {
                log::warn!("SET_THEME: buffer={} not found", buffer);
            }
            continue;
        }

        eprintln!("unknown command: {cmd}");
        continue;
    }

    Ok(())
}

fn main() {
    let (mode, verbose) = parse_args();
    let base_dir = std::env::temp_dir().join(format!("giallo-kak-{}", process::id()));

    if let Mode::KakouneRc = mode {
        const RC: &str = include_str!("../rc/giallo.kak");
        println!("{RC}");
        return;
    }

    if verbose {
        simple_logger::init_with_level(log::Level::Debug).expect("failed to initialize logging");
    }

    log::info!("starting giallo-kak server");
    log::debug!("base_dir: {}", base_dir.display());

    // Create server resources for cleanup management
    let resources = ServerResources::new(base_dir.clone());

    // Setup signal handler for graceful shutdown
    if let Err(e) = resources.setup_signal_handler() {
        log::warn!("failed to setup signal handler: {}", e);
    } else {
        log::debug!("signal handler installed successfully");
    }

    let mut registry = match Registry::builtin() {
        Ok(registry) => registry,
        Err(err) => {
            log::error!("failed to load giallo registry: {err}");
            eprintln!("failed to load giallo registry: {err}");
            process::exit(1);
        }
    };
    log::debug!("registry loaded successfully");

    let config = Config::load();
    log::debug!("config loaded: {:?}", config);

    // Load custom grammars from config
    if let Some(ref grammars_path) = config.grammars_path {
        if let Err(err) = load_custom_grammars(&mut registry, grammars_path) {
            log::error!("failed to load custom grammars: {err}");
            eprintln!("warning: failed to load custom grammars: {err}");
        }
    }

    // Load custom themes from config
    if let Some(ref themes_path) = config.themes_path {
        if let Err(err) = load_custom_themes(&mut registry, themes_path) {
            log::error!("failed to load custom themes: {err}");
            eprintln!("warning: failed to load custom themes: {err}");
        }
    }

    registry.link_grammars();
    log::debug!("grammars linked");

    // Wrap registry in Arc for sharing across threads
    let registry = Arc::new(registry);
    log::debug!("registry wrapped in Arc for thread sharing");

    match mode {
        Mode::Stdio => {
            log::debug!("running in stdio mode");
            let stdin = io::stdin();
            let stdout = io::stdout();
            let mut stdin_lock = stdin.lock();
            let mut stdout_lock = stdout.lock();
            if let Err(err) = run_server(
                &mut stdin_lock,
                &mut stdout_lock,
                Arc::clone(&registry),
                &config,
                false,
                Some(&base_dir),
                None,
                &resources,
            ) {
                log::error!("server error: {err}");
                eprintln!("server error: {err}");
            }
        }
        Mode::Oneshoot => {
            log::debug!("running in oneshot mode");
            let stdin = io::stdin();
            let stdout = io::stdout();
            let mut stdin_lock = stdin.lock();
            let mut stdout_lock = stdout.lock();
            if let Err(err) = run_server(
                &mut stdin_lock,
                &mut stdout_lock,
                Arc::clone(&registry),
                &config,
                true,
                Some(&base_dir),
                None,
                &resources,
            ) {
                log::error!("oneshot error: {err}");
                eprintln!("oneshot error: {err}");
            }
        }
        Mode::Fifo { req, resp } => {
            log::debug!("running in fifo mode");
            log::debug!("req fifo: {req}");
            if let Some(ref r) = resp {
                log::debug!("resp fifo: {r}");
            }

            let req_file = match OpenOptions::new().read(true).write(true).open(&req) {
                Ok(file) => file,
                Err(err) => {
                    log::error!("failed to open fifo for read: {req}: {err}");
                    eprintln!("failed to open fifo for read: {req}: {err}");
                    process::exit(1);
                }
            };

            let mut req_reader = io::BufReader::new(req_file);

            let mut resp_writer: Box<dyn Write> = if let Some(resp_path) = resp {
                match OpenOptions::new().write(true).open(&resp_path) {
                    Ok(file) => Box::new(file),
                    Err(err) => {
                        log::error!("failed to open fifo for write: {resp_path}: {err}");
                        eprintln!("failed to open fifo for write: {resp_path}: {err}");
                        process::exit(1);
                    }
                }
            } else {
                Box::new(io::stdout())
            };

            if let Err(err) = run_server(
                &mut req_reader,
                &mut resp_writer,
                Arc::clone(&registry),
                &config,
                false,
                Some(&base_dir),
                None,
                &resources,
            ) {
                log::error!("fifo server error: {err}");
                eprintln!("fifo server error: {err}");
            }
        }
        Mode::ListGrammars => {
            list_grammars(&registry, &config);
        }
        Mode::ListThemes => {
            list_themes(&registry, &config);
        }
        Mode::KakouneRc => unreachable!(),
    }

    log::info!("giallo-kak server exiting");
}
