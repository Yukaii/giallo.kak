use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::thread;

use giallo::{HighlightOptions, Registry, ThemeVariant, PLAIN_GRAMMAR_NAME};

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

fn style_to_face_spec(style: &giallo::Style) -> String {
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

    let fg = normalize_hex(&style.foreground.as_hex());
    let bg = normalize_hex(&style.background.as_hex());

    if attrs.is_empty() {
        format!("{fg},{bg}")
    } else {
        format!("{fg},{bg}+{attrs}")
    }
}

fn build_kakoune_commands(
    highlighted: &giallo::HighlightedCode<'_>,
) -> (Vec<FaceDef>, String) {
    let theme = match highlighted.theme {
        ThemeVariant::Single(theme) => theme,
        ThemeVariant::Dual { light, .. } => light,
    };

    let default_style = theme.default_style;

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
                    let spec = style_to_face_spec(&style);
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

            ranges.push(format!(
                "{line}.{col_start},{line}.{col_end}|{face_name}"
            ));
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
        commands.push_str("set-face global ");
        commands.push_str(&face.name);
        commands.push(' ');
        commands.push_str(&face.spec);
        commands.push('\n');
    }

    commands.push_str("set-option buffer giallo_hl_ranges %val{timestamp}");
    if !ranges.is_empty() {
        commands.push(' ');
        commands.push_str(ranges);
    }
    commands.push('\n');

    commands
}

fn write_response(mut out: impl Write, commands: &str) -> io::Result<()> {
    let len = commands.as_bytes().len();
    writeln!(out, "OK {len}")?;
    out.write_all(commands.as_bytes())?;
    out.flush()
}

fn read_exact_bytes(reader: &mut impl Read, len: usize) -> io::Result<Vec<u8>> {
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

enum Mode {
    Stdio,
    Oneshoot,
    Fifo { req: String, resp: Option<String> },
}

fn parse_args() -> Mode {
    let mut oneshot = false;
    let mut fifo_req: Option<String> = None;
    let mut fifo_resp: Option<String> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" => {
                println!("giallo-kak 0.1.0");
                process::exit(0);
            }
            "--oneshot" => oneshot = true,
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

    if let Some(req) = fifo_req {
        return Mode::Fifo { req, resp: fifo_resp };
    }

    if oneshot {
        Mode::Oneshoot
    } else {
        Mode::Stdio
    }
}

