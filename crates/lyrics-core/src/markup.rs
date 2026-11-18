//! Lightweight lyric markup, stored as plain text so files stay readable in
//! any editor (Notepad, phones, Genius-style lyric sites):
//!
//! - `# Song Title`, `## Subheading` -- headings
//! - `[Verse 1]`, `[Chorus]`, `Chorus:` -- section headers; each starts a
//!   block that runs to the next header and can be folded
//! - `**bold**`, `*italic*`, `__underline__` -- inline styles
//!
//! Everything here is pure text logic. Positions are CHARACTER offsets
//! unless a name says `byte`.

use std::ops::Range;

use crate::core;
use crate::textutil::byte_index;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    /// `#` (1), `##` (2), `###` (3) heading.
    Heading(u8),
    /// `[Verse 1]` / `Chorus:` style section header.
    Section,
    Plain,
}

pub fn heading_level(line: &str) -> Option<u8> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    let rest = &line[hashes..];
    ((1..=3).contains(&hashes) && rest.starts_with(' ') && !rest.trim().is_empty()).then_some(hashes as u8)
}

pub fn line_kind(line: &str) -> LineKind {
    if let Some(level) = heading_level(line) {
        LineKind::Heading(level)
    } else if core::is_section_header(line) {
        LineKind::Section
    } else {
        LineKind::Plain
    }
}

/// The display title of a header line: `[Verse 1]` -> `Verse 1`,
/// `Chorus:` -> `Chorus`, `## Intro` -> `Intro`.
pub fn header_title(line: &str) -> String {
    let t = line.trim();
    let t = t.trim_start_matches('#').trim();
    let t = t.strip_prefix('[').and_then(|s| s.strip_suffix(']')).unwrap_or(t);
    t.strip_suffix(':').unwrap_or(t).trim().to_string()
}

// --- inline styles -----------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    /// The `**` / `*` / `__` characters themselves (drawn dimmed).
    pub marker: bool,
}

/// Split a line into byte ranges of uniform style, covering the whole line.
pub fn inline_spans(line: &str) -> Vec<(Range<usize>, Style)> {
    let bytes = line.as_bytes();
    let mut styles = vec![Style::default(); bytes.len()];

    // Pair up non-overlapping occurrences of a 2-char marker left to right.
    let mut pair_double = |marker: &[u8; 2], apply: fn(&mut Style)| {
        let mut opens = Vec::new();
        let mut i = 0;
        while i + 1 < bytes.len() {
            if &bytes[i..i + 2] == marker && !styles[i].marker && !styles[i + 1].marker {
                opens.push(i);
                i += 2;
            } else {
                i += 1;
            }
        }
        for pair in opens.chunks_exact(2) {
            let (open, close) = (pair[0], pair[1]);
            if close == open + 2 {
                continue; // "****" -- nothing inside
            }
            for s in &mut styles[open..open + 2] {
                s.marker = true;
            }
            for s in &mut styles[close..close + 2] {
                s.marker = true;
            }
            for s in &mut styles[open + 2..close] {
                apply(s);
            }
        }
    };
    pair_double(b"**", |s| s.bold = true);
    pair_double(b"__", |s| s.underline = true);

    // Remaining single `*`s pair up as italics.
    let singles: Vec<usize> = (0..bytes.len()).filter(|&i| bytes[i] == b'*' && !styles[i].marker).collect();
    for pair in singles.chunks_exact(2) {
        let (open, close) = (pair[0], pair[1]);
        if close == open + 1 {
            continue;
        }
        styles[open].marker = true;
        styles[close].marker = true;
        for s in &mut styles[open + 1..close] {
            s.italic = true;
        }
    }

    // Collapse into runs, never splitting a multi-byte character.
    let mut runs: Vec<(Range<usize>, Style)> = Vec::new();
    for (i, _) in line.char_indices() {
        let style = styles[i];
        match runs.last_mut() {
            Some((range, s)) if *s == style => range.end = next_char_boundary(line, i),
            _ => runs.push((i..next_char_boundary(line, i), style)),
        }
    }
    runs
}

fn next_char_boundary(s: &str, i: usize) -> usize {
    i + s[i..].chars().next().map_or(0, char::len_utf8)
}

// --- sections / folding ------------------------------------------------------------

/// A foldable block: a header line plus everything up to the next header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub header_line: usize,
    /// Exclusive.
    pub end_line: usize,
    pub title: String,
    /// Stable identity across edits: (title, nth block with that title).
    pub key: (String, usize),
}

