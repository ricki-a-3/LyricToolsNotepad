//! Editor-independent helpers for the UI layer: the syllable palette,
//! whole-word selection rules, replacement casing, and file decoding.
//!
//! All positions here are CHARACTER offsets (what egui's cursors use), never
//! byte offsets.

use std::sync::LazyLock;

use regex::Regex;

// --- syllable palette ---------------------------------------------------------

/// Stepping the hue wheel by the golden angle (instead of evenly slicing it in
/// count order) puts NEIGHBOURING counts -- the comparison that matters when
/// eyeballing a verse -- on near-opposite hues, so a 6 next to a 7 reads as a
/// clearly different color, not a subtle shade shift.
pub const GOLDEN_ANGLE: f64 = 0.6180339887498949;
pub const PALETTE_SIZE: usize = 12;

fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (u8, u8, u8) {
    // Same algorithm as Python's colorsys.hsv_to_rgb, truncated like int().
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    let (r, g, b) = match (i as i64).rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

/// 12 colors: hue starts 0.02, steps by the golden angle, S=0.85, V=0.95.
pub static SYLLABLE_COLORS: LazyLock<[(u8, u8, u8); PALETTE_SIZE]> = LazyLock::new(|| {
    let mut colors = [(0, 0, 0); PALETTE_SIZE];
    let mut hue = 0.02;
    for c in colors.iter_mut() {
        *c = hsv_to_rgb(hue, 0.85, 0.95);
        hue = (hue + GOLDEN_ANGLE) % 1.0;
    }
    colors
});

/// Palette color for a syllable count, or None for 0 (no number shown).
pub fn color_for_count(count: u32) -> Option<(u8, u8, u8)> {
    if count == 0 { None } else { Some(SYLLABLE_COLORS[(count as usize - 1) % PALETTE_SIZE]) }
}

// --- whole-word selection -------------------------------------------------------

/// core's word shape plus an optional trailing apostrophe, so a fully
/// selected g-dropped word like "droppin'" still counts as one word.
static SELECTED_WORD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-z]+(?:['\u{2019}][A-Za-z]+)*(?:-[A-Za-z]+(?:['\u{2019}][A-Za-z]+)*)*['\u{2019}]?$").unwrap()
});
static WORD_SPAN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z]+(?:['\u{2019}][A-Za-z]+)*(?:-[A-Za-z]+(?:['\u{2019}][A-Za-z]+)*)*['\u{2019}]?").unwrap()
});

/// True when chars `start..end` of `line` are exactly one whole word: not a
/// partial word, not several words, not punctuation.
pub fn is_whole_word_selection(line: &str, start: usize, end: usize) -> bool {
    let chars: Vec<char> = line.chars().collect();
    if end <= start || end > chars.len() {
        return false;
    }
    let selected: String = chars[start..end].iter().collect();
    if !SELECTED_WORD_RE.is_match(&selected) {
        return false;
    }
    let letter_at = |i: Option<usize>| i.and_then(|i| chars.get(i)).is_some_and(|c| c.is_ascii_alphabetic());
    !letter_at(start.checked_sub(1)) && !letter_at(Some(end))
}

/// Char span of the word touching char offset `pos` in a line, if any.
pub fn word_span_at(line: &str, pos: usize) -> Option<(usize, usize)> {
    WORD_SPAN_RE.find_iter(line).find_map(|m| {
        let start = line[..m.start()].chars().count();
        let end = start + m.as_str().chars().count();
        (start <= pos && pos <= end).then_some((start, end))
    })
}

// --- text positions ---------------------------------------------------------------

/// Byte offset of char offset `char_idx` (clamped to the end).
pub fn byte_index(text: &str, char_idx: usize) -> usize {
    text.char_indices().nth(char_idx).map_or(text.len(), |(b, _)| b)
}

/// Chars `start..end` of `text`.
pub fn char_slice(text: &str, start: usize, end: usize) -> &str {
    &text[byte_index(text, start)..byte_index(text, end)]
}

/// (line number, char offset where that line starts, line text) for a char
/// offset into the whole document.
pub fn line_at(text: &str, char_idx: usize) -> (usize, usize, &str) {
    let mut line_start_char = 0;
    let mut chars_seen = 0;
    for (n, line) in text.split('\n').enumerate() {
        let len = line.chars().count();
        if char_idx <= chars_seen + len {
            return (n, line_start_char, line);
        }
        chars_seen += len + 1;
        line_start_char = chars_seen;
    }
    (0, 0, "")
}

// --- replacement casing ----------------------------------------------------------

/// Carry the original word's capitalization over to its replacement:
/// `Night` -> `Light`, `NIGHT` -> `LIGHT`, `night` -> `light`. Anything else
/// keeps the replacement as given (so "Paris" stays "Paris").
pub fn match_case(original: &str, replacement: &str) -> String {
    let letters: Vec<char> = original.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.is_empty() {
        return replacement.to_string();
    }
    if letters.len() > 1 && letters.iter().all(|c| c.is_uppercase()) {
        return replacement.to_uppercase();
    }
    if letters[0].is_uppercase() && letters[1..].iter().all(|c| c.is_lowercase()) {
        let mut chars = replacement.chars();
        return match chars.next() {
            Some(first) => first.to_uppercase().chain(chars).collect(),
            None => String::new(),
        };
    }
    replacement.to_string()
}

// --- files -------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Newline {
    Lf,
    CrLf,
    Cr,
}

