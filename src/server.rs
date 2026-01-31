use std::io::{self, BufRead, Write};
use std::path::Path;
use std::sync::Arc;

use giallo::{HighlightOptions, Registry, ThemeVariant, PLAIN_GRAMMAR_NAME};
use log;

use crate::config::Config;
use crate::fifo;
use crate::highlight::send_to_kak;
use crate::highlight::BufferContext;
use crate::server_resources::ServerResources;

pub fn run_server<R: BufRead, W: Write>(
    mut reader: R,
    mut writer: W,
    registry: Arc<Registry>,
    config: &Config,
    oneshot: bool,
    base_dir: Option<&Path>,
    resources: &ServerResources,
) -> io::Result<()> {
    let mut line = String::new();
    let mut buffer_contexts: std::collections::HashMap<String, BufferContext> =
        std::collections::HashMap::new();

    loop {
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

        // Handle oneshot highlight command: H <lang> <theme> <len>
        if oneshot && line.starts_with("H ") {
            let mut parts = line.split_whitespace();
            let _ = parts.next();
            let lang = parts.next().unwrap_or("plain");
            let theme = parts.next().unwrap_or("catppuccin-frappe");
            let len_str = parts.next().unwrap_or("0");
            let len = len_str.parse::<usize>().unwrap_or(0);

            log::debug!(
                "oneshot highlight: lang={} theme={} len={}",
                lang,
                theme,
                len
            );

            let mut payload = vec![0u8; len];
            if len > 0 {
                let mut total_read = 0;
                while total_read < len {
                    match reader.read(&mut payload[total_read..]) {
                        Ok(0) => break,
                        Ok(n) => total_read += n,
                        Err(e) => return Err(e),
                    }
                }
                if total_read < len {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        format!("expected {} bytes, got {}", len, total_read),
                    ));
                }
            }
            let text = String::from_utf8_lossy(&payload);

            let ctx = BufferContext::new(
                "oneshot".to_string(),
                "buffer".to_string(),
                "".to_string(),
                lang.to_string(),
                theme.to_string(),
            );

            let resolved_lang = config.resolve_lang(lang);
            let resolved_theme = config.resolve_theme(theme);

            log::debug!(
                "oneshot highlight: buffer={} lang={} (resolved={}) theme={} (resolved={}) text_len={}",
                ctx.buffer, lang, resolved_lang, theme, resolved_theme, text.len()
            );

            let options =
                HighlightOptions::new(&resolved_lang, ThemeVariant::Single(resolved_theme));
            let highlighted = match registry.highlight(&text, &options) {
                Ok(h) => {
                    log::debug!("oneshot highlight: success for {} tokens", h.tokens.len());
                    h
                }
                Err(err) => {
                    log::warn!(
                        "oneshot highlight: failed for lang={} theme={}: {}",
                        resolved_lang,
                        resolved_theme,
                        err
                    );
                    let fallback = HighlightOptions::new(
                        PLAIN_GRAMMAR_NAME,
                        ThemeVariant::Single(resolved_theme),
                    );
                    match registry.highlight(&text, &fallback) {
                        Ok(h) => {
                            log::debug!(
                                "oneshot highlight: fallback success for {} tokens",
                                h.tokens.len()
                            );
                            h
                        }
                        Err(err) => {
                            log::error!("oneshot highlight: fallback also failed: {}", err);
                            eprintln!("highlight error: {err}");
                            break;
                        }
                    }
                }
            };

            let (faces, ranges) = crate::highlighting::build_kakoune_commands(&highlighted);
            let commands = crate::highlighting::build_commands(&faces, &ranges);

            if let Err(err) = writeln!(writer, "{}", commands) {
                log::error!("oneshot highlight: failed to write to stdout: {}", err);
                eprintln!("failed to write output: {err}");
            } else {
                log::debug!("oneshot highlight: wrote output successfully");
            }
            break;
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

            let (req, sentinel) = match fifo::handle_init(&token, base_dir) {
                Ok(v) => v,
                Err(err) => {
                    log::error!("INIT: error creating FIFO: {}", err);
                    eprintln!("init error: {err}");
                    continue;
                }
            };

            let highlighter = config.resolve_highlighter(&lang);

            let commands = format!(
                "set-option buffer giallo_buf_fifo_path {req}\nset-option buffer giallo_buf_sentinel {sentinel}\nset-option buffer giallo_highlighter {highlighter}\n",
                req = req.display(),
                sentinel = sentinel,
                highlighter = highlighter
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
            let ctx_for_map = ctx.clone();

            log::debug!("INIT: spawning buffer handler thread");
            let thread_quit_flag = resources.quit_flag();
            let thread_registry = Arc::clone(&registry);
            std::thread::spawn(move || {
                log::debug!("buffer thread: starting for {}", token_clone);

                match fifo::run_buffer_fifo(
                    &req_path,
                    &thread_registry,
                    &config_clone,
                    ctx,
                    Some(&thread_quit_flag),
                ) {
                    Ok(_) => log::debug!("buffer thread: completed normally for {}", token_clone),
                    Err(err) => log::error!("buffer thread: error for {}: {}", token_clone, err),
                }

                let _ = std::fs::remove_file(&req_path);
                log::debug!("buffer thread: exiting for {}", token_clone);
            });

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
