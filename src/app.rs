//! The app shell: Home / Editor / Settings views, menus, shortcuts, files.
//! The editor view lives in `editor.rs`, the dashboard in `home.rs`, the
//! settings page in `settings_view.rs`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, Key, Modifiers};
use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};

use crate::editor::{Pending, Popup};
use crate::library::{self, LyricFile};
use crate::lookup::{LookupService, Mode};
use crate::settings::{Settings, StartPage, settings_path};
use crate::textutil::{FileFormat, decode_text, encode_text};

pub const APP_NAME: &str = "LyricToolsNotepad";
const STATUS_MESSAGE_SECS: f64 = 5.0;
const APP_ICON: &[u8] = include_bytes!("../assets/start.ico");

pub fn app_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(APP_ICON).ok()?.to_rgba8();
    Some(egui::IconData { width: img.width(), height: img.height(), rgba: img.into_raw() })
}

/// Font families for styled lyric text (see `install_fonts`).
pub const FONT_BOLD: &str = "lyric-bold";
pub const FONT_ITALIC: &str = "lyric-italic";
pub const FONT_BOLD_ITALIC: &str = "lyric-bold-italic";

/// JetBrains Mono in four styles for the editor; egui's default fonts stay
/// as fallbacks (emoji, symbols) and for the rest of the UI.
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let faces: [(&str, &'static [u8]); 4] = [
        ("jbm-regular", include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf")),
        ("jbm-bold", include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf")),
        ("jbm-italic", include_bytes!("../assets/fonts/JetBrainsMono-Italic.ttf")),
        ("jbm-bold-italic", include_bytes!("../assets/fonts/JetBrainsMono-BoldItalic.ttf")),
    ];
    for (name, bytes) in faces {
        fonts.font_data.insert(name.into(), Arc::new(FontData::from_static(bytes)));
    }
    let fallbacks = fonts.families.get(&FontFamily::Monospace).cloned().unwrap_or_default();
    let family = |face: &str| std::iter::once(face.to_string()).chain(fallbacks.iter().cloned()).collect::<Vec<_>>();
    fonts.families.insert(FontFamily::Monospace, family("jbm-regular"));
    fonts.families.insert(FontFamily::Name(FONT_BOLD.into()), family("jbm-bold"));
    fonts.families.insert(FontFamily::Name(FONT_ITALIC.into()), family("jbm-italic"));
    fonts.families.insert(FontFamily::Name(FONT_BOLD_ITALIC.into()), family("jbm-bold-italic"));
    ctx.set_fonts(fonts);
}

fn apply_theme(ctx: &egui::Context, dark: bool) {
    let mut visuals = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    if dark {
        visuals.panel_fill = Color32::from_rgb(0x20, 0x23, 0x29);
        visuals.extreme_bg_color = Color32::from_rgb(0x16, 0x18, 0x1d);
        visuals.override_text_color = Some(Color32::from_rgb(0xe6, 0xe6, 0xe6));
        visuals.selection.bg_fill = Color32::from_rgb(0x3d, 0x5a, 0x99);
    } else {
        visuals.extreme_bg_color = Color32::WHITE;
    }
    ctx.set_visuals(visuals);
}

/// Consume a key press whose modifiers match `m` exactly. (egui's own
/// `consume_key` ignores extra Shift, so Ctrl+Shift+N would also fire Ctrl+N.)
pub(crate) fn shortcut(ctx: &egui::Context, m: Modifiers, k: Key) -> bool {
    ctx.input_mut(|i| {
        let before = i.events.len();
        i.events.retain(|e| {
            !matches!(e, egui::Event::Key { key, pressed: true, modifiers, .. } if *key == k && modifiers.matches_exact(m))
        });
        i.events.len() != before
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Home,
    Editor,
    Settings,
}

/// Small modal text prompts.
pub enum Prompt {
    /// A word for Synonyms / Deep Search when nothing is selected.
    Word(Mode, String),
    /// Title for a new lyric file.
    NewLyric(String),
    /// Title for a new `[Section]`.
    Section(String),
}

pub struct LyricApp {
    // The open document.
    pub(crate) text: String,
    pub(crate) path: Option<PathBuf>,
    pub(crate) format: FileFormat,
    pub(crate) dirty: bool,

    pub(crate) settings: Settings,
    settings_path: PathBuf,
    pub(crate) view: View,

    // Editor state (see editor.rs).
    pub(crate) lookups: LookupService,
    pub(crate) pending: Option<Pending>,
    pub(crate) popup: Option<Popup>,
    pub(crate) last_sel: Option<(usize, usize)>,
    pub(crate) sel_changed_at: f64,
    pub(crate) sel_evaluated: bool,
    pub(crate) last_word_signature: Option<(usize, usize, String, Mode)>,
    pub(crate) caret: usize,
    pub(crate) set_cursor: Option<(usize, usize)>,
    pub(crate) pending_state: Option<egui::text_edit::TextEditState>,
    pub(crate) focus_editor: bool,
    pub(crate) counts: HashMap<String, u32>,
    pub(crate) folds: HashSet<(String, usize)>,

    // Home dashboard.
    pub(crate) library: Vec<LyricFile>,
    pub(crate) library_stale: bool,
    pub(crate) library_scanned_at: f64,
    pub(crate) home_search: String,

    pub(crate) prompt: Option<Prompt>,
    /// Cached "is the right-click menu entry installed" (Windows).
    pub(crate) shell_registered: Option<bool>,
    status: Option<(String, f64)>,
    allow_close: bool,
    title: String,
    applied_dark: Option<bool>,
}

impl LyricApp {
    pub fn new(cc: &eframe::CreationContext<'_>, open: Option<PathBuf>) -> Self {
        let ctx = cc.egui_ctx.clone();
        // We handle Ctrl +/- ourselves (editor text size, not whole-UI zoom).
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        install_fonts(&ctx);
        let settings_path = settings_path();
        let settings = Settings::load(&settings_path);
        let repaint_ctx = ctx.clone();
        let mut app = LyricApp {
            text: String::new(),
            path: None,
            format: FileFormat::default(),
            dirty: false,
            settings,
            settings_path,
            view: View::Home,
            lookups: LookupService::new(move || repaint_ctx.request_repaint()),
            pending: None,
            popup: None,
            last_sel: None,
            sel_changed_at: 0.0,
            sel_evaluated: true,
            last_word_signature: None,
            caret: 0,
            set_cursor: None,
            pending_state: None,
            focus_editor: false,
            counts: HashMap::new(),
            folds: HashSet::new(),
            library: Vec::new(),
            library_stale: true,
            library_scanned_at: 0.0,
            home_search: String::new(),
            prompt: None,
            shell_registered: None,
            status: None,
            allow_close: false,
            title: String::new(),
            applied_dark: None,
        };
        if !Mode::ARMED.contains(&app.settings.mode) {
            app.settings.mode = Mode::Report;
        }
        let last = app.settings.last_file.clone().filter(|p| p.exists());
        match (open, app.settings.start_page) {
            (Some(path), _) => {
                app.load(&path);
            }
            (None, StartPage::LastFile) if last.is_some() => {
                app.load(&last.unwrap());
            }
            _ => {}
        }
        app
    }

    // --- small helpers ------------------------------------------------------------

    pub(crate) fn flash(&mut self, ctx: &egui::Context, msg: impl Into<String>) {
        self.status = Some((msg.into(), ctx.input(|i| i.time) + STATUS_MESSAGE_SECS));
    }

    pub(crate) fn save_settings(&self) {
        self.settings.save(&self.settings_path);
    }

    pub(crate) fn settings_file(&self) -> &Path {
        &self.settings_path
    }

    pub(crate) fn display_name(&self) -> String {
        self.path
            .as_deref()
            .and_then(Path::file_name)
            .map_or_else(|| "Untitled".to_string(), |n| n.to_string_lossy().into_owned())
    }

    pub(crate) fn has_document(&self) -> bool {
        self.path.is_some() || !self.text.is_empty()
    }

    // --- files --------------------------------------------------------------------------

    /// Unsaved-changes prompt. Returns false if the user cancelled.
    pub(crate) fn maybe_save(&mut self, ctx: &egui::Context) -> bool {
        if !self.dirty {
            return true;
        }
        let answer = MessageDialog::new()
            .set_level(MessageLevel::Warning)
            .set_title(APP_NAME)
            .set_description(format!("Save changes to {}?", self.display_name()))
            .set_buttons(MessageButtons::YesNoCancel)
            .show();
        match answer {
            MessageDialogResult::Yes => self.save(ctx),
            MessageDialogResult::No => true,
            _ => false,
        }
    }

    fn file_dialog(&self) -> FileDialog {
        let dir = self
            .path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.settings.lyrics_dir());
        FileDialog::new()
            .add_filter("Lyrics / text", &library::EXTENSIONS)
            .add_filter("All files", &["*"])
            .set_directory(dir)
    }

    pub(crate) fn open_dialog(&mut self, ctx: &egui::Context) {
        if !self.maybe_save(ctx) {
            return;
        }
        if let Some(path) = self.file_dialog().pick_file() {
            self.load(&path);
        }
    }

    pub(crate) fn open_path(&mut self, ctx: &egui::Context, path: &Path) {
        if self.path.as_deref() == Some(path) {
            self.view = View::Editor;
            return;
        }
        if self.maybe_save(ctx) {
            self.load(path);
        }
    }

    pub(crate) fn load(&mut self, path: &Path) -> bool {
        match std::fs::read(path) {
            Ok(bytes) => {
                let (text, format) = decode_text(&bytes);
                let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
                self.settings.remember(&path);
                self.save_settings();
                self.replace_document(text, Some(path), format);
                true
            }
            Err(err) => {
                self.settings.recent.retain(|p| p != path);
                self.save_settings();
                MessageDialog::new()
                    .set_level(MessageLevel::Error)
                    .set_title(APP_NAME)
                    .set_description(format!("Couldn't open {}:\n{err}", path.display()))
                    .show();
                false
            }
        }
    }

    pub(crate) fn replace_document(&mut self, text: String, path: Option<PathBuf>, format: FileFormat) {
        self.text = text;
        self.path = path;
        self.format = format;
        self.dirty = false;
        self.popup = None;
        self.pending = None;
        self.last_word_signature = None;
        self.last_sel = None;
        self.folds.clear();
        // A fresh document must not inherit the old one's undo history.
        self.pending_state = Some(egui::text_edit::TextEditState::default());
        self.set_cursor = Some((0, 0));
        self.focus_editor = true;
        self.view = View::Editor;
    }

    /// Create `<lyrics folder>/<title>.txt` from the template and open it.
    pub(crate) fn new_lyric(&mut self, ctx: &egui::Context, title: &str) {
        if !self.maybe_save(ctx) {
            return;
        }
        let dir = self.settings.lyrics_dir();
        let path = library::unique_path(&dir, &library::file_name_for(title));
        let text = library::template(title);
        let result = std::fs::create_dir_all(&dir)
            .and_then(|_| std::fs::write(&path, encode_text(&text, FileFormat::default())));
        match result {
            Ok(()) => {
                self.load(&path);
                let end = self.text.chars().count();
                self.set_cursor = Some((end, end));
                self.library_stale = true;
                self.flash(ctx, format!("Created {}", path.display()));
            }
            Err(err) => {
                MessageDialog::new()
                    .set_level(MessageLevel::Error)
                    .set_title(APP_NAME)
                    .set_description(format!("Couldn't create {}:\n{err}", path.display()))
                    .show();
            }
        }
    }

    pub(crate) fn save(&mut self, ctx: &egui::Context) -> bool {
        match self.path.clone() {
            Some(path) => self.write(ctx, &path),
            None => self.save_as(ctx),
        }
    }

    pub(crate) fn save_as(&mut self, ctx: &egui::Context) -> bool {
        let name = self.path.as_deref().and_then(Path::file_name).map_or_else(
            || format!("{}.txt", library::file_name_for(&library::summarize(&self.text, "Untitled").title)),
            |n| n.to_string_lossy().into_owned(),
        );
        let Some(mut path) = self.file_dialog().set_file_name(name).save_file() else { return false };
        if path.extension().is_none() {
            path.set_extension("txt");
        }
        self.write(ctx, &path)
    }

    fn write(&mut self, ctx: &egui::Context, path: &Path) -> bool {
        let data = encode_text(&self.text, self.format);
        // Write-then-rename so a crash mid-save can't truncate the lyrics.
        let tmp = path.with_extension("lyrictoolsnotepad-tmp");
        let result = std::fs::write(&tmp, &data).and_then(|_| std::fs::rename(&tmp, path));
        if let Err(err) = result {
            let _ = std::fs::remove_file(&tmp);
            MessageDialog::new()
                .set_level(MessageLevel::Error)
                .set_title(APP_NAME)
                .set_description(format!("Couldn't save {}:\n{err}", path.display()))
                .show();
            return false;
        }
        let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        self.settings.remember(&path);
        self.save_settings();
        self.flash(ctx, format!("Saved {}", path.display()));
        self.path = Some(path);
        self.dirty = false;
        self.library_stale = true;
        true
    }

    // --- chrome ----------------------------------------------------------------------

    fn handle_global_shortcuts(&mut self, ctx: &egui::Context) {
        let cmd = Modifiers::COMMAND;
        let pressed = |m: Modifiers, k: Key| shortcut(ctx, m, k);
        if pressed(cmd | Modifiers::SHIFT, Key::S) {
            self.save_as(ctx);
        }
        if pressed(cmd, Key::S) {
            self.save(ctx);
        }
        if pressed(cmd, Key::N) {
            self.prompt = Some(Prompt::NewLyric(String::new()));
        }
        if pressed(cmd, Key::O) {
            self.open_dialog(ctx);
        }
        if pressed(cmd, Key::Q) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.add(egui::Button::new("New Lyric...").shortcut_text("Ctrl+N")).clicked() {
                    self.prompt = Some(Prompt::NewLyric(String::new()));
                }
                if ui.add(egui::Button::new("Open...").shortcut_text("Ctrl+O")).clicked() {
                    self.open_dialog(&ctx);
                }
                ui.menu_button("Open Recent", |ui| {
                    let recent = self.settings.recent.clone();
                    if recent.is_empty() {
                        ui.label("(none)");
                    }
                    for path in recent {
                        if ui.button(path.display().to_string()).clicked() {
                            self.open_path(&ctx, &path);
                        }
                    }
                    if !self.settings.recent.is_empty() {
                        ui.separator();
                        if ui.button("Clear List").clicked() {
                            self.settings.recent.clear();
                            self.save_settings();
                        }
                    }
                });
                ui.separator();
                if ui.add(egui::Button::new("Save").shortcut_text("Ctrl+S")).clicked() {
                    self.save(&ctx);
                }
                if ui.add(egui::Button::new("Save As...").shortcut_text("Ctrl+Shift+S")).clicked() {
                    self.save_as(&ctx);
                }
                ui.separator();
                if ui.add(egui::Button::new("Exit").shortcut_text("Ctrl+Q")).clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            if self.view == View::Editor {
                self.editor_menus(ui);
            }
            ui.menu_button("Help", |ui| {
                ui.label(format!("{APP_NAME} {}", env!("CARGO_PKG_VERSION")));
                ui.label(
                    "Highlight a word for rhymes. Rhymes and syllables are offline\n\
                     (CMU Pronouncing Dictionary); Synonyms and Deep Search use\n\
                     the free Datamuse API.",
                );
                ui.label("MIT licensed. Built with Rust + egui.");
            });

            // View tabs, right-aligned.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                for (view, label) in
                    [(View::Settings, "⚙ Settings"), (View::Editor, "✏ Editor"), (View::Home, "🏠 Home")]
                {
                    let label = if view == View::Editor && self.has_document() {
                        format!("✏ {}{}", self.display_name(), if self.dirty { " •" } else { "" })
                    } else {
                        label.to_string()
                    };
                    if ui.add(egui::Button::selectable(self.view == view, label)).clicked() {
                        self.view = view;
                        if view == View::Home {
                            self.library_stale = true;
                        }
                        if view == View::Editor {
                            self.focus_editor = true;
                        }
                    }
                }
            });
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let now = ui.input(|i| i.time);
        ui.horizontal(|ui| {
            if let Some((msg, until)) = &self.status
                && now < *until
            {
                ui.label(msg);
                ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(until - now));
            }
            if self.view == View::Editor {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| self.editor_status(ui));
            }
        });
    }

    fn prompts(&mut self, ctx: &egui::Context) {
        let Some(prompt) = self.prompt.take() else { return };
        let (title, label, button, mut value) = match &prompt {
            Prompt::Word(mode, v) => (mode.title(), "Word:", "Search", v.clone()),
            Prompt::NewLyric(v) => ("New Lyric", "Song title:", "Create", v.clone()),
            Prompt::Section(v) => ("New Section", "Section name (e.g. Chorus, Verse 2):", "Add", v.clone()),
        };
        let mut submit = false;
        let mut cancel = false;
        let modal = egui::Modal::new(egui::Id::unique("prompt")).show(ctx, |ui| {
            ui.set_width(320.0);
            ui.heading(title);
            ui.label(label);
            // Catch Enter before the text field does (it would just drop focus).
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
                submit = true;
            }
            ui.add(egui::TextEdit::singleline(&mut value).desired_width(f32::INFINITY)).request_focus();
            if let Prompt::NewLyric(_) = prompt {
                ui.label(
                    egui::RichText::new(format!("Saved in {}", self.settings.lyrics_dir().display())).small().weak(),
                );
            }
            ui.horizontal(|ui| {
                submit |= ui.button(button).clicked();
                cancel |= ui.button("Cancel").clicked();
            });
        });
        if modal.should_close() {
            cancel = true;
        }
        let trimmed = value.trim().to_string();
        if submit && !trimmed.is_empty() {
            match prompt {
                Prompt::Word(mode, _) => {
                    let caret = self.caret;
                    self.request(ctx, &trimmed, caret, caret, mode);
                }
                Prompt::NewLyric(_) => self.new_lyric(ctx, &trimmed),
                Prompt::Section(_) => self.insert_section(&trimmed),
            }
        } else if !cancel {
            self.prompt = Some(match prompt {
                Prompt::Word(mode, _) => Prompt::Word(mode, value),
                Prompt::NewLyric(_) => Prompt::NewLyric(value),
                Prompt::Section(_) => Prompt::Section(value),
            });
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = match self.view {
            View::Editor => format!("{}{} \u{2014} {APP_NAME}", if self.dirty { "*" } else { "" }, self.display_name()),
            View::Home => format!("Home \u{2014} {APP_NAME}"),
            View::Settings => format!("Settings \u{2014} {APP_NAME}"),
        };
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    fn handle_close(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) || self.allow_close {
            return;
        }
        if self.maybe_save(ctx) {
            self.allow_close = true;
            if let Some(rect) = ctx.input(|i| i.viewport().inner_rect) {
                self.settings.window_size = Some([rect.width(), rect.height()]);
            }
            self.save_settings();
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
    }
}

