//! Offline perfect / near (slant) rhymes from CMUdict phonemes, with the
//! g-dropping and spelling-suffix fallbacks.

use std::collections::{HashMap, HashSet};

use super::dict::{prons, rhyme_indexes, rhyme_tail, spelling_index};
use super::{WordResult, gdrop_normalize, normalize_word, syllables_for_word};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RhymeKind {
    Perfect,
    Near,
}

/// Fast, network-free lookup used by the highlight dropdown.
pub fn find_words_local(anchor: &str, kind: RhymeKind, max_results: usize) -> Vec<WordResult> {
    let anchor = anchor.trim();
    if anchor.is_empty() {
        return Vec::new();
    }
    local_rhymes(anchor, kind == RhymeKind::Near, max_results)
}

pub(crate) fn local_rhymes(anchor: &str, near: bool, max_results: usize) -> Vec<WordResult> {
    let anchor = normalize_word(anchor);
    let prons = prons();
    let mut variants = prons.get(&anchor);
    let mut lookup_word = anchor.clone();

    if variants.is_none() {
        // "droppin'" -> "dropping": the normalized form's pronunciation is a
        // legitimate stand-in for the slang spelling.
        if let Some(g) = gdrop_normalize(&anchor)
            && let Some(gv) = prons.get(&g)
        {
            variants = Some(gv);
            lookup_word = g;
        }
    }
    let Some(variants) = variants else {
        return orthographic_rhymes(&anchor, max_results);
    };

    let indexes = rhyme_indexes();
    let anchor_syl = syllables_for_word(&anchor) as i64;
    let anchor_tails: Vec<Vec<u8>> = variants.iter().map(|p| rhyme_tail(p)).filter(|t| !t.is_empty()).collect();
    let is_anchor = |w: &str| w == lookup_word || w == anchor;
    let mut scores: HashMap<usize, i64> = HashMap::new();

    if !near {
        for tail in &anchor_tails {
            for &id in indexes.perfect.get(tail).into_iter().flatten() {
                if !is_anchor(&prons.words[id]) {
                    scores.insert(id, 1000);
                }
            }
        }
    } else {
        let mut candidates: HashSet<usize> = HashSet::new();
        for tail in &anchor_tails {
            candidates.extend(indexes.by_last.get(tail.last().unwrap()).into_iter().flatten());
            candidates.extend(indexes.by_vowel.get(&tail[0]).into_iter().flatten());
        }
        for id in candidates {
            let word = &prons.words[id];
            if is_anchor(word) {
                continue;
            }
            let syl_diff = (syllables_for_word(word) as i64 - anchor_syl).abs();
            let mut best = 0.0f64;
            for atail in &anchor_tails {
                for phones in &prons.variants[id] {
                    let ctail = rhyme_tail(phones);
                    if ctail.is_empty() || &ctail == atail {
                        continue; // keep the near list focused on slant rhymes
                    }
                    let mut score = super::difflib::ratio(atail, &ctail);
                    if atail.last() == ctail.last() {
                        score += 0.18;
                    }
                    if atail[0] == ctail[0] {
                        score += 0.14;
                    }
                    if atail.len() >= 2 && ctail.len() >= 2 && atail[atail.len() - 2..] == ctail[ctail.len() - 2..] {
                        score += 0.10;
                    }
                    score -= (syl_diff as f64 * 0.04).min(0.18);
                    if score > best {
                        best = score;
                    }
                }
            }
            if best >= 0.60 {
                scores.insert(id, (best * 1000.0) as i64);
            }
        }
    }

    let mut out: Vec<WordResult> = scores
        .into_iter()
        .map(|(id, score)| {
            let word = prons.words[id].clone();
            WordResult { syllables: syllables_for_word(&word), word, score }
        })
        .collect();
    out.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then((a.syllables as i64 - anchor_syl).abs().cmp(&(b.syllables as i64 - anchor_syl).abs()))
            .then(a.word.cmp(&b.word))
    });
    out.truncate(max_results);
    out
}

/// Last resort for words with no phonetic data: dictionary words sharing the
/// last 4, 3 or 2 letters. Spelling isn't pronunciation, so these score 1
/// (far below phonetic matches) -- but "probably close" beats nothing.
fn orthographic_rhymes(anchor: &str, max_results: usize) -> Vec<WordResult> {
    let clean: String = anchor.chars().filter(|c| c.is_ascii_lowercase()).collect();
    if clean.len() < 2 {
        return Vec::new();
    }
    let index = spelling_index();
    let anchor_syl = syllables_for_word(anchor) as i64;
    let mut words: &[String] = &[];
    for n in [4, 3, 2] {
        if clean.len() >= n {
            words = index.get(&(n, clean[clean.len() - n..].to_string())).map_or(&[], |v| v.as_slice());
            if !words.is_empty() {
                break;
            }
        }
    }
    let mut out: Vec<WordResult> = words
        .iter()
        .filter(|w| **w != clean && *w != anchor)
        .map(|w| WordResult { word: w.clone(), syllables: syllables_for_word(w), score: 1 })
        .collect();
    out.sort_by(|a, b| {
        (a.syllables as i64 - anchor_syl).abs().cmp(&(b.syllables as i64 - anchor_syl).abs()).then(a.word.cmp(&b.word))
    });
    out.truncate(max_results);
    out
}
