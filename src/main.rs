use std::collections::HashMap;
use std::io::{self, BufRead, Read, Write};
use std::process;

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

fn main() {
    let mut oneshot = false;
    for arg in std::env::args().skip(1) {
        if arg == "--version" {
            println!("giallo-kak 0.1.0");
            return;
        }
        if arg == "--oneshot" {
            oneshot = true;
        }
    }

    let mut registry = match Registry::builtin() {
        Ok(registry) => registry,
        Err(err) => {
            eprintln!("failed to load giallo registry: {err}");
            process::exit(1);
        }
    };
    registry.link_grammars();

    let stdin = io::stdin();
    let mut stdin_lock = stdin.lock();
    let mut line = String::new();
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();

    loop {
        line.clear();
        let bytes = match stdin_lock.read_line(&mut line) {
            Ok(0) => break,
            Ok(n) => n,
            Err(err) => {
                eprintln!("read error: {err}");
                break;
            }
        };

        if bytes == 0 {
            break;
        }

        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }

        if line == "PING" {
            writeln!(stdout_lock, "PONG").ok();
            stdout_lock.flush().ok();
            continue;
        }

        let mut parts = line.split_whitespace();
        let cmd = parts.next().unwrap_or("");

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

        let buf = match read_exact_bytes(&mut stdin_lock, len) {
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
            if let Err(err) = stdout_lock.write_all(commands.as_bytes()) {
                eprintln!("failed to write oneshot response: {err}");
                break;
            }
            stdout_lock.flush().ok();
            break;
        } else if let Err(err) = write_response(&mut stdout_lock, &commands) {
            eprintln!("failed to write response: {err}");
            break;
        }
    }
}
