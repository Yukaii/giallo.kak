use giallo::ThemeVariant;
use std::collections::HashMap;
use std::fmt::Write;

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

/// Assigns stable face names to styles across successive highlights of the
/// same buffer, so unchanged styles keep their name (and number) between
/// updates. This makes per-line range diffs meaningful.
pub struct FaceAllocator {
    map: HashMap<StyleKey, String>,
    counter: usize,
}

impl Default for FaceAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl FaceAllocator {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            counter: 0,
        }
    }

    /// Return the face name for `style`, allocating a new one (pushing its
    /// definition to `new_faces`) when seen for the first time.
    pub fn face_for(
        &mut self,
        style: &giallo::Style,
        default_bg: &str,
        new_faces: &mut Vec<FaceDef>,
    ) -> String {
        let key = style_key(style);
        if let Some(name) = self.map.get(&key) {
            return name.clone();
        }
        self.counter += 1;
        let name = format!("giallo_{:04}", self.counter);
        let spec = style_to_face_spec(style, Some(default_bg));
        new_faces.push(FaceDef {
            name: name.clone(),
            spec,
        });
        self.map.insert(key, name.clone());
        name
    }
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

/// A single highlighted span within one line, independent of the line's
/// number so cached tokens stay valid when lines shift up/down.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RangeToken {
    pub start: usize,
    pub end: usize,
    pub face: String,
}

/// Build per-line [`RangeToken`] lists, using `allocator` for stable face
/// naming. Newly allocated face definitions are appended to `new_faces`.
pub fn build_line_tokens(
    highlighted: &giallo::HighlightedCode<'_>,
    allocator: &mut FaceAllocator,
    new_faces: &mut Vec<FaceDef>,
) -> Vec<Vec<RangeToken>> {
    let theme = match highlighted.theme {
        ThemeVariant::Single(theme) => theme,
        ThemeVariant::Dual { light, .. } => light,
    };

    let default_style = theme.default_style;
    let default_bg = default_style.background.as_hex();

    let mut lines: Vec<Vec<RangeToken>> = Vec::with_capacity(highlighted.tokens.len());

    for line_tokens in highlighted.tokens.iter() {
        let mut col = 0usize;
        let mut out: Vec<RangeToken> = Vec::new();

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

            let face_name: &str = if style == default_style {
                "default"
            } else {
                &allocator.face_for(&style, &default_bg, new_faces)
            };

            out.push(RangeToken {
                start,
                end: end_excl,
                face: face_name.to_string(),
            });
        }

        lines.push(out);
    }

    lines
}

/// Render one line's tokens into `out` as
/// `{line}.{col_start},{line}.{col_end}|{face}` entries separated by spaces.
pub fn render_line(line_no: usize, tokens: &[RangeToken], out: &mut String) {
    for tok in tokens {
        if !out.is_empty() {
            out.push(' ');
        }
        let col_start = tok.start + 1;
        let col_end = tok.end.max(1);
        let _ = write!(out, "{line_no}.{col_start},{line_no}.{col_end}|{}", tok.face);
    }
}

/// Build one range string per line from cached tokens.
pub fn render_all_lines(lines: &[Vec<RangeToken>]) -> Vec<String> {
    lines
        .iter()
        .enumerate()
        .map(|(idx, toks)| {
            let mut s = String::new();
            render_line(idx + 1, toks, &mut s);
            s
        })
        .collect()
}

/// Single-shot helper (oneshot mode): fresh face allocation, all faces
/// returned, ranges joined into one string. Output is byte-identical to a
/// first-time incremental build.
pub fn build_kakoune_commands(highlighted: &giallo::HighlightedCode<'_>) -> (Vec<FaceDef>, String) {
    let mut allocator = FaceAllocator::new();
    let mut new_faces = Vec::new();
    let line_tokens = build_line_tokens(highlighted, &mut allocator, &mut new_faces);

    let mut joined = String::new();
    for (idx, toks) in line_tokens.iter().enumerate() {
        if toks.is_empty() {
            continue;
        }
        if !joined.is_empty() {
            joined.push(' ');
        }
        render_line(idx + 1, toks, &mut joined);
    }

    (new_faces, joined)
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
