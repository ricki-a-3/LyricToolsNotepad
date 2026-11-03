//! Pure helpers for lyric files: what a library card shows, file names and
//! the new-lyric template. (Folder scanning lives in each app.)

use std::time::SystemTime;

use crate::markup::{self, LineKind};

/// What a library card shows for one lyric file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LyricSummary {
    /// First `#` heading, else the fallback (usually the file name).
    pub title: String,
    pub sections: Vec<String>,
    /// First two lyric lines, markers stripped.
    pub preview: Vec<String>,
}

/// Title, section names and a two-line preview from a file's text.
pub fn summarize(text: &str, fallback_title: &str) -> LyricSummary {
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
    LyricSummary { title: title.unwrap_or_else(|| fallback_title.to_string()), sections, preview }
}

/// A line without its `**` / `*` / `__` markers.
pub fn strip_markers(line: &str) -> String {
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
    fn file_names() {
        assert_eq!(file_name_for("  Late Night: Pt. 2?? "), "Late Night Pt. 2");
        assert_eq!(file_name_for("///"), "Untitled");
    }
}
