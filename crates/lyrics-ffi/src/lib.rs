//! Swift-facing API for the lyric engine (via UniFFI).
//!
//! iOS text APIs (NSString, NSRange, UITextView selections) count positions
//! in UTF-16 code units, while the engine counts characters. Every position
//! crossing this boundary is UTF-16 on the Swift side and converted here, so
//! "é" or an emoji can't shift a selection.

use lyrics_core::core::{self, RhymeKind};
use lyrics_core::{library, markup, textutil};

uniffi::setup_scaffolding!();

const RESULTS_PER_GROUP: usize = 60;
const DEEP_SEARCH_MAX: usize = 150;

// --- position conversion ------------------------------------------------------

/// UTF-16 offset -> character offset (clamped to the end).
fn utf16_to_char(text: &str, utf16: u32) -> usize {
    let mut units = 0u32;
    for (i, c) in text.chars().enumerate() {
        if units >= utf16 {
            return i;
        }
        units += c.len_utf16() as u32;
    }
    text.chars().count()
}

/// Character offset -> UTF-16 offset.
fn char_to_utf16(text: &str, chars: usize) -> u32 {
    text.chars().take(chars).map(|c| c.len_utf16() as u32).sum()
}

/// Byte offset -> UTF-16 offset.
fn byte_to_utf16(text: &str, byte: usize) -> u32 {
    text[..byte].encode_utf16().count() as u32
}

// --- syllables & colors -----------------------------------------------------------

/// Build the rhyme indexes ahead of time (call once, off the main thread).
#[uniffi::export]
pub fn warm_up() {
    core::warm_up();
}

/// Syllables per line of a whole document (`\n`-separated). Headings,
/// section headers and blank lines count 0.
#[uniffi::export]
pub fn line_syllable_counts(text: String) -> Vec<u32> {
    text.split('\n').map(line_syllables).collect()
}

fn line_syllables(line: &str) -> u32 {
    if line.trim().is_empty() || markup::line_kind(line) != markup::LineKind::Plain {
        0
    } else {
        core::syllables_for_line(line)
    }
}

#[uniffi::export]
pub fn syllables_for_line(line: String) -> u32 {
    line_syllables(&line)
}

#[derive(uniffi::Record, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// The gutter color for a syllable count (none for 0).
#[uniffi::export]
pub fn color_for_count(count: u32) -> Option<Rgb> {
    textutil::color_for_count(count).map(|(r, g, b)| Rgb { r, g, b })
}

// --- rhymes ---------------------------------------------------------------------------

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub word: String,
    pub syllables: u32,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct SuggestionGroup {
    pub heading: String,
    pub words: Vec<Suggestion>,
}

#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum RhymeMode {
    Perfect,
    Near,
    NearAndPerfect,
}

#[derive(uniffi::Error, Debug)]
pub enum LookupError {
    Network { message: String },
}

impl std::fmt::Display for LookupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LookupError::Network { message } => f.write_str(message),
        }
    }
}

impl std::error::Error for LookupError {}

fn suggestions(results: Vec<core::WordResult>) -> Vec<Suggestion> {
    results.into_iter().map(|r| Suggestion { word: r.word, syllables: r.syllables }).collect()
}

/// Offline rhymes for the armed mode. Near + Perfect returns two groups;
/// words already listed under Perfect are not repeated under Near.
#[uniffi::export]
pub fn rhyme_groups(word: String, mode: RhymeMode) -> Vec<SuggestionGroup> {
    let perfect = || core::find_words_local(&word, RhymeKind::Perfect, RESULTS_PER_GROUP);
    let near = || core::find_words_local(&word, RhymeKind::Near, RESULTS_PER_GROUP);
    let groups = match mode {
        RhymeMode::Perfect => vec![("Perfect", perfect())],
        RhymeMode::Near => vec![("Near", near())],
        RhymeMode::NearAndPerfect => vec![("Perfect", perfect()), ("Near", near())],
    };
    let mut seen = std::collections::HashSet::new();
    groups
        .into_iter()
        .map(|(heading, results)| {
            let fresh = results.into_iter().filter(|r| seen.insert(r.word.to_lowercase())).collect();
            SuggestionGroup { heading: heading.to_string(), words: suggestions(fresh) }
        })
        .filter(|g| !g.words.is_empty())
        .collect()
}

