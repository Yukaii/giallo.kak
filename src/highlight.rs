use std::collections::HashSet;
use std::fmt::Write as _;
use std::io::Write;
use std::process::Command;
use std::process::Stdio;
use std::sync::Mutex;

use giallo::{HighlightOptions, Registry, ThemeVariant, PLAIN_GRAMMAR_NAME};
use log;

use crate::config::Config;
use crate::highlighting::{
    build_commands, build_kakoune_commands, build_line_tokens, render_line, FaceAllocator,
    FaceDef, RangeToken,
};
use crate::kakoune::kak_quote;

/// Per-buffer incremental state, owned by the buffer's processor thread.
pub struct HighlightCache {
    pub lang: String,
    pub theme: String,
    pub text: String,
    pub allocator: FaceAllocator,
    /// Cached tokens for each line of the last successful highlight.
    pub line_tokens: Vec<Vec<RangeToken>>,
    /// Every face name ever sent to Kakoune for this buffer.
    pub known_faces: HashSet<String>,
    /// Chunks whose ranges-highlighter has been registered in Kakoune.
    /// Survives lang/theme resets since highlighters stay valid.
    pub ensured_chunks: HashSet<usize>,
}

impl Default for HighlightCache {
    fn default() -> Self {
        Self {
            lang: String::new(),
            theme: String::new(),
            text: String::new(),
            allocator: FaceAllocator::new(),
            line_tokens: Vec::new(),
            known_faces: HashSet::new(),
            ensured_chunks: HashSet::new(),
        }
    }
}

impl HighlightCache {
    /// Reset all state for a new (lang, theme) pair.
    pub fn reset_for(&mut self, lang: &str, theme: &str) {
        self.lang = lang.to_string();
        self.theme = theme.to_string();
        self.text.clear();
        self.allocator = FaceAllocator::new();
        self.line_tokens.clear();
        self.known_faces.clear();
    }

    /// Whether cached results can be reused for this (lang, theme).
    pub fn matches(&self, lang: &str, theme: &str) -> bool {
        self.lang == lang && self.theme == theme
    }
}

#[derive(Clone, Debug)]
pub struct BufferContext {
    pub session: String,
    pub buffer: String,
    pub sentinel: String,
    pub lang: std::sync::Arc<std::sync::Mutex<String>>,
    pub theme: std::sync::Arc<std::sync::Mutex<String>>,
}

impl BufferContext {
    pub fn new(
        session: String,
        buffer: String,
        sentinel: String,
        lang: String,
        theme: String,
    ) -> Self {
        Self {
            session,
            buffer,
            sentinel,
            lang: std::sync::Arc::new(std::sync::Mutex::new(lang)),
            theme: std::sync::Arc::new(std::sync::Mutex::new(theme)),
        }
    }
}

/// Lines per range-specs option chunk. Chunk 0 keeps the legacy
/// `giallo_hl_ranges` option name; chunk N uses `giallo_hl_ranges_N`.
pub const CHUNK_LINES: usize = 1000;

pub fn chunk_option_name(chunk: usize) -> String {
    if chunk == 0 {
        "giallo_hl_ranges".to_string()
    } else {
        format!("giallo_hl_ranges_{chunk}")
    }
}

fn chunk_slice<'a>(lines: &'a [Vec<RangeToken>], chunk: usize) -> &'a [Vec<RangeToken>] {
    let start = chunk * CHUNK_LINES;
    if start >= lines.len() {
        &[]
    } else {
        &lines[start..(start + CHUNK_LINES).min(lines.len())]
    }
}

/// Name of the ranges-highlighter covering `chunk`.
pub fn chunk_highlighter_name(chunk: usize) -> String {
    if chunk == 0 {
        "buffer/giallo".to_string()
    } else {
        format!("buffer/giallo_{chunk}")
    }
}

