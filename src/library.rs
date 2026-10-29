//! The lyrics folder shown on the Home dashboard: find the lyric files in it
//! and pull out what a card needs (title, sections, a preview).

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::markup::{self, LineKind};
use crate::textutil::decode_text;

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
    let mut card = summarize(&text, &stem);
    card.path = path.to_path_buf();
    card.modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    card
}

/// Title, section names and a two-line preview from a file's text.
pub fn summarize(text: &str, fallback_title: &str) -> LyricFile {
    let mut title = None;
    let mut sections = Vec::new();
    let mut preview = Vec::new();
    for line in text.lines() {
        match markup::line_kind(line) {
            LineKind::Heading(1) if title.is_none() => title = Some(markup::header_title(line)),
            LineKind::Heading(_) | LineKind::Section => {
                let name = markup::header_title(line);
                if !name.is_empty() && !sections.contains(&name) {
                    sections.push(name);
                }
            }
            LineKind::Plain if preview.len() < 2 && !line.trim().is_empty() => {
                preview.push(strip_markers(line.trim()));
            }
            _ => {}
        }
    }
    LyricFile {
        path: PathBuf::new(),
        title: title.unwrap_or_else(|| fallback_title.to_string()),
        sections,
        preview,
        modified: None,
    }
}

fn strip_markers(line: &str) -> String {
    markup::inline_spans(line).into_iter().filter(|(_, s)| !s.marker).map(|(r, _)| &line[r]).collect()
}

/// "just now", "5 min ago", "3 h ago", "yesterday", "12 days ago"...
pub fn relative_time(modified: Option<SystemTime>) -> String {
    let Some(secs) = modified.and_then(|m| SystemTime::now().duration_since(m).ok()).map(|d| d.as_secs()) else {
        return String::new();
    };
    match secs {
        0..60 => "just now".into(),
        60..3600 => format!("{} min ago", secs / 60),
        3600..86_400 => format!("{} h ago", secs / 3600),
        86_400..172_800 => "yesterday".into(),
        _ if secs < 60 * 86_400 => format!("{} days ago", secs / 86_400),
        _ => format!("{} months ago", secs / (30 * 86_400)),
    }
}

/// A file name from a song title: keeps letters, digits, spaces and
/// `-_'()&,.`; never empty.
pub fn file_name_for(title: &str) -> String {
    let cleaned: String =
        title.trim().chars().map(|c| if c.is_alphanumeric() || " -_'()&,.".contains(c) { c } else { ' ' }).collect();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let cleaned = cleaned.trim_matches('.').to_string();
    if cleaned.is_empty() { "Untitled".into() } else { cleaned }
}

/// `dir/name.txt`, or `dir/name (2).txt` etc. if that's taken.
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(format!("{name}.txt"));
    if !first.exists() {
        return first;
    }
    (2..).map(|n| dir.join(format!("{name} ({n}).txt"))).find(|p| !p.exists()).unwrap()
}

/// Starter text for a new lyric.
pub fn template(title: &str) -> String {
    format!("# {}\n\n[Verse 1]\n", title.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_a_lyric() {
        let card = summarize(
            "# Late Night\n\n[Verse 1]\nI been **droppin'** bars\nsecond line\nthird\n[Chorus]\nhook",
            "file",
        );
        assert_eq!(card.title, "Late Night");
        assert_eq!(card.sections, vec!["Verse 1", "Chorus"]);
        assert_eq!(card.preview, vec!["I been droppin' bars", "second line"]);
        assert_eq!(summarize("just words", "my-song").title, "my-song");
    }

    #[test]
    fn names_and_paths() {
        assert_eq!(file_name_for("  Late Night: Pt. 2?? "), "Late Night Pt. 2");
        assert_eq!(file_name_for("///"), "Untitled");
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
