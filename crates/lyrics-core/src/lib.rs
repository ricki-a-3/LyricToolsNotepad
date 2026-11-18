//! The LyricToolsNotepad lyric engine, shared by the Windows app and the
//! iOS app: syllable counting, offline rhymes (CMUdict), Datamuse lookups,
//! lyric markup, and file-format helpers. No UI code.

pub mod core;
pub mod library;
pub mod markup;
pub mod textutil;