/// Build Kakoune commands that update only the chunks whose contents
/// changed between `old_lines` and `new_lines`. Returns None when nothing
/// changed, meaning nothing needs to be sent at all.
///
/// Commands are emitted with `-no-hooks`-compatible plain syntax; note that
/// commands delivered over the session socket run in a hook-less context,
/// so highlighter registration must be explicit (never hook-driven).
pub fn build_delta_commands(
    old_lines: &[Vec<RangeToken>],
    new_lines: &[Vec<RangeToken>],
    new_faces: &[FaceDef],
    ensured_chunks: &mut HashSet<usize>,
) -> Option<String> {
    let old_chunks = old_lines.len().div_ceil(CHUNK_LINES);
    let new_chunks = new_lines.len().div_ceil(CHUNK_LINES);

    let mut cmd = String::new();
    for face in new_faces {
        let _ = write!(cmd, "set-face global {} %{{{}}}\n", face.name, face.spec);
    }

    for chunk in 0..old_chunks.max(new_chunks) {
        let old_slice = chunk_slice(old_lines, chunk);
        let new_slice = chunk_slice(new_lines, chunk);
        if old_slice == new_slice {
            continue;
        }

        let name = chunk_option_name(chunk);
        if chunk >= new_chunks {
            // Chunk beyond the (shrunken) content: clear it.
            let _ = write!(cmd, "set-option buffer {name} %val{{timestamp}}\n");
            continue;
        }

        // Register the ranges-highlighter for this chunk once. Idempotent:
        // remove+add replaces any existing registration.
        if !ensured_chunks.contains(&chunk) {
            let hl = chunk_highlighter_name(chunk);
            let _ = write!(
                cmd,
                "try %{{ remove-highlighter {hl} }}; add-highlighter -override {hl} ranges {name}\n"
            );
            ensured_chunks.insert(chunk);
        }

        let mut val = String::new();
        let base = chunk * CHUNK_LINES;
        for (i, toks) in new_slice.iter().enumerate() {
            if toks.is_empty() {
                continue;
            }
            if !val.is_empty() {
                val.push(' ');
            }
            render_line(base + i + 1, toks, &mut val);
        }

        if val.is_empty() {
            let _ = write!(cmd, "set-option buffer {name} %val{{timestamp}}\n");
        } else {
            let _ = write!(cmd, "set-option buffer {name} %val{{timestamp}} {val}\n");
        }
    }

    if cmd.is_empty() {
        None
    } else {
        Some(cmd)
    }
}

pub fn highlight_and_send(
    text: &str,
    lang: &str,
    theme: &str,
    registry: &Registry,
    config: &Config,
    ctx: &BufferContext,
    cache: Option<&Mutex<HighlightCache>>,
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

    // Reset the cache when language or theme changed; otherwise reuse the
    // face allocator so face names stay stable across updates.
    if let Some(cache) = cache {
        let mut cache = cache.lock().unwrap();
        if !cache.matches(&resolved_lang, &resolved_theme) {
            log::debug!("highlight: resetting cache for buffer={} (lang/theme changed)", ctx.buffer);
            cache.reset_for(&resolved_lang, &resolved_theme);
        }
    }

    let options = HighlightOptions::new(&resolved_lang, ThemeVariant::Single(resolved_theme));    let highlighted = match registry.highlight(text, &options) {
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

    let (commands, line_count, face_count) = if let Some(cache) = cache {
        let mut cache = cache.lock().unwrap();
        let mut new_faces = Vec::new();
        let new_lines = build_line_tokens(&highlighted, &mut cache.allocator, &mut new_faces);
        for face in &new_faces {
            cache.known_faces.insert(face.name.clone());
        }
        let HighlightCache {
            line_tokens,
            ensured_chunks,
            ..
        } = &mut *cache;
        let commands = build_delta_commands(line_tokens, &new_lines, &new_faces, ensured_chunks);
        cache.text.clear();
        cache.text.push_str(text);
        cache.line_tokens = new_lines;
        (commands, cache.line_tokens.len(), new_faces.len())
    } else {
        let (faces, ranges) = build_kakoune_commands(&highlighted);
        (Some(build_commands(&faces, &ranges)), 0, faces.len())
    };

    log::debug!(
        "highlight: built {} faces across {} lines; {}",
        face_count,
        line_count,
        if commands.is_some() { "sending delta" } else { "no changes to send" }
    );

    match commands {
        Some(commands) => {
            log::trace!("highlight: sending commands:\n{}", commands);
            if let Err(err) = send_to_kak(&ctx.session, &ctx.buffer, &commands) {
                log::error!("highlight: failed to send to kak: {}", err);
                eprintln!("failed to send highlights to kak: {err}");
            } else {
                log::debug!("highlight: sent highlights to kak successfully");
            }
        }
        None => log::debug!("highlight: nothing changed, skipping send"),
    }
}

fn cached_kak_path() -> Option<&'static std::path::PathBuf> {
    static KAK_PATH: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();
    KAK_PATH.get_or_init(|| which::which("kak").ok()).as_ref()
}

pub fn send_to_kak(session: &str, buffer: &str, payload: &str) -> std::io::Result<()> {
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

    let preview_len = cmd.len().min(500);
    log::trace!("send_to_kak: command: {}", &cmd[..preview_len]);

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

    match crate::kakoune::send_command_to_session(session, &cmd) {
        Ok(()) => return Ok(()),
        Err(err) => {
            log::debug!(
                "send_to_kak: direct socket send failed ({err}), falling back to kak -p"
            );
        }
    }

    if cached_kak_path().is_none() {
        log::error!("send_to_kak: kak command not found in PATH");
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
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
        if !crate::server_resources::is_kakoune_session_alive(session) {
            log::info!("send_to_kak: Kakoune session '{session}' is no longer alive");
            return Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionReset,
                format!("session '{session}' is dead"),
            ));
        }
    }
    Ok(())
}
