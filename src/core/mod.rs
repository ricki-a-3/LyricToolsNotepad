//! Lyric logic with no UI dependencies: syllable counting, offline
//! perfect/near rhymes from the bundled CMU Pronouncing Dictionary, and the
//! Datamuse-backed Synonyms / Deep Search.
//!
//! A line-for-line behavioral port of the Python `lyrictools.core`; the
//! `tests/parity.rs` suite checks the two produce identical output.
//!
//! Word coverage, in priority order:
//! 1. `LYRIC_EXCEPTIONS` -- hand-picked rap/informal vocabulary CMUdict lacks
//!    or gets wrong for how it's spoken in lyrics.
//! 2. CMUdict -- ~134,000 words, for syllable counts and phonetic rhymes.
//! 3. G-dropping -- an open-class rule mapping "droppin'"/"spittin" to the
//!    "-ing" dictionary form, so any verb in that pattern works.
//! 4. A vowel-group heuristic (syllables) and a spelling-suffix fallback
//!    (rhymes) for made-up words, unusual slang and proper nouns.

pub mod datamuse;
mod dict;
pub mod difflib;
mod rhymes;

use std::sync::LazyLock;

use regex::Regex;

pub use datamuse::{DatamuseError, find_synonyms, find_words_deep};
pub use dict::warm_up;
pub use rhymes::{RhymeKind, find_words_local};

/// One suggestion: the word, its syllable count, and a source-specific score.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WordResult {
    pub word: String,
    pub syllables: u32,
    pub score: i64,
}

static WORD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z]+(?:['\u{2019}][A-Za-z]+)*(?:-[A-Za-z]+(?:['\u{2019}][A-Za-z]+)*)*").unwrap()
});
static HEADER_BRACKET_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\[[^\]]+\]$").unwrap());
static HEADER_LABEL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(verse|chorus|hook|bridge|intro|outro|pre[- ]?chorus)\b.*:$").unwrap());
static VOWEL_GROUP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new("[aeiouy]+").unwrap());
static SIBILANT_END_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new("(?:s|z|sh|ch|x|j|ge)$").unwrap());
static CONSONANT_LE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new("[^aeiou]le$").unwrap());

/// Characters stripped from both ends of a word before lookup.
const EDGE_PUNCT: &[char] = &['\'', '"', '.', ',', '!', '?', ';', ':', '(', ')', '[', ']', '{', '}'];

pub(crate) fn lyric_exception(word: &str) -> Option<u32> {
    Some(match word {
        // Interjections / ad-libs
        "yeah" | "yea" | "nah" | "uh" | "uhh" | "oh" | "ooh" | "ayy" | "ay" | "yo" | "bro" | "bruh" | "fam"
        | "mane" | "skrrt" | "woo" | "yah" | "nuh" => 1,
        "ayo" => 2,
        // "to/have/of" reductions -- a closed class, so listed outright
        "gon" => 1,
        "gonna" | "wanna" | "gotta" | "kinda" | "sorta" | "lemme" | "gimme" | "tryna" | "finna" | "imma" | "boutta"
        | "outta" | "lotta" | "hafta" | "needa" | "useta" | "oughta" | "shoulda" | "coulda" | "woulda" | "musta"
        | "mighta" => 2,
        // Dropped/relaxed consonants
        "wit" | "dat" | "dis" | "dem" | "dey" | "deez" | "doe" | "nuff" | "lil" | "ol" => 1,
        "prolly" => 2,
        // Contractions, with AND without the apostrophe -- lyrics are rarely
        // typed with consistent punctuation.
        "ain't" | "aint" | "cause" | "cuz" | "coz" | "bout" | "em" | "yall" | "y'all" | "im" | "i'm" | "dont"
        | "don't" | "wont" | "won't" | "cant" | "can't" | "arent" | "aren't" | "werent" | "weren't" | "til"
        | "'til" => 1,
        "wasnt" | "wasn't" | "isnt" | "isn't" | "hasnt" | "hasn't" | "hadnt" | "hadn't" | "doesnt" | "doesn't"
        | "didnt" | "didn't" | "couldnt" | "couldn't" | "wouldnt" | "wouldn't" | "shouldnt" | "shouldn't" => 2,
        _ => return None,
    })
}

fn letter_syllables(c: char) -> u32 {
    if c == 'W' { 3 } else { 1 }
}

