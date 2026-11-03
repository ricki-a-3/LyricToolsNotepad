//! The lyrics folder shown on the Home dashboard: find the lyric files in it
//! and pull out what a card needs (title, sections, a preview).

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::textutil::decode_text;
pub use lyrics_core::library::{LyricSummary, file_name_for, relative_time, summarize, template};

pub const EXTENSIONS: [&str; 3] = ["txt", "lyrics", "md"];
const MAX_DEPTH: usize = 3;
const MAX_FILES: usize = 2000;
/// Only the start of each file is read for the card.
const PEEK_BYTES: u64 = 16 * 1024;

#[derive(Clone, Debug)]
pub struct LyricFile {
    pub path: PathBuf,
    /// First `#` heading, else the file name without extension.
    pub title: String,
    pub sections: Vec<String>,
    pub preview: Vec<String>,
    pub modified: Option<SystemTime>,
}

/// Default lyrics folder: Documents\LyricToolsNotepad.
pub fn default_dir() -> PathBuf {
    dirs::document_dir().or_else(dirs::home_dir).unwrap_or_else(|| PathBuf::from(".")).join("LyricToolsNotepad")
}

pub fn is_lyric_file(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

/// Every lyric file under `dir` (a few levels deep), newest first.
pub fn scan(dir: &Path) -> Vec<LyricFile> {
    let mut paths = Vec::new();
    collect(dir, 0, &mut paths);
    let mut files: Vec<LyricFile> = paths.into_iter().map(|p| read_card(&p)).collect();
    files.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase())));
    files
}

fn collect(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        if out.len() >= MAX_FILES {
            return;
        }
        let path = entry.path();
        let hidden = path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with('.'));
        let Ok(kind) = entry.file_type() else { continue };
        if hidden {
            continue;
        }
        if kind.is_dir() && depth + 1 < MAX_DEPTH {
            collect(&path, depth + 1, out);
        } else if kind.is_file() && is_lyric_file(&path) {
            out.push(path);
        }
    }
}

pub fn read_card(path: &Path) -> LyricFile {
    use std::io::Read;
    let mut bytes = Vec::new();
    if let Ok(f) = std::fs::File::open(path) {
        let _ = f.take(PEEK_BYTES).read_to_end(&mut bytes);
    }
    let (text, _) = decode_text(&bytes);
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let summary = summarize(&text, &stem);
    LyricFile {
        path: path.to_path_buf(),
        title: summary.title,
        sections: summary.sections,
        preview: summary.preview,
        modified: std::fs::metadata(path).and_then(|m| m.modified()).ok(),
    }
}

/// `dir/name.txt`, or `dir/name (2).txt` etc. if that's taken.
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(format!("{name}.txt"));
    if !first.exists() {
        return first;
    }
    (2..).map(|n| dir.join(format!("{name} ({n}).txt"))).find(|p| !p.exists()).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_paths() {
        let dir = tempfile::tempdir().unwrap();
        let a = unique_path(dir.path(), "Song");
        assert_eq!(a.file_name().unwrap(), "Song.txt");
        std::fs::write(&a, "x").unwrap();
        assert_eq!(unique_path(dir.path(), "Song").file_name().unwrap(), "Song (2).txt");
    }

    #[test]
    fn scans_folder() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "# Alpha\nline").unwrap();
        std::fs::write(dir.path().join("notes.pdf"), "nope").unwrap();
        std::fs::create_dir(dir.path().join("Album")).unwrap();
        std::fs::write(dir.path().join("Album").join("b.TXT"), "[Hook]\nyeah").unwrap();
        std::fs::create_dir(dir.path().join(".hidden")).unwrap();
        std::fs::write(dir.path().join(".hidden").join("c.txt"), "x").unwrap();
        let mut titles: Vec<String> = scan(dir.path()).into_iter().map(|f| f.title).collect();
        titles.sort();
        assert_eq!(titles, vec!["Alpha", "b"]);
    }
}