impl Newline {
    pub fn as_str(self) -> &'static str {
        match self {
            Newline::Lf => "\n",
            Newline::CrLf => "\r\n",
            Newline::Cr => "\r",
        }
    }

    pub fn platform_default() -> Self {
        if cfg!(windows) { Newline::CrLf } else { Newline::Lf }
    }
}

/// How a file was stored, so saving writes it back the same way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileFormat {
    pub newline: Newline,
    pub bom: bool,
}

impl Default for FileFormat {
    fn default() -> Self {
        FileFormat { newline: Newline::platform_default(), bom: false }
    }
}

/// Decode file bytes into text with `\n` newlines. Lyrics come from all over
/// (Notepad, phones, old Notepad++ installs), so accept UTF-8 with or without
/// a BOM and fall back to Windows-1252.
pub fn decode_text(data: &[u8]) -> (String, FileFormat) {
    let (bom, body) = match data.strip_prefix(b"\xEF\xBB\xBF") {
        Some(rest) => (true, rest),
        None => (false, data),
    };
    let text = match std::str::from_utf8(body) {
        Ok(s) => s.to_string(),
        Err(_) => body.iter().map(|&b| cp1252_char(b)).collect(),
    };
    let newline = if text.contains("\r\n") {
        Newline::CrLf
    } else if text.contains('\r') {
        Newline::Cr
    } else if text.contains('\n') {
        Newline::Lf
    } else {
        Newline::platform_default()
    };
    (text.replace("\r\n", "\n").replace('\r', "\n"), FileFormat { newline, bom })
}

/// Encode for saving. Always UTF-8 (legacy files are upgraded so nothing
/// typed later can fail to encode); a BOM only if the file had one.
pub fn encode_text(text: &str, format: FileFormat) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 16);
    if format.bom {
        out.extend_from_slice(b"\xEF\xBB\xBF");
    }
    out.extend_from_slice(text.replace('\n', format.newline.as_str()).as_bytes());
    out
}

fn cp1252_char(b: u8) -> char {
    const HIGH: [char; 32] = [
        '\u{20AC}', '\u{81}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}', '\u{02C6}',
        '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{8D}', '\u{017D}', '\u{8F}', '\u{90}', '\u{2018}',
        '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}', '\u{02DC}', '\u{2122}', '\u{0161}',
        '\u{203A}', '\u{0153}', '\u{9D}', '\u{017E}', '\u{0178}',
    ];
    match b {
        0x80..=0x9F => HIGH[(b - 0x80) as usize],
        _ => b as char,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_matches_python_colorsys() {
        // Python: [tuple(int(c*255) for c in colorsys.hsv_to_rgb(h, .85, .95))]
        let python = [
            (242, 61, 36),
            (36, 71, 242),
            (131, 242, 36),
            (242, 36, 191),
            (36, 242, 232),
            (242, 172, 36),
            (112, 36, 242),
            (36, 242, 52),
            (242, 36, 80),
            (36, 140, 242),
            (200, 242, 36),
            (223, 36, 242),
        ];
        assert_eq!(*SYLLABLE_COLORS, python);
        assert_eq!(color_for_count(13), color_for_count(1));
        assert_eq!(color_for_count(0), None);
    }

    #[test]
    fn whole_word_rules() {
        let line = "I been droppin' bars";
        assert!(is_whole_word_selection(line, 7, 15)); // with trailing '
        assert!(is_whole_word_selection(line, 7, 14)); // without
        assert!(!is_whole_word_selection(line, 7, 12)); // partial
        assert!(!is_whole_word_selection(line, 2, 14)); // two words
        assert!(!is_whole_word_selection("night, light", 0, 6)); // comma
        assert!(is_whole_word_selection("it's bitch'll", 5, 13));
        assert!(is_whole_word_selection("café night", 5, 10)); // char, not byte, offsets
    }

    #[test]
    fn spans_and_lines() {
        assert_eq!(word_span_at("hey there", 5), Some((4, 9)));
        assert_eq!(word_span_at("hey there", 3), Some((0, 3)));
        assert_eq!(word_span_at("   ", 1), None);
        let doc = "[Hook]\ncafé night\nend";
        assert_eq!(line_at(doc, 0), (0, 0, "[Hook]"));
        assert_eq!(line_at(doc, 8), (1, 7, "café night"));
        assert_eq!(line_at(doc, 18), (2, 18, "end"));
        assert_eq!(char_slice(doc, 12, 17), "night");
    }

    #[test]
    fn casing() {
        assert_eq!(match_case("night", "light"), "light");
        assert_eq!(match_case("Night", "light"), "Light");
        assert_eq!(match_case("NIGHT", "light"), "LIGHT");
        assert_eq!(match_case("I", "eye"), "Eye");
        assert_eq!(match_case("night", "Paris"), "Paris");
    }

    #[test]
    fn file_round_trips() {
        let raw = b"\xEF\xBB\xBF[Verse]\r\nline one\r\n";
        let (text, fmt) = decode_text(raw);
        assert_eq!(text, "[Verse]\nline one\n");
        assert_eq!(fmt, FileFormat { newline: Newline::CrLf, bom: true });
        assert_eq!(encode_text(&text, fmt), raw);

        let (text, fmt) = decode_text(b"line\n");
        assert_eq!(encode_text(&text, fmt), b"line\n");

        let (text, _) = decode_text(b"caf\xe9 \x93quote\x94");
        assert_eq!(text, "café \u{201C}quote\u{201D}");
    }
}
