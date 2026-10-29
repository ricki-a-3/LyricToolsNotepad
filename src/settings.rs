//! User settings, stored as JSON.
//!
//! Portable builds ship a `portable.txt` marker next to the exe and keep
//! `settings.json` beside it (nothing in AppData, so the folder can live on a
//! USB stick). Otherwise settings go in the per-user config folder.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::lookup::Mode;

pub const MAX_RECENT: usize = 10;

/// What the app shows when it starts without a file to open.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StartPage {
    Home,
    LastFile,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub dark: bool,
    pub mode: Mode,
    pub font_size: f32,
    pub wrap: bool,
    pub recent: Vec<PathBuf>,
    pub last_dir: Option<PathBuf>,
    pub window_size: Option<[f32; 2]>,
    /// Folder the Home dashboard lists and new lyrics are saved to.
    pub lyrics_dir: Option<PathBuf>,
    pub start_page: StartPage,
    pub last_file: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            dark: true,
            mode: Mode::Report,
            font_size: 17.0,
            wrap: true,
            recent: Vec::new(),
            last_dir: None,
            window_size: None,
            lyrics_dir: None,
            start_page: StartPage::Home,
            last_file: None,
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }

    pub fn remember(&mut self, path: &Path) {
        self.recent.retain(|p| !same_path(p, path));
        self.recent.insert(0, path.to_path_buf());
        self.recent.truncate(MAX_RECENT);
        self.last_dir = path.parent().map(Path::to_path_buf);
        self.last_file = Some(path.to_path_buf());
    }

    pub fn lyrics_dir(&self) -> PathBuf {
        self.lyrics_dir.clone().unwrap_or_else(crate::library::default_dir)
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    if cfg!(windows) { a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy()) } else { a == b }
}

/// Where settings live (see module docs).
pub fn settings_path() -> PathBuf {
    if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf))
        && dir.join("portable.txt").exists()
    {
        return dir.join("settings.json");
    }
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("LyricToolsNotepad").join("settings.json")
}