/// Lowercase, straighten curly apostrophes, strip edge punctuation.
pub(crate) fn normalize_word(word: &str) -> String {
    word.to_lowercase().replace('\u{2019}', "'").trim_matches(EDGE_PUNCT).to_string()
}

fn only_chars(s: &str, keep: impl Fn(char) -> bool) -> String {
    s.chars().filter(|&c| keep(c)).collect()
}

/// Map a dropped-g spelling back to its dictionary form ("droppin'" ->
/// "dropping"). Only ever tried after a direct lookup failed, so real words
/// ending in "in" (win, pin, sin) never reach it.
pub(crate) fn gdrop_normalize(word: &str) -> Option<String> {
    let w = word.strip_suffix('\'').unwrap_or(word);
    if w.chars().count() > 3 && w.ends_with("in") && !w.ends_with("ing") { Some(format!("{w}g")) } else { None }
}

fn heuristic_syllables(original: &str) -> u32 {
    let lowered = original.to_lowercase().replace('\u{2019}', "'");
    let cleaned = only_chars(&lowered, |c| c.is_ascii_lowercase() || c == '\'');
    let word = cleaned.trim_matches('\'');
    if word.is_empty() {
        return 0;
    }
    if let Some(n) = lyric_exception(word) {
        return n;
    }
    if word.len() > 2
        && let Some(base) = word.strip_suffix("'s")
    {
        let extra = if SIBILANT_END_RE.is_match(base) { 1 } else { 0 };
        return syllables_for_word(base) + extra;
    }

    let letters = only_chars(original, |c| c.is_ascii_alphabetic());
    if (2..=6).contains(&letters.len()) && letters.chars().all(|c| c.is_ascii_uppercase()) {
        return letters.chars().map(letter_syllables).sum();
    }

    let mut count = VOWEL_GROUP_RE.find_iter(word).count() as i64;

    // "'ll" after a consonant is its own spoken syllable (bitch'll, it'll);
    // after a vowel it glides in (he'll, we'll, I'll).
    if word.len() > 3
        && let Some(before) = word.strip_suffix("'ll")
        && before.chars().last().is_some_and(|c| !"aeiouy".contains(c))
    {
        count += 1;
    }
    if word.len() > 3 && word.ends_with('e') && !CONSONANT_LE_RE.is_match(word) {
        count -= 1;
    }
    if word.len() > 4
        && word.ends_with("ed")
        && !(word.ends_with("ted") || word.ends_with("ded"))
        && VOWEL_GROUP_RE.is_match(&word[..word.len() - 2])
    {
        count -= 1;
    }
    for pattern in ["ia", "io", "eo", "ua"] {
        if word.contains(pattern) {
            count += 1;
        }
    }
    count.max(1) as u32
}

/// Syllables in one word, as spoken in lyrics.
pub fn syllables_for_word(word: &str) -> u32 {
    if word.is_empty() {
        return 0;
    }
    let normalized = normalize_word(word);
    if normalized.is_empty() {
        return 0;
    }
    if let Some(n) = lyric_exception(&normalized) {
        return n;
    }
    let counts = dict::counts();
    if let Some(&n) = counts.get(&normalized) {
        return n;
    }
    if normalized.contains('-') {
        let parts: Vec<&str> = normalized.split('-').filter(|p| !p.is_empty()).collect();
        if !parts.is_empty() {
            return parts.iter().map(|p| syllables_for_word(p)).sum();
        }
    }
    let stripped = only_chars(&normalized, |c| c.is_ascii_lowercase() || c == '\'');
    if let Some(&n) = counts.get(&stripped) {
        return n;
    }
    if let Some(g) = gdrop_normalize(&stripped)
        && let Some(&n) = counts.get(&g)
    {
        return n;
    }
    heuristic_syllables(word)
}

/// Words in a line, as the syllable counter sees them.
pub fn tokenize_line(text: &str) -> Vec<&str> {
    WORD_RE.find_iter(text).map(|m| m.as_str()).collect()
}

/// `[Verse 1]`, `Chorus:`, `Pre-Chorus:` ... -- these get no syllable count.
pub fn is_section_header(text: &str) -> bool {
    let stripped = text.trim();
    HEADER_BRACKET_RE.is_match(stripped) || HEADER_LABEL_RE.is_match(stripped)
}

pub fn syllables_for_line(text: &str) -> u32 {
    if text.is_empty() || is_section_header(text) {
        return 0;
    }
    tokenize_line(text).into_iter().map(syllables_for_word).sum()
}
