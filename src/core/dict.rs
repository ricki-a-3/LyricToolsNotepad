//! The bundled CMU Pronouncing Dictionary, compiled into the binary so the
//! portable build is a single .exe. Parsed lazily, once, on first use.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

static CMUDICT: &str = include_str!("../../data/cmudict.dict");

/// Phoneme with its stress digit split off: `AY1` -> base id of "AY", stress 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Phone {
    pub base: u8,
    pub stress: Option<u8>,
}

pub(crate) struct Pronunciations {
    pub words: Vec<String>,
    pub index: HashMap<String, usize>,
    /// Per word id: distinct pronunciations, in dictionary order.
    pub variants: Vec<Vec<Vec<Phone>>>,
}

impl Pronunciations {
    pub fn get(&self, word: &str) -> Option<&[Vec<Phone>]> {
        self.index.get(word).map(|&id| self.variants[id].as_slice())
    }
}

/// Perfect rhymes share a tail; near-rhyme candidates share the tail's last
/// phone or its stressed vowel.
pub(crate) struct RhymeIndexes {
    pub perfect: HashMap<Vec<u8>, Vec<usize>>,
    pub by_last: HashMap<u8, Vec<usize>>,
    pub by_vowel: HashMap<u8, Vec<usize>>,
}

/// Lines as (headword with any "(2)" variant marker removed, phones).
fn entries() -> impl Iterator<Item = (String, Vec<&'static str>)> {
    CMUDICT.lines().filter_map(|raw| {
        let raw = raw.trim();
        if raw.is_empty() || raw.starts_with(";;;") {
            return None;
        }
        // A few entries carry a trailing "# place, danish" style comment.
        let raw = raw.split('#').next().unwrap_or("");
        let mut parts = raw.split_whitespace();
        let head = parts.next()?;
        let phones: Vec<&'static str> = parts.collect();
        if phones.is_empty() {
            return None;
        }
        let word = head.to_lowercase();
        let word = match word.rfind('(') {
            Some(i)
                if word.ends_with(')')
                    && word[i + 1..word.len() - 1].chars().all(|c| c.is_ascii_digit())
                    && i + 2 < word.len() =>
            {
                word[..i].to_string()
            }
            _ => word,
        };
        Some((word, phones))
    })
}

fn has_stress_digit(phone: &str) -> bool {
    phone.chars().last().is_some_and(|c| c.is_ascii_digit())
}

static COUNTS: LazyLock<HashMap<String, u32>> = LazyLock::new(|| {
    let mut counts = HashMap::new();
    for (word, phones) in entries() {
        if counts.contains_key(&word) {
            continue;
        }
        let syllables = phones.iter().filter(|p| has_stress_digit(p)).count() as u32;
        if syllables > 0 {
            counts.insert(word, syllables);
        }
    }
    counts
});

/// Phone base names ("AY", "T", ...) <-> small ids, so tails are cheap to
/// hash and compare.
static PHONE_IDS: LazyLock<std::sync::Mutex<HashMap<String, u8>>> = LazyLock::new(Default::default);

fn phone_id(base: &str) -> u8 {
    let mut ids = PHONE_IDS.lock().unwrap();
    if let Some(&id) = ids.get(base) {
        return id;
    }
    let id = u8::try_from(ids.len()).expect("more than 256 distinct phones");
    ids.insert(base.to_string(), id);
    id
}

fn parse_phone(raw: &str) -> Phone {
    let base: String = raw.chars().filter(|c| !c.is_ascii_digit()).collect();
    let stress = raw.chars().last().and_then(|c| c.to_digit(10)).map(|d| d as u8);
    Phone { base: phone_id(&base), stress }
}

static PRONS: LazyLock<Pronunciations> = LazyLock::new(|| {
    let mut prons = Pronunciations { words: Vec::new(), index: HashMap::new(), variants: Vec::new() };
    for (word, phones) in entries() {
        if !word.chars().any(|c| c.is_ascii_lowercase()) {
            continue;
        }
        let phones: Vec<Phone> = phones.iter().map(|p| parse_phone(p)).collect();
        let id = *prons.index.entry(word.clone()).or_insert_with(|| {
            prons.words.push(word);
            prons.variants.push(Vec::new());
            prons.variants.len() - 1
        });
        if !prons.variants[id].contains(&phones) {
            prons.variants[id].push(phones);
        }
    }
    prons
});

/// The final stressed-vowel rhyme portion, as phone base ids.
pub(crate) fn rhyme_tail(phones: &[Phone]) -> Vec<u8> {
    if phones.is_empty() {
        return Vec::new();
    }
    let start = phones
        .iter()
        .rposition(|p| matches!(p.stress, Some(1) | Some(2)))
        .or_else(|| phones.iter().rposition(|p| p.stress.is_some()))
        .unwrap_or(phones.len().saturating_sub(2));
    phones[start..].iter().map(|p| p.base).collect()
}

static RHYME_INDEXES: LazyLock<RhymeIndexes> = LazyLock::new(|| {
    let prons = prons();
    let mut perfect: HashMap<Vec<u8>, HashSet<usize>> = HashMap::new();
    let mut by_last: HashMap<u8, HashSet<usize>> = HashMap::new();
    let mut by_vowel: HashMap<u8, HashSet<usize>> = HashMap::new();
    for (id, variants) in prons.variants.iter().enumerate() {
        for phones in variants {
            let tail = rhyme_tail(phones);
            let (Some(&first), Some(&last)) = (tail.first(), tail.last()) else { continue };
            by_last.entry(last).or_default().insert(id);
            by_vowel.entry(first).or_default().insert(id);
            perfect.entry(tail).or_default().insert(id);
        }
    }
    RhymeIndexes { perfect: flatten(perfect), by_last: flatten(by_last), by_vowel: flatten(by_vowel) }
});

fn flatten<K: std::hash::Hash + Eq>(m: HashMap<K, HashSet<usize>>) -> HashMap<K, Vec<usize>> {
    m.into_iter().map(|(k, v)| (k, v.into_iter().collect())).collect()
}

/// Dictionary words by their last 2-4 letters -- the last-resort spelling
/// match for words with no phonetic data at all.
static SPELLING_INDEX: LazyLock<HashMap<(usize, String), Vec<String>>> = LazyLock::new(|| {
    let mut index: HashMap<(usize, String), Vec<String>> = HashMap::new();
    for word in counts().keys() {
        let clean: String = word.chars().filter(|c| c.is_ascii_lowercase()).collect();
        if clean.len() < 2 {
            continue;
        }
        for n in [4, 3, 2] {
            if clean.len() >= n {
                index.entry((n, clean[clean.len() - n..].to_string())).or_default().push(word.clone());
            }
        }
    }
    index
});

pub(crate) fn counts() -> &'static HashMap<String, u32> {
    &COUNTS
}

pub(crate) fn prons() -> &'static Pronunciations {
    &PRONS
}

pub(crate) fn rhyme_indexes() -> &'static RhymeIndexes {
    &RHYME_INDEXES
}

pub(crate) fn spelling_index() -> &'static HashMap<(usize, String), Vec<String>> {
    &SPELLING_INDEX
}

/// Build everything up front (call from a background thread at startup) so
/// the first highlighted word gets its dropdown instantly.
pub fn warm_up() {
    let _ = (counts().len(), rhyme_indexes().perfect.len(), spelling_index().len());
}