/// Synonyms from Datamuse. Blocks on the network: call off the main thread.
#[uniffi::export]
pub fn synonyms(word: String) -> Result<Vec<Suggestion>, LookupError> {
    core::find_synonyms(&word, RESULTS_PER_GROUP).map(suggestions).map_err(|e| LookupError::Network { message: e.0 })
}

/// Biggest rhyme list: offline tiers first, then Datamuse. Never fails (falls
/// back to offline results). Blocks on the network: call off the main thread.
#[uniffi::export]
pub fn deep_search(word: String) -> Vec<Suggestion> {
    suggestions(core::find_words_deep(&word, DEEP_SEARCH_MAX))
}

/// Is `line[start..end]` (UTF-16 offsets) exactly one whole word?
#[uniffi::export]
pub fn is_whole_word(line: String, start: u32, end: u32) -> bool {
    let (s, e) = (utf16_to_char(&line, start), utf16_to_char(&line, end));
    textutil::is_whole_word_selection(&line, s, e)
}

/// `Night` + `light` -> `Light`, `NIGHT` -> `LIGHT`.
#[uniffi::export]
pub fn match_case(original: String, replacement: String) -> String {
    textutil::match_case(&original, &replacement)
}

// --- markup --------------------------------------------------------------------------

#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Heading { level: u8 },
    Section,
    Plain,
}

#[uniffi::export]
pub fn line_kind(line: String) -> LineKind {
    match markup::line_kind(&line) {
        markup::LineKind::Heading(level) => LineKind::Heading { level },
        markup::LineKind::Section => LineKind::Section,
        markup::LineKind::Plain => LineKind::Plain,
    }
}

/// A run of uniformly styled text within one line (UTF-16 offsets).
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct StyledRun {
    pub start: u32,
    pub length: u32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    /// The `**` / `*` / `__` characters themselves (draw dimmed).
    pub marker: bool,
}

#[uniffi::export]
pub fn inline_runs(line: String) -> Vec<StyledRun> {
    markup::inline_spans(&line)
        .into_iter()
        .map(|(range, s)| {
            let start = byte_to_utf16(&line, range.start);
            StyledRun {
                start,
                length: byte_to_utf16(&line, range.end) - start,
                bold: s.bold,
                italic: s.italic,
                underline: s.underline,
                marker: s.marker,
            }
        })
        .collect()
}

/// A foldable section: its header line and the lines up to the next header.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct LyricSection {
    pub header_line: u32,
    /// Exclusive.
    pub end_line: u32,
    pub title: String,
    /// Stable fold identity: `title` + this, the nth section with that title.
    pub occurrence: u32,
}

#[uniffi::export]
pub fn sections(text: String) -> Vec<LyricSection> {
    let lines: Vec<&str> = text.split('\n').collect();
    markup::blocks(&lines)
        .into_iter()
        .map(|b| LyricSection {
            header_line: b.header_line as u32,
            end_line: b.end_line as u32,
            title: b.title,
            occurrence: b.key.1 as u32,
        })
        .collect()
}

/// New text and selection (UTF-16) after an edit command.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct EditResult {
    pub text: String,
    pub selection_start: u32,
    pub selection_end: u32,
}

fn to_result(edit: markup::Edit) -> EditResult {
    let (s, e) = edit.selection;
    let (selection_start, selection_end) = (char_to_utf16(&edit.text, s), char_to_utf16(&edit.text, e));
    EditResult { text: edit.text, selection_start, selection_end }
}

/// Toggle `**`, `*` or `__` around the selection (UTF-16 offsets).
#[uniffi::export]
pub fn toggle_wrap(text: String, selection_start: u32, selection_end: u32, marker: String) -> EditResult {
    let sel = (utf16_to_char(&text, selection_start), utf16_to_char(&text, selection_end));
    to_result(markup::toggle_wrap(&text, sel, &marker))
}

