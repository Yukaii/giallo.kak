use giallo::ThemeVariant;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StyleKey {
    pub fg: String,
    pub bg: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
}

#[derive(Clone, Debug)]
pub struct FaceDef {
    pub name: String,
    pub spec: String,
}

pub fn normalize_hex(hex: &str) -> String {
    if hex.len() == 9 {
        hex[..7].to_string()
    } else {
        hex.to_string()
    }
}

pub fn style_key(style: &giallo::Style) -> StyleKey {
    StyleKey {
        fg: normalize_hex(&style.foreground.as_hex()),
        bg: normalize_hex(&style.background.as_hex()),
        bold: style.font_style.contains(giallo::FontStyle::BOLD),
        italic: style.font_style.contains(giallo::FontStyle::ITALIC),
        underline: style.font_style.contains(giallo::FontStyle::UNDERLINE),
        strike: style.font_style.contains(giallo::FontStyle::STRIKETHROUGH),
    }
}

pub fn strip_hash(hex: &str) -> &str {
    if hex.starts_with('#') {
        &hex[1..]
    } else {
        hex
    }
}

pub fn style_to_face_spec(style: &giallo::Style, default_bg: Option<&str>) -> String {
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

pub fn build_kakoune_commands(highlighted: &giallo::HighlightedCode<'_>) -> (Vec<FaceDef>, String) {
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

pub fn build_commands(faces: &[FaceDef], ranges: &str) -> String {
    let mut commands = String::new();
    for face in faces {
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