impl Block {
    pub fn body_lines(&self) -> Range<usize> {
        self.header_line + 1..self.end_line
    }
}

pub fn blocks(lines: &[&str]) -> Vec<Block> {
    let headers: Vec<usize> =
        lines.iter().enumerate().filter(|(_, l)| line_kind(l) != LineKind::Plain).map(|(i, _)| i).collect();
    let mut seen: std::collections::HashMap<String, usize> = Default::default();
    headers
        .iter()
        .enumerate()
        .map(|(n, &start)| {
            let mut end = headers.get(n + 1).copied().unwrap_or(lines.len());
            // Trailing blank lines stay visible as spacing between blocks.
            while end > start + 1 && lines[end - 1].trim().is_empty() {
                end -= 1;
            }
            let title = header_title(lines[start]);
            let nth = seen.entry(title.clone()).or_default();
            let key = (title.clone(), *nth);
            *nth += 1;
            Block { header_line: start, end_line: end, title, key }
        })
        .collect()
}

// --- editing commands ----------------------------------------------------------------

/// Result of an edit: the new text and the new selection (chars).
#[derive(Debug, PartialEq, Eq)]
pub struct Edit {
    pub text: String,
    pub selection: (usize, usize),
}

fn replace_chars(text: &str, start: usize, end: usize, with: &str) -> String {
    let (b0, b1) = (byte_index(text, start), byte_index(text, end));
    format!("{}{}{}", &text[..b0], with, &text[b1..])
}

fn chars_of(text: &str, start: usize, end: usize) -> String {
    text.chars().skip(start).take(end.saturating_sub(start)).collect()
}

/// Bold / italic / underline toggle around the selection (or insert an
/// empty pair at the caret).
pub fn toggle_wrap(text: &str, sel: (usize, usize), marker: &str) -> Edit {
    let (start, end) = (sel.0.min(sel.1), sel.0.max(sel.1));
    let m = marker.chars().count();
    let total = text.chars().count();
    // Already wrapped just outside the selection -> unwrap.
    if start >= m
        && end + m <= total
        && chars_of(text, start - m, start) == marker
        && chars_of(text, end, end + m) == marker
    {
        let without_close = replace_chars(text, end, end + m, "");
        return Edit { text: replace_chars(&without_close, start - m, start, ""), selection: (start - m, end - m) };
    }
    let inner = chars_of(text, start, end);
    // Selection includes the markers -> unwrap.
    if end - start >= 2 * m && inner.starts_with(marker) && inner.ends_with(marker) {
        let stripped: String = inner.chars().skip(m).take(end - start - 2 * m).collect();
        return Edit { text: replace_chars(text, start, end, &stripped), selection: (start, end - 2 * m) };
    }
    let wrapped = format!("{marker}{inner}{marker}");
    Edit { text: replace_chars(text, start, end, &wrapped), selection: (start + m, end + m) }
}

/// (char offset of line start, char offset of line end) for the line holding `pos`.
fn line_bounds(text: &str, pos: usize) -> (usize, usize) {
    let (_, start, line) = crate::textutil::line_at(text, pos);
    (start, start + line.chars().count())
}

/// Cycle the caret's line: plain -> `# ` -> `## ` -> plain.
pub fn cycle_heading(text: &str, caret: usize) -> Edit {
    let (ls, le) = line_bounds(text, caret);
    let line = chars_of(text, ls, le);
    let body = match heading_level(&line) {
        Some(level) => line[level as usize + 1..].to_string(),
        None => line.clone(),
    };
    let new_line = match heading_level(&line) {
        None => format!("# {body}"),
        Some(1) => format!("## {body}"),
        Some(_) => body,
    };
    let delta = new_line.chars().count() as isize - line.chars().count() as isize;
    let caret = ((caret as isize + delta).max(ls as isize)) as usize;
    Edit { text: replace_chars(text, ls, le, &new_line), selection: (caret, caret) }
}