fn token_hash(token: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    token.hash(&mut hasher);
    format!("{:x}", hasher.finish())
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

fn handle_init(
    token: &str,
    base_dir: &Path,
) -> io::Result<(PathBuf, PathBuf, String)> {
    fs::create_dir_all(base_dir)?;
    let hash = token_hash(token);
    let req = base_dir.join(format!("{hash}.req.fifo"));
    let resp = base_dir.join(format!("{hash}.resp.fifo"));
    let sentinel = format!("giallo-{hash}");

    if !req.exists() {
        create_fifo(&req)?;
    }
    if !resp.exists() {
        create_fifo(&resp)?;
    }

    Ok((req, resp, sentinel))
}

fn run_server<R: BufRead, W: Write>(
    mut reader: R,
    mut writer: W,
    registry: &mut Registry,
    oneshot: bool,
    base_dir: Option<&Path>,
) -> io::Result<()> {
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            break;
        }

        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }

        if line == "PING" {
            writeln!(writer, "PONG").ok();
            writer.flush().ok();
            continue;
        }

        let mut parts = line.split_whitespace();
        let cmd = parts.next().unwrap_or("");

        if cmd == "INIT" {
            let token = match parts.next() {
                Some(v) => v.to_string(),
                None => {
                    eprintln!("missing token");
                    continue;
                }
            };
            let Some(base_dir) = base_dir else {
                eprintln!("init not supported in this mode");
                continue;
            };

            let (req, resp, sentinel) = match handle_init(&token, base_dir) {
                Ok(v) => v,
                Err(err) => {
                    eprintln!("init error: {err}");
                    continue;
                }
            };

            let commands = format!(
                "set-option buffer giallo_buf_fifo_path {req}\nset-option buffer giallo_buf_resp_path {resp}\nset-option buffer giallo_buf_sentinel {sentinel}\n",
                req = req.display(),
                resp = resp.display(),
                sentinel = sentinel
            );

            let req_path = req.clone();
            let resp_path = resp.clone();
            let token_clone = token.clone();
            thread::spawn(move || {
                let mut registry = match Registry::builtin() {
                    Ok(registry) => registry,
                    Err(err) => {
                        eprintln!("init thread registry error ({token_clone}): {err}");
                        return;
                    }
                };
                registry.link_grammars();

                let req_file = match OpenOptions::new().read(true).open(&req_path) {
                    Ok(file) => file,
                    Err(err) => {
                        eprintln!("init thread open req error ({token_clone}): {err}");
                        return;
                    }
                };
                let resp_file = match OpenOptions::new().write(true).open(&resp_path) {
                    Ok(file) => file,
                    Err(err) => {
                        eprintln!("init thread open resp error ({token_clone}): {err}");
                        return;
                    }
                };

                let mut req_reader = io::BufReader::new(req_file);
                let mut resp_writer = resp_file;
                let _ = run_server(
                    &mut req_reader,
                    &mut resp_writer,
                    &mut registry,
                    false,
                    None,
                );
            });

            if oneshot {
                writer.write_all(commands.as_bytes())?;
                writer.flush()?;
                break;
            } else {
                write_response(&mut writer, &commands)?;
            }
            continue;
        }

        if cmd != "H" {
            eprintln!("unknown command: {cmd}");
            continue;
        }

        let lang = match parts.next() {
            Some(v) => v.to_string(),
            None => {
                eprintln!("missing language");
                continue;
            }
        };
        let theme = match parts.next() {
            Some(v) => v.to_string(),
            None => {
                eprintln!("missing theme");
                continue;
            }
        };
        let len = match parts.next() {
            Some(v) => match v.parse::<usize>() {
                Ok(n) => n,
                Err(_) => {
                    eprintln!("invalid length");
                    continue;
                }
            },
            None => {
                eprintln!("missing length");
                continue;
            }
        };

        let buf = match read_exact_bytes(&mut reader, len) {
            Ok(b) => b,
            Err(err) => {
                eprintln!("failed to read payload: {err}");
                continue;
            }
        };
        let text = match String::from_utf8(buf) {
            Ok(s) => s,
            Err(err) => {
                eprintln!("payload is not utf-8: {err}");
                continue;
            }
        };

        let options = HighlightOptions::new(&lang, ThemeVariant::Single(theme.as_str()));
        let highlighted = match registry.highlight(&text, &options) {
            Ok(h) => h,
            Err(_) => {
                let fallback =
                    HighlightOptions::new(PLAIN_GRAMMAR_NAME, ThemeVariant::Single(theme.as_str()));
                match registry.highlight(&text, &fallback) {
                    Ok(h) => h,
                    Err(err) => {
                        eprintln!("highlight error: {err}");
                        continue;
                    }
                }
            }
        };

        let (faces, ranges) = build_kakoune_commands(&highlighted);
        let commands = build_commands(&faces, &ranges);

        if oneshot {
            writer.write_all(commands.as_bytes())?;
            writer.flush()?;
            break;
        } else {
            write_response(&mut writer, &commands)?;
        }
    }

    Ok(())
}

fn main() {
    let mode = parse_args();
    let base_dir = std::env::temp_dir().join(format!("giallo-kak-{}", process::id()));

    let mut registry = match Registry::builtin() {
        Ok(registry) => registry,
        Err(err) => {
            eprintln!("failed to load giallo registry: {err}");
            process::exit(1);
        }
    };
    registry.link_grammars();

    match mode {
        Mode::Stdio => {
            let stdin = io::stdin();
            let stdout = io::stdout();
            let mut stdin_lock = stdin.lock();
            let mut stdout_lock = stdout.lock();
            if let Err(err) = run_server(
                &mut stdin_lock,
                &mut stdout_lock,
                &mut registry,
                false,
                Some(&base_dir),
            ) {
                eprintln!("server error: {err}");
            }
        }
        Mode::Oneshoot => {
            let stdin = io::stdin();
            let stdout = io::stdout();
            let mut stdin_lock = stdin.lock();
            let mut stdout_lock = stdout.lock();
            if let Err(err) = run_server(
                &mut stdin_lock,
                &mut stdout_lock,
                &mut registry,
                true,
                Some(&base_dir),
            ) {
                eprintln!("oneshot error: {err}");
            }
        }
        Mode::Fifo { req, resp } => {
            let req_file = match OpenOptions::new().read(true).open(&req) {
                Ok(file) => file,
                Err(err) => {
                    eprintln!("failed to open fifo for read: {req}: {err}");
                    process::exit(1);
                }
            };

            let mut req_reader = io::BufReader::new(req_file);

            let mut resp_writer: Box<dyn Write> = if let Some(resp_path) = resp {
                match OpenOptions::new().write(true).open(&resp_path) {
                    Ok(file) => Box::new(file),
                    Err(err) => {
                        eprintln!("failed to open fifo for write: {resp_path}: {err}");
                        process::exit(1);
                    }
                }
            } else {
                Box::new(io::stdout())
            };

            if let Err(err) =
                run_server(&mut req_reader, &mut resp_writer, &mut registry, false, Some(&base_dir))
            {
                eprintln!("fifo server error: {err}");
            }
        }
    }
}
