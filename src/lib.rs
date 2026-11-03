//! LyricToolsNotepad -- lyric notepad with live syllable counts and rhymes.

pub mod app;
// The shared engine, re-exported so app code keeps using crate::core etc.
pub use lyrics_core::{core, markup, textutil};
mod editor;
mod home;
pub mod library;
pub mod lookup;
pub mod settings;
mod settings_view;
pub mod winshell;
