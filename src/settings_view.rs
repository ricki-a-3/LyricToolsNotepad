//! The Settings page.

use eframe::egui::{self, RichText};

use crate::app::LyricApp;
use crate::home::reveal;
use crate::library;
use crate::lookup::Mode;
use crate::settings::{Settings, StartPage};
use crate::winshell;

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(14.0);
    ui.label(RichText::new(title).size(16.0).strong());
    ui.add_space(4.0);
}

impl LyricApp {
    pub(crate) fn settings_page(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.set_max_width(640.0);
            ui.add_space(8.0);
            ui.heading(RichText::new("Settings").size(26.0).strong());

            section(ui, "Lyrics folder");
            ui.label(
                RichText::new("New lyrics are saved here, and the Home page lists every lyric file in it.").weak(),
            );
            let dir = self.settings.lyrics_dir();
            ui.horizontal(|ui| {
                ui.monospace(dir.display().to_string());
            });
            ui.horizontal(|ui| {
                if ui.button("Change...").clicked()
                    && let Some(picked) = rfd::FileDialog::new().set_directory(&dir).pick_folder()
                {
                    self.settings.lyrics_dir = Some(picked);
                    self.library_stale = true;
                    changed = true;
                }
                if ui.button("Open folder").clicked() {
                    let _ = std::fs::create_dir_all(&dir);
                    reveal(&dir);
                }
                if self.settings.lyrics_dir.is_some() && ui.button("Reset to default").clicked() {
                    self.settings.lyrics_dir = None;
                    self.library_stale = true;
                    changed = true;
                }
            });
            if self.settings.lyrics_dir.is_none() {
                ui.label(RichText::new(format!("Default: {}", library::default_dir().display())).small().weak());
            }

            section(ui, "When LyricToolsNotepad starts");
            changed |= ui.radio_value(&mut self.settings.start_page, StartPage::Home, "Show the Home page").changed();
            changed |= ui
                .radio_value(&mut self.settings.start_page, StartPage::LastFile, "Reopen the last lyric I worked on")
                .changed();

            section(ui, "Appearance");
            changed |= ui.checkbox(&mut self.settings.dark, "Dark theme").changed();
            changed |= ui.checkbox(&mut self.settings.wrap, "Wrap long lines").changed();
            ui.horizontal(|ui| {
                ui.label("Text size");
                changed |= ui.add(egui::Slider::new(&mut self.settings.font_size, 10.0..=36.0).step_by(1.0)).changed();
                if ui.small_button("Reset").clicked() {
                    self.settings.font_size = Settings::default().font_size;
                    changed = true;
                }
            });

            section(ui, "Rhymes");
            ui.label(
                RichText::new("Which rhymes appear when you highlight a word (also on the editor toolbar).").weak(),
            );
            for (mode, label) in [(Mode::Perfect, "Perfect"), (Mode::Near, "Near"), (Mode::Report, "Near + Perfect")] {
                changed |= ui.radio_value(&mut self.settings.mode, mode, label).changed();
            }

            section(ui, "Windows right-click menu");
            if cfg!(windows) {
                let registered = *self.shell_registered.get_or_insert_with(winshell::is_registered);
                let mut on = registered;
                if ui.checkbox(&mut on, format!("Show \u{201C}{}\u{201D} on .txt files", winshell::MENU_TEXT)).changed()
                {
                    let result = if on { winshell::register() } else { winshell::unregister() };
                    match result {
                        Ok(()) => self.shell_registered = Some(on),
                        Err(err) => {
                            let ctx = ui.ctx().clone();
                            self.flash(&ctx, format!("Couldn't change the right-click menu: {err}"));
                        }
                    }
                }
                ui.label(
                    RichText::new("Points at this copy of the app. The installer turns this on for you.")
                        .small()
                        .weak(),
                );
            } else {
                ui.label(RichText::new("Available on Windows.").weak());
            }

            section(ui, "About");
            ui.label(format!("LyricToolsNotepad {}", env!("CARGO_PKG_VERSION")));
            ui.label(RichText::new(format!("Settings file: {}", self.settings_file().display())).small().weak());
        });
        if changed {
            self.save_settings();
        }
    }
}