/// Cycle the caret's line: plain -> `# ` -> `## ` -> plain.
#[uniffi::export]
pub fn cycle_heading(text: String, caret: u32) -> EditResult {
    let caret = utf16_to_char(&text, caret);
    to_result(markup::cycle_heading(&text, caret))
}

/// Add a `[Title]` section at/below the caret's line.
#[uniffi::export]
pub fn insert_section(text: String, caret: u32, title: String) -> EditResult {
    let caret = utf16_to_char(&text, caret);
    to_result(markup::insert_section(&text, caret, &title))
}

// --- library & files ----------------------------------------------------------------

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    pub title: String,
    pub sections: Vec<String>,
    pub preview: Vec<String>,
}

#[uniffi::export]
pub fn summarize(text: String, fallback_title: String) -> Summary {
    let s = library::summarize(&text, &fallback_title);
    Summary { title: s.title, sections: s.sections, preview: s.preview }
}

#[uniffi::export]
pub fn file_name_for(title: String) -> String {
    library::file_name_for(&title)
}

/// Starter text for a new lyric.
#[uniffi::export]
pub fn new_lyric_template(title: String) -> String {
    library::template(&title)
}

/// A decoded lyric file: text with `\n` newlines plus how to write it back
/// (files from Windows keep their CRLF line endings).
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct DecodedFile {
    pub text: String,
    pub newline: String,
    pub bom: bool,
}

#[uniffi::export]
pub fn decode_file(data: Vec<u8>) -> DecodedFile {
    let (text, format) = textutil::decode_text(&data);
    DecodedFile { text, newline: format.newline.as_str().to_string(), bom: format.bom }
}

#[uniffi::export]
pub fn encode_file(text: String, newline: String, bom: bool) -> Vec<u8> {
    let newline = match newline.as_str() {
        "\r\n" => textutil::Newline::CrLf,
        "\r" => textutil::Newline::Cr,
        _ => textutil::Newline::Lf,
    };
    textutil::encode_text(&text, textutil::FileFormat { newline, bom })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_round_trips() {
        let s = "é👍a";
        assert_eq!(utf16_to_char(s, 0), 0);
        assert_eq!(utf16_to_char(s, 1), 1);
        assert_eq!(utf16_to_char(s, 3), 2); // the emoji is two UTF-16 units
        assert_eq!(char_to_utf16(s, 3), 4);
        assert_eq!(byte_to_utf16(s, s.len()), 4);
    }

    #[test]
    fn styled_runs_use_utf16() {
        let runs = inline_runs("👍 **big**".into());
        assert_eq!((runs[0].start, runs[0].length), (0, 3)); // "👍 "
        assert_eq!((runs[1].start, runs[1].length, runs[1].marker), (3, 2, true));
        assert_eq!((runs[2].start, runs[2].length, runs[2].bold), (5, 3, true));
    }

    #[test]
    fn edits_return_utf16_selection() {
        let r = toggle_wrap("é big".into(), 2, 5, "**".into());
        assert_eq!(r.text, "é **big**");
        assert_eq!((r.selection_start, r.selection_end), (4, 7));
    }

    #[test]
    fn whole_word_and_counts() {
        assert!(is_whole_word("the late night".into(), 9, 14));
        assert!(!is_whole_word("the late night".into(), 9, 12));
        assert_eq!(line_syllable_counts("[Verse]\nMy bitch'll pop a cap\n".into()), vec![0, 6, 0]);
    }

    #[test]
    fn rhyme_groups_dedupe() {
        let groups = rhyme_groups("night".into(), RhymeMode::NearAndPerfect);
        assert_eq!(groups[0].heading, "Perfect");
        let perfect: std::collections::HashSet<_> = groups[0].words.iter().map(|w| w.word.clone()).collect();
        assert!(perfect.contains("light"));
        assert!(groups.iter().skip(1).all(|g| g.words.iter().all(|w| !perfect.contains(&w.word))));
    }

    #[test]
    fn files_keep_crlf() {
        let d = decode_file(b"a\r\nb".to_vec());
        assert_eq!((d.text.as_str(), d.newline.as_str()), ("a\nb", "\r\n"));
        assert_eq!(encode_file(d.text, d.newline, d.bom), b"a\r\nb");
    }
}