/// Add a `[Title]` section header. On an empty line the header goes right
/// there; otherwise it goes below the current line, after a blank line.
/// The caret ends up on the line under the header, ready to write.
pub fn insert_section(text: &str, caret: usize, title: &str) -> Edit {
    let (ls, le) = line_bounds(text, caret);
    let header = format!("[{}]", title.trim());
    let header_len = header.chars().count();
    if chars_of(text, ls, le).trim().is_empty() {
        let new = replace_chars(text, ls, le, &format!("{header}\n"));
        let pos = ls + header_len + 1;
        return Edit { text: new, selection: (pos, pos) };
    }
    let insert = format!("\n\n{header}\n");
    let new = replace_chars(text, le, le, &insert);
    let pos = le + insert.chars().count();
    Edit { text: new, selection: (pos, pos) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn styled(line: &str) -> Vec<(&str, Style)> {
        inline_spans(line).into_iter().map(|(r, s)| (&line[r], s)).collect()
    }

    const B: Style = Style { bold: true, italic: false, underline: false, marker: false };
    const I: Style = Style { bold: false, italic: true, underline: false, marker: false };
    const U: Style = Style { bold: false, italic: false, underline: true, marker: false };
    const M: Style = Style { bold: false, italic: false, underline: false, marker: true };
    const P: Style = Style { bold: false, italic: false, underline: false, marker: false };

    #[test]
    fn inline_styles() {
        assert_eq!(styled("a **big** deal"), vec![("a ", P), ("**", M), ("big", B), ("**", M), (" deal", P)]);
        assert_eq!(
            styled("*soft* __low__"),
            vec![("*", M), ("soft", I), ("*", M), (" ", P), ("__", M), ("low", U), ("__", M)]
        );
        assert_eq!(styled("2 * 3 = 6"), vec![("2 * 3 = 6", P)]); // a lone * is just a star
        assert_eq!(styled("unclosed **bold"), vec![("unclosed **bold", P)]);
        assert_eq!(styled("café **é**"), vec![("café ", P), ("**", M), ("é", B), ("**", M)]);
    }

    #[test]
    fn kinds_and_titles() {
        assert_eq!(line_kind("# My Song"), LineKind::Heading(1));
        assert_eq!(line_kind("## Intro"), LineKind::Heading(2));
        assert_eq!(line_kind("#hashtag"), LineKind::Plain);
        assert_eq!(line_kind("[Verse 1]"), LineKind::Section);
        assert_eq!(line_kind("Chorus:"), LineKind::Section);
        assert_eq!(line_kind("I been droppin' bars"), LineKind::Plain);
        assert_eq!(header_title("[Verse 1]"), "Verse 1");
        assert_eq!(header_title("Chorus:"), "Chorus");
        assert_eq!(header_title("## Bridge"), "Bridge");
    }

    #[test]
    fn block_ranges() {
        let doc = ["# Song", "", "[Verse 1]", "a", "b", "", "[Chorus]", "c", "", "[Verse 1]", "d"];
        let b = blocks(&doc);
        assert_eq!(b.len(), 4);
        assert_eq!((b[0].header_line, b[0].end_line), (0, 1)); // trailing blank stays visible
        assert_eq!((b[1].header_line, b[1].end_line), (2, 5));
        assert_eq!((b[2].header_line, b[2].end_line), (6, 8));
        assert_eq!(b[1].key, ("Verse 1".to_string(), 0));
        assert_eq!(b[3].key, ("Verse 1".to_string(), 1));
    }

    #[test]
    fn wrap_toggles() {
        let e = toggle_wrap("make it big", (8, 11), "**");
        assert_eq!(e, Edit { text: "make it **big**".into(), selection: (10, 13) });
        // toggling again with the same (inner) selection unwraps
        let e2 = toggle_wrap(&e.text, e.selection, "**");
        assert_eq!(e2, Edit { text: "make it big".into(), selection: (8, 11) });
        // selection including the markers unwraps too
        let e3 = toggle_wrap("x __u__ y", (2, 7), "__");
        assert_eq!(e3.text, "x u y");
        // empty selection inserts a pair with the caret inside
        let e4 = toggle_wrap("ab", (1, 1), "*");
        assert_eq!(e4, Edit { text: "a**b".into(), selection: (2, 2) });
    }

    #[test]
    fn heading_and_section_commands() {
        let e = cycle_heading("intro\nline", 2);
        assert_eq!(e.text, "# intro\nline");
        let e = cycle_heading(&e.text, 4);
        assert_eq!(e.text, "## intro\nline");
        let e = cycle_heading(&e.text, 4);
        assert_eq!(e.text, "intro\nline");

        let e = insert_section("a\nb", 2, "Chorus");
        assert_eq!(e, Edit { text: "a\nb\n\n[Chorus]\n".into(), selection: (14, 14) });
        let e = insert_section("a\n\nb", 2, " Verse 2 ");
        assert_eq!(e.text, "a\n[Verse 2]\n\nb");
    }
}
