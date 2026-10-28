// No console window behind the app in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use lyrictools::app::{APP_NAME, LyricApp, app_icon};
use lyrictools::settings::{Settings, settings_path};

fn main() -> eframe::Result {
    let open = std::env::args_os().skip(1).find(|a| !a.to_string_lossy().starts_with('-')).map(Into::into);
    let size = Settings::load(&settings_path()).window_size.unwrap_or([900.0, 700.0]);

    let mut viewport = egui::ViewportBuilder::default()
        .with_title(APP_NAME)
        .with_inner_size(size)
        .with_min_inner_size([420.0, 300.0])
        .with_drag_and_drop(true);
    if let Some(icon) = app_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(APP_NAME, options, Box::new(|cc| Ok(Box::new(LyricApp::new(cc, open)))))
}
