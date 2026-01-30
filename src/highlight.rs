use std::io::Write;
use std::process::Command;
use std::process::Stdio;

use giallo::{HighlightOptions, Registry, ThemeVariant, PLAIN_GRAMMAR_NAME};
use log;

use crate::config::Config;
use crate::highlighting::{build_commands, build_kakoune_commands};
use crate::kakoune::kak_quote;

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

pub fn highlight_and_send(
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

    if which::which("kak").is_err() {
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
    }
    Ok(())
}
