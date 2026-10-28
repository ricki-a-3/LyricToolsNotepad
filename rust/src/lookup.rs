//! Background word lookups.
//!
//! Every lookup -- even offline rhymes -- runs off the UI thread. Offline
//! work goes to one long-lived worker (which also builds the rhyme indexes at
//! startup); each network lookup gets its own thread, so a slow Datamuse call
//! can never hold up the offline dropdown. Only the newest request's result
//! is delivered; superseded ones are dropped.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use serde::{Deserialize, Serialize};

use crate::core::{self, RhymeKind, WordResult};

pub const DROPDOWN_RESULTS_PER_CATEGORY: usize = 60;
pub const DEEP_SEARCH_MAX_RESULTS: usize = 150;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Perfect,
    Near,
    Report,
    Synonyms,
    Deep,
}

impl Mode {
    pub const ARMED: [Mode; 3] = [Mode::Perfect, Mode::Near, Mode::Report];

    pub fn title(self) -> &'static str {
        match self {
            Mode::Perfect => "Perfect Rhymes",
            Mode::Near => "Near / Slant Rhymes",
            Mode::Report => "Perfect + Near Rhymes",
            Mode::Synonyms => "Synonyms",
            Mode::Deep => "Deep Search",
        }
    }

    pub fn is_network(self) -> bool {
        matches!(self, Mode::Synonyms | Mode::Deep)
    }
}

/// Results under headings ("Perfect", "Near", ...).
pub type Groups = Vec<(&'static str, Vec<WordResult>)>;

pub fn collect(word: &str, mode: Mode) -> Result<Groups, String> {
    let n = DROPDOWN_RESULTS_PER_CATEGORY;
    Ok(match mode {
        Mode::Perfect => vec![("Perfect", core::find_words_local(word, RhymeKind::Perfect, n))],
        Mode::Near => vec![("Near", core::find_words_local(word, RhymeKind::Near, n))],
        Mode::Report => vec![
            ("Perfect", core::find_words_local(word, RhymeKind::Perfect, n)),
            ("Near", core::find_words_local(word, RhymeKind::Near, n)),
        ],
        Mode::Synonyms => vec![("Synonyms", core::find_synonyms(word, n).map_err(|e| e.0)?)],
        Mode::Deep => vec![("Deep Search", core::find_words_deep(word, DEEP_SEARCH_MAX_RESULTS))],
    })
}

pub struct Finished {
    pub id: u64,
    pub word: String,
    pub mode: Mode,
    pub result: Result<Groups, String>,
}

struct Job {
    id: u64,
    word: String,
    mode: Mode,
}

pub struct LookupService {
    offline: Sender<Job>,
    results_tx: Sender<Finished>,
    results: Receiver<Finished>,
    latest: u64,
    repaint: Arc<dyn Fn() + Send + Sync>,
}

impl LookupService {
    /// `repaint` wakes the UI when a result is ready.
    pub fn new(repaint: impl Fn() + Send + Sync + 'static) -> Self {
        let repaint: Arc<dyn Fn() + Send + Sync> = Arc::new(repaint);
        let (results_tx, results) = channel::<Finished>();
        let (offline, jobs) = channel::<Job>();
        let tx = results_tx.clone();
        let wake = repaint.clone();
        thread::Builder::new()
            .name("lyric-offline".into())
            .spawn(move || {
                core::warm_up();
                while let Ok(mut job) = jobs.recv() {
                    // Skip straight to the newest queued job.
                    while let Ok(newer) = jobs.try_recv() {
                        job = newer;
                    }
                    let result = collect(&job.word, job.mode);
                    let _ = tx.send(Finished { id: job.id, word: job.word, mode: job.mode, result });
                    wake();
                }
            })
            .expect("spawn offline worker");
        LookupService { offline, results_tx, results, latest: 0, repaint }
    }

    /// Start a lookup; any older pending request is superseded.
    pub fn request(&mut self, word: &str, mode: Mode) -> u64 {
        self.latest += 1;
        let job = Job { id: self.latest, word: word.to_string(), mode };
        if mode.is_network() {
            let tx = self.results_tx.clone();
            let wake = self.repaint.clone();
            thread::spawn(move || {
                let result = collect(&job.word, job.mode);
                let _ = tx.send(Finished { id: job.id, word: job.word, mode: job.mode, result });
                wake();
            });
        } else {
            let _ = self.offline.send(job);
        }
        self.latest
    }

    /// Drop whatever is in flight (the work itself finishes harmlessly).
    pub fn cancel(&mut self) {
        self.latest += 1;
    }

    /// The newest finished result, if it's still the one we want.
    pub fn poll(&mut self) -> Option<Finished> {
        let mut current = None;
        while let Ok(done) = self.results.try_recv() {
            if done.id == self.latest {
                current = Some(done);
            }
        }
        current
    }
}