impl eframe::App for LyricApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.applied_dark != Some(self.settings.dark) {
            apply_theme(&ctx, self.settings.dark);
            self.applied_dark = Some(self.settings.dark);
        }

        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect()
        });
        if let Some(path) = dropped.first() {
            self.open_path(&ctx, path);
        }

        self.poll_lookups(&ctx);
        if self.prompt.is_none() {
            self.handle_global_shortcuts(&ctx);
            if self.view == View::Editor {
                self.handle_editor_shortcuts(&ctx);
            }
        }

        egui::Panel::top("menu").show(ui, |ui| {
            self.menu_bar(ui);
            if self.view == View::Editor {
                ui.add_space(2.0);
                self.editor_toolbar(ui);
                ui.add_space(2.0);
            }
        });
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        let bg = ctx.global_style().visuals.extreme_bg_color;
        match self.view {
            View::Editor => {
                egui::CentralPanel::default().frame(egui::Frame::NONE.fill(bg)).show(ui, |ui| self.editor(ui));
            }
            View::Home => {
                egui::CentralPanel::default().show(ui, |ui| self.home(ui));
            }
            View::Settings => {
                egui::CentralPanel::default().show(ui, |ui| self.settings_page(ui));
            }
        }

        self.prompts(&ctx);
        self.update_title(&ctx);
        self.handle_close(&ctx);
    }
}
