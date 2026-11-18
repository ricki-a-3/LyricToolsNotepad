//! Datamuse-backed lookups: Synonyms and Deep Search.
//!
//! These block on the network, so the UI only ever calls them from a worker
//! thread, and only as explicit one-shot commands -- never from the passive
//! highlight dropdown. A network call on passive highlight is what froze the
//! original plugin.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use serde_json::Value;

use super::rhymes::local_rhymes;
use super::{WordResult, syllables_for_line, syllables_for_word};

pub const TIMEOUT: Duration = Duration::from_secs(6);

#[derive(Debug, Clone)]
pub struct DatamuseError(pub String);

impl std::fmt::Display for DatamuseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Query params -> result rows. Swappable so tests never touch the network.
pub type Fetch<'a> = &'a dyn Fn(&[(&str, String)]) -> Result<Vec<Value>, DatamuseError>;

/// Sorted query params -> rows.
type Cache = HashMap<Vec<(String, String)>, Vec<Value>>;

static CACHE: LazyLock<Mutex<Cache>> = LazyLock::new(Default::default);

/// GET https://api.datamuse.com/words with the given params (cached).
pub fn fetch(params: &[(&str, String)]) -> Result<Vec<Value>, DatamuseError> {
    let mut key: Vec<(String, String)> = params.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    key.sort();
    if let Some(hit) = CACHE.lock().unwrap().get(&key) {
        return Ok(hit.clone());
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(concat!("LyricToolsNotepad/", env!("CARGO_PKG_VERSION"), " (+https://www.datamuse.com/api/)"))
        .build()
        .into();
    let mut request = agent.get("https://api.datamuse.com/words");
    for (k, v) in params {
        request = request.query(*k, v);
    }
    let value: Value = request
        .call()
        .map_err(|e| DatamuseError(friendly(&e)))?
        .body_mut()
        .read_json()
        .map_err(|e| DatamuseError(format!("bad response: {e}")))?;
    let rows = match value {
        Value::Array(rows) => rows,
        _ => Vec::new(),
    };
    CACHE.lock().unwrap().insert(key, rows.clone());
    Ok(rows)
}

fn friendly(err: &ureq::Error) -> String {
    match err {
        ureq::Error::Timeout(_) => "timed out".into(),
        ureq::Error::Io(_) | ureq::Error::HostNotFound | ureq::Error::ConnectionFailed => "no connection".into(),
        other => other.to_string(),
    }
}

fn row_word(row: &Value) -> String {
    row.get("word").and_then(Value::as_str).unwrap_or("").trim().to_string()
}

fn row_score(row: &Value) -> i64 {
    match row.get("score") {
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).unwrap_or(0),
        _ => 0,
    }
}

/// Synonyms for one word (online).
pub fn find_synonyms(anchor: &str, max_results: usize) -> Result<Vec<WordResult>, DatamuseError> {
    find_synonyms_with(anchor, max_results, &fetch)
}

pub fn find_synonyms_with(anchor: &str, max_results: usize, fetch: Fetch) -> Result<Vec<WordResult>, DatamuseError> {
    let anchor = anchor.trim();
    if anchor.is_empty() {
        return Ok(Vec::new());
    }
    let max = (max_results * 2).clamp(100, 250);
    let rows = fetch(&[("rel_syn", anchor.to_string()), ("max", max.to_string()), ("md", "s".into())])?;
    Ok(normalize_results(&rows, anchor, max_results))
}

fn normalize_results(rows: &[Value], anchor: &str, max_results: usize) -> Vec<WordResult> {
    let anchor_l = anchor.to_lowercase();
    let anchor_syl = syllables_for_word(anchor) as i64;
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for row in rows {
        let word = row_word(row);
        let lower = word.to_lowercase();
        if word.is_empty() || lower == anchor_l || !seen.insert(lower) {
            continue;
        }
        let syllables = match row.get("numSyllables").and_then(Value::as_u64) {
            Some(n) => n as u32,
            None => syllables_for_line(&word),
        };
        out.push(WordResult { word, syllables, score: row_score(row) });
    }
    out.sort_by(|a, b| {
        (a.syllables as i64 - anchor_syl)
            .abs()
            .cmp(&(b.syllables as i64 - anchor_syl).abs())
            .then(b.score.cmp(&a.score))
            .then(a.word.to_lowercase().cmp(&b.word.to_lowercase()))
    });
    out.truncate(max_results);
    out
}

/// Maximum-coverage rhymes: the full offline engine merged with Datamuse's
/// perfect (rel_rhy) and near (rel_nry) relations.
///
/// Ranking is by source tier, not raw score: offline perfect -> offline near
/// -> Datamuse perfect -> Datamuse near. Scores from different sources aren't
/// comparable (offline near scores can exceed the perfect baseline of 1000),
/// so blending them let slant rhymes outrank true perfect rhymes.
///
/// Never fails: if Datamuse is unreachable you still get the offline tiers.
pub fn find_words_deep(anchor: &str, max_results: usize) -> Vec<WordResult> {
    find_words_deep_with(anchor, max_results, &fetch)
}

pub fn find_words_deep_with(anchor: &str, max_results: usize, fetch: Fetch) -> Vec<WordResult> {
    let anchor = anchor.trim();
    if anchor.is_empty() {
        return Vec::new();
    }
    let anchor_l = anchor.to_lowercase();
    let anchor_syl = syllables_for_word(anchor) as i64;
    // word (lowercase) -> (tier, result)
    let mut combined: HashMap<String, (u8, WordResult)> = HashMap::new();
    let mut consider = |word: &str, tier: u8, score: i64, syllables: u32| {
        let word = word.trim();
        let key = word.to_lowercase();
        if key.is_empty() || key == anchor_l {
            return;
        }
        if combined.get(&key).is_some_and(|(t, _)| *t <= tier) {
            return; // already have it from an equal-or-better tier
        }
        combined.insert(key, (tier, WordResult { word: word.to_string(), syllables, score }));
    };

    for r in local_rhymes(anchor, false, max_results) {
        consider(&r.word, 0, r.score, r.syllables);
    }
    for r in local_rhymes(anchor, true, max_results) {
        consider(&r.word, 1, r.score, r.syllables);
    }
    for (tier, relation) in [(2u8, "rel_rhy"), (3u8, "rel_nry")] {
        // One relation failing shouldn't lose the other or the offline tiers.
        let Ok(rows) = fetch(&[(relation, anchor.to_string()), ("max", "250".into())]) else { continue };
        for row in &rows {
            let word = row_word(row);
            if !word.is_empty() {
                consider(&word, tier, row_score(row), syllables_for_line(&word));
            }
        }
    }

    let mut entries: Vec<(u8, WordResult)> = combined.into_values().collect();
    entries.sort_by(|(ta, a), (tb, b)| {
        ta.cmp(tb)
            .then(b.score.cmp(&a.score))
            .then((a.syllables as i64 - anchor_syl).abs().cmp(&(b.syllables as i64 - anchor_syl).abs()))
            .then(a.word.cmp(&b.word))
    });
    entries.into_iter().take(max_results).map(|(_, r)| r).collect()
}
