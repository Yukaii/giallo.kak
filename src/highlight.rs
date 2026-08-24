use std::collections::HashSet;
use std::fmt::Write as _;
use std::io::Write;
use std::process::Command;
use std::process::Stdio;
use std::sync::Mutex;

use giallo::{HighlightOptions, Registry, ThemeVariant, PLAIN_GRAMMAR_NAME};
use log;

use crate::config::{Config, Tuning};
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
    /// Windowed updates since the last full parse.
    pub updates_since_full: usize,
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
            updates_since_full: 0,
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

pub fn chunk_option_name(chunk: usize) -> String {
    if chunk == 0 {
        "giallo_hl_ranges".to_string()
    } else {
        format!("giallo_hl_ranges_{chunk}")
    }
}

fn chunk_slice<'a>(
    lines: &'a [Vec<RangeToken>],
    chunk: usize,
    chunk_lines: usize,
) -> &'a [Vec<RangeToken>] {
    let start = chunk * chunk_lines;
    if start >= lines.len() {
        &[]
    } else {
        &lines[start..(start + chunk_lines).min(lines.len())]
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
    tuning: &Tuning,
    old_lines: &[Vec<RangeToken>],
    new_lines: &[Vec<RangeToken>],
    new_faces: &[FaceDef],
    ensured_chunks: &mut HashSet<usize>,
) -> Option<String> {
    let chunk_lines = tuning.chunk_lines;
    let old_chunks = old_lines.len().div_ceil(chunk_lines);
    let new_chunks = new_lines.len().div_ceil(chunk_lines);

    let mut cmd = String::new();
    for face in new_faces {
        let _ = write!(cmd, "set-face global {} %{{{}}}\n", face.name, face.spec);
    }

    for chunk in 0..old_chunks.max(new_chunks) {
        let old_slice = chunk_slice(old_lines, chunk, chunk_lines);
        let new_slice = chunk_slice(new_lines, chunk, chunk_lines);
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
        let base = chunk * chunk_lines;
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

/// How much of the document needs re-parsing for an update.
pub enum Plan {
    /// Text unchanged since the cached highlight: nothing to do.
    NoChange,
    /// Re-parse the entire text.
    Full,
    /// Re-parse a bounded window of the document and splice.
    Window(SplicePlan),
}

/// Describes a bounded re-parse: which new-text lines to parse and which
/// cached token range to replace.
#[derive(Debug)]
pub struct SplicePlan {
    /// First new-text line fed to the parser (warmup before the edit).
    pub window_start: usize,
    /// Last new-text line fed to the parser.
    pub window_end: usize,
    /// First new-text line whose cached tokens get replaced.
    pub dirty_start: usize,
    /// Last changed line in the new text.
    pub dirty_end_new: usize,
    /// Last changed line in the old text.
    pub dirty_end_old: usize,
}

/// Whether a windowed plan should be escalated to a full re-parse because
/// `updates_since_full` windowed updates have accumulated. Periodic full
/// refreshes let any grammar-state divergence across splices self-heal.
pub fn escalate_to_full(plan: Plan, updates_since_full: usize, interval: usize) -> Plan {
    match plan {
        Plan::Window(_) if updates_since_full + 1 >= interval => Plan::Full,
        other => other,
    }
}

fn count_lines(s: &str) -> usize {
    s.bytes().filter(|&b| b == b'\n').count()
}

/// Byte offsets `(start, end)` into `new` of the first differing region,
/// or None when the texts are equal.
fn changed_byte_span(old: &str, new: &str) -> Option<(usize, usize)> {
    if old == new {
        return None;
    }
    let prefix = old
        .bytes()
        .zip(new.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    let suffix = old[prefix..]
        .bytes()
        .rev()
        .zip(new[prefix..].bytes().rev())
        .take_while(|(a, b)| a == b)
        .count();
    Some((prefix, new.len() - suffix))
}

/// Characters that can open or close multiline constructs (strings,
/// comments). An edit touching these forces a conservative full re-parse.
fn risky_edit(region: &str) -> bool {
    region.contains(['"', '\'', '`', '#']) || region.contains("/*") || region.contains("*/")
}

/// Decide how to parse `new_text` given the previously highlighted
/// `old_text`. Returns Full for first highlights, risky or oversized edits.
pub fn parse_plan(tuning: &Tuning, old_text: &str, new_text: &str) -> Plan {
    if old_text == new_text {
        return Plan::NoChange;
    }
    let Some((bstart, bend_new)) = changed_byte_span(old_text, new_text) else {
        return Plan::Full;
    };
    let bend_old = old_text.len() - (new_text.len() - bend_new);
    if risky_edit(&old_text[bstart..bend_old]) || risky_edit(&new_text[bstart..bend_new]) {
        return Plan::Full;
    }

    let total_new = count_lines(new_text) + 1;
    let dirty_start = count_lines(&new_text[..bstart]);
    let dirty_end_new = count_lines(&new_text[..bend_new]);
    let dirty_end_old = count_lines(&old_text[..bend_old]);
    let changed = dirty_end_new - dirty_start + 1;

    if changed > tuning.chunk_lines || changed * 4 > total_new {
        return Plan::Full;
    }

    let window_start = dirty_start.saturating_sub(tuning.warmup_lines);
    let window_end = (dirty_end_new + tuning.margin_lines).min(total_new.saturating_sub(1));
    if window_start == 0 && window_end >= total_new - 1 {
        // Window spans the whole document; no point slicing.
        return Plan::Full;
    }

    Plan::Window(SplicePlan {
        window_start,
        window_end,
        dirty_start,
        dirty_end_new,
        dirty_end_old,
    })
}

/// Extract lines `[start_line, end_line]` (inclusive, 0-indexed) of `text`.
pub fn line_slice(text: &str, start_line: usize, end_line: usize) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let end = end_line.min(lines.len().saturating_sub(1));
    let start = start_line.min(end);
    lines[start..=end].join("\n")
}

/// Parse `text` with the configured grammar, falling back to the plain
/// grammar when the primary one fails. Returns None when both fail.
fn highlight_with_fallback<'a>(
    text: &'a str,
    lang: &str,
    theme: &str,
    registry: &'a Registry,
) -> Option<giallo::HighlightedCode<'a>> {
    let options = HighlightOptions::new(lang, ThemeVariant::Single(theme));
    match registry.highlight(text, &options) {
        Ok(h) => {
            log::debug!("highlight: success for {} tokens", h.tokens.len());
            Some(h)
        }
        Err(err) => {
            log::warn!("highlight: failed for lang={lang} theme={theme}: {err}");
            log::warn!("highlight: trying plain grammar fallback");
            let fallback = HighlightOptions::new(PLAIN_GRAMMAR_NAME, ThemeVariant::Single(theme));
            match registry.highlight(text, &fallback) {
                Ok(h) => {
                    log::debug!("highlight: fallback success for {} tokens", h.tokens.len());
                    Some(h)
                }
                Err(err) => {
                    log::error!("highlight: fallback also failed: {}", err);
                    eprintln!("highlight error: {err}");
                    None
                }
            }
        }
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

    // Decide how much of the document needs re-parsing.
    let plan = match cache {
        Some(cache) => {
            let c = cache.lock().unwrap();
            if c.matches(&resolved_lang, &resolved_theme)
                && !c.text.is_empty()
                && !c.line_tokens.is_empty()
            {
                escalate_to_full(
                    parse_plan(&config.tuning, &c.text.clone(), text),
                    c.updates_since_full,
                    config.tuning.full_refresh_interval,
                )
            } else {
                Plan::Full
            }
        }
        None => Plan::Full,
    };

    if matches!(plan, Plan::NoChange) {
        log::debug!("highlight: text unchanged, skipping parse and send");
        return;
    }

    let mut plan = plan;
    let highlighted = match plan {
        Plan::NoChange => unreachable!(),
        Plan::Full => match highlight_with_fallback(text, &resolved_lang, resolved_theme, registry)
        {
            Some(h) => h,
            None => return,
        },
        Plan::Window(ref splice) => {
            log::debug!(
                "highlight: windowed re-parse lines {}..{} (dirty {}..{})",
                splice.window_start,
                splice.window_end,
                splice.dirty_start,
                splice.dirty_end_new
            );
            let slice = line_slice(text, splice.window_start, splice.window_end);
            let window_options =
                HighlightOptions::new(&resolved_lang, ThemeVariant::Single(resolved_theme));
            match registry.highlight(&slice, &window_options) {
                Ok(h) => h,
                Err(err) => {
                    log::warn!(
                        "highlight: windowed parse failed ({}), falling back to full",
                        err
                    );
                    plan = Plan::Full;
                    match highlight_with_fallback(text, &resolved_lang, resolved_theme, registry) {
                        Some(h) => h,
                        None => return,
                    }
                }
            }
        }
    };

    let (commands, line_count, face_count) = if let Some(cache) = cache {
        let mut cache = cache.lock().unwrap();
        let mut new_faces = Vec::new();

        let old_tokens = cache.line_tokens.clone();
        match &plan {
            Plan::Window(ref splice) => {
                // Tokens for the parse window; drop the warm-up lines.
                let window_tokens =
                    build_line_tokens(&highlighted, &mut cache.allocator, &mut new_faces);
                let skip = splice.dirty_start - splice.window_start;
                let mut replacement: Vec<Vec<RangeToken>> =
                    window_tokens.into_iter().skip(skip).collect();
                let avail = count_lines(text) + 1 - splice.dirty_start;
                replacement.truncate(avail);

                let total_old = cache.line_tokens.len();
                let we_old = (splice
                    .dirty_end_old
                    .saturating_add(config.tuning.margin_lines))
                .min(total_old.saturating_sub(1));
                let mut tail = cache.line_tokens.split_off((we_old + 1).min(total_old));
                cache.line_tokens.truncate(splice.dirty_start);
                cache.line_tokens.extend(replacement);
                cache.line_tokens.append(&mut tail);
                cache.updates_since_full += 1;
            }
            _ => {
                let new_lines =
                    build_line_tokens(&highlighted, &mut cache.allocator, &mut new_faces);
                cache.line_tokens = new_lines;
                cache.updates_since_full = 0;
            }
        }

        for face in &new_faces {
            cache.known_faces.insert(face.name.clone());
        }
        let HighlightCache {
            line_tokens,
            ensured_chunks,
            ..
        } = &mut *cache;
        let commands = build_delta_commands(
            &config.tuning,
            &old_tokens,
            line_tokens,
            &new_faces,
            ensured_chunks,
        );
        cache.text.clear();
        cache.text.push_str(text);
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
