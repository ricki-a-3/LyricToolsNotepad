//! The main window: editor + syllable gutter, rhyme dropdown, files, menus.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::{
    self, Align2, Color32, FontId, Id, Key, Margin, Modifiers, Order, Pos2, Rect, Sense, TextureHandle, Vec2,
    text::CCursor, text_selection::CCursorRange,
};
use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};

use crate::core::{self, WordResult};
use crate::lookup::{Groups, LookupService, Mode};
use crate::settings::{Settings, settings_path};
use crate::textutil::{
    FileFormat, char_slice, color_for_count, decode_text, encode_text, is_whole_word_selection, line_at, match_case,
    word_span_at,
};

pub const APP_NAME: &str = "LyricTools++";
const EDITOR_ID: &str = "lyric_editor";
/// Wait this long after the selection stops changing (and the mouse is up)
/// before treating it as "a word was highlighted".
const SELECTION_SETTLE_SECS: f64 = 0.18;
const POPUP_WIDTH: f32 = 260.0;
const POPUP_MAX_HEIGHT: f32 = 300.0;
const STATUS_MESSAGE_SECS: f64 = 5.0;

const ICONS: [(&str, &[u8]); 6] = [
    ("start", include_bytes!("../assets/start.ico")),
    ("rhymes", include_bytes!("../assets/rhymes.ico")),
    ("nearrhymes", include_bytes!("../assets/nearrhymes.ico")),
    ("rhymereport", include_bytes!("../assets/rhymereport.ico")),
    ("synonyms", include_bytes!("../assets/synonyms.ico")),
    ("deepsearch", include_bytes!("../assets/deepsearch.ico")),
];

pub fn decode_icon(bytes: &[u8]) -> Option<egui::ColorImage> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()))
}

pub fn app_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(ICONS[0].1).ok()?.to_rgba8();
    Some(egui::IconData { width: img.width(), height: img.height(), rgba: img.into_raw() })
}

enum PopupRow {
    Header(&'static str),
    Word(WordResult),
}

struct Popup {
    rows: Vec<PopupRow>,
    /// Index into `rows` of the highlighted word.
    selected: usize,
    /// Chars `start..end` the pick replaces (equal = insert at caret).
    start: usize,
    end: usize,
    word: String,
    scroll_to_selected: bool,
}

impl Popup {
    fn new(groups: Groups, show_headers: bool, start: usize, end: usize, word: String) -> Option<Self> {
        let mut rows = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for (heading, results) in groups {
            let fresh: Vec<WordResult> = results.into_iter().filter(|r| seen.insert(r.word.to_lowercase())).collect();
            if fresh.is_empty() {
                continue;
            }
            if show_headers {
                rows.push(PopupRow::Header(heading));
            }
            rows.extend(fresh.into_iter().map(PopupRow::Word));
        }
        let selected = rows.iter().position(|r| matches!(r, PopupRow::Word(_)))?;
        Some(Popup { rows, selected, start, end, word, scroll_to_selected: true })
    }

    fn word_count(&self) -> usize {
        self.rows.iter().filter(|r| matches!(r, PopupRow::Word(_))).count()
    }

    fn step(&mut self, delta: isize) {
        let words: Vec<usize> =
            self.rows.iter().enumerate().filter(|(_, r)| matches!(r, PopupRow::Word(_))).map(|(i, _)| i).collect();
        let pos = words.iter().position(|&i| i == self.selected).unwrap_or(0) as isize;
        let next = (pos + delta).clamp(0, words.len() as isize - 1) as usize;
        self.selected = words[next];
        self.scroll_to_selected = true;
    }

    fn selected_word(&self) -> Option<&str> {
        match self.rows.get(self.selected) {
            Some(PopupRow::Word(r)) => Some(&r.word),
            _ => None,
        }
    }
}

struct Pending {
    id: u64,
    start: usize,
    end: usize,
}

pub struct LyricApp {
    text: String,
    path: Option<PathBuf>,
    format: FileFormat,
    dirty: bool,

    settings: Settings,
    settings_path: PathBuf,
    lookups: LookupService,
    pending: Option<Pending>,
    popup: Option<Popup>,

    /// Selection tracking for the highlight-to-rhyme trigger.
    last_sel: Option<(usize, usize)>,
    sel_changed_at: f64,
    sel_evaluated: bool,
    last_word_signature: Option<(usize, usize, String, Mode)>,
    caret: usize,
    /// Set to move the caret (applied before the editor is drawn).
    set_cursor: Option<CCursorRange>,
    /// Replaces the editor's whole state (cursor + undo history) on load/new.
    pending_state: Option<egui::text_edit::TextEditState>,

    counts: HashMap<String, u32>,
    icons: HashMap<&'static str, TextureHandle>,
    status: Option<(String, f64)>,
    prompt: Option<(Mode, String)>,
    allow_close: bool,
    title: String,
    applied_dark: Option<bool>,
}

impl LyricApp {
    pub fn new(cc: &eframe::CreationContext<'_>, open: Option<PathBuf>) -> Self {
        let ctx = cc.egui_ctx.clone();
        // We handle Ctrl +/- ourselves (editor font size, not whole-UI zoom).
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let icons = ICONS
            .iter()
            .filter_map(|(name, bytes)| {
                decode_icon(bytes).map(|img| (*name, ctx.load_texture(*name, img, egui::TextureOptions::LINEAR)))
            })
            .collect();
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
            counts: HashMap::new(),
            icons,
            status: None,
            prompt: None,
            allow_close: false,
            title: String::new(),
            applied_dark: None,
        };
        if !Mode::ARMED.contains(&app.settings.mode) {
            app.settings.mode = Mode::Report;
        }
        if let Some(path) = open {
            app.load(&path);
        }
        app
    }

    // --- status ---------------------------------------------------------------

    fn flash(&mut self, ctx: &egui::Context, msg: impl Into<String>) {
        self.status = Some((msg.into(), ctx.input(|i| i.time) + STATUS_MESSAGE_SECS));
    }

    fn save_settings(&self) {
        self.settings.save(&self.settings_path);
    }

    fn line_syllables(&mut self, line: &str) -> u32 {
        if let Some(&n) = self.counts.get(line) {
            return n;
        }
        let n = if line.trim().is_empty() { 0 } else { core::syllables_for_line(line) };
        if self.counts.len() > 20_000 {
            self.counts.clear();
        }
        self.counts.insert(line.to_string(), n);
        n
    }

    // --- files ------------------------------------------------------------------

    /// Unsaved-changes prompt. Returns false if the user cancelled.
    fn maybe_save(&mut self, ctx: &egui::Context) -> bool {
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
        let mut dialog =
            FileDialog::new().add_filter("Lyrics / text", &["txt", "lyrics", "md"]).add_filter("All files", &["*"]);
        let dir = self.path.as_deref().and_then(Path::parent).map(Path::to_path_buf).or(self.settings.last_dir.clone());
        if let Some(dir) = dir {
            dialog = dialog.set_directory(dir);
        }
        dialog
    }

    fn new_file(&mut self, ctx: &egui::Context) {
        if self.maybe_save(ctx) {
            self.replace_document(String::new(), None, FileFormat::default());
        }
    }

    fn open_dialog(&mut self, ctx: &egui::Context) {
        if !self.maybe_save(ctx) {
            return;
        }
        if let Some(path) = self.file_dialog().pick_file() {
            self.load(&path);
        }
    }

    fn open_path(&mut self, ctx: &egui::Context, path: &Path) {
        if self.maybe_save(ctx) && !self.load(path) {
            self.flash(ctx, format!("Couldn't open {}", path.display()));
        }
    }

    fn load(&mut self, path: &Path) -> bool {
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

    fn replace_document(&mut self, text: String, path: Option<PathBuf>, format: FileFormat) {
        self.text = text;
        self.path = path;
        self.format = format;
        self.dirty = false;
        self.close_popup();
        self.last_word_signature = None;
        // A fresh document must not inherit the old one's undo history.
        let mut state = egui::text_edit::TextEditState::default();
        state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(0))));
        self.pending_state = Some(state);
    }

    fn save(&mut self, ctx: &egui::Context) -> bool {
        match self.path.clone() {
            Some(path) => self.write(ctx, &path),
            None => self.save_as(ctx),
        }
    }

    fn save_as(&mut self, ctx: &egui::Context) -> bool {
        let name = self
            .path
            .as_deref()
            .and_then(Path::file_name)
            .map_or("lyrics.txt".into(), |n| n.to_string_lossy().into_owned());
        let Some(mut path) = self.file_dialog().set_file_name(name).save_file() else { return false };
        if path.extension().is_none() {
            path.set_extension("txt");
        }
        self.write(ctx, &path)
    }

    fn write(&mut self, ctx: &egui::Context, path: &Path) -> bool {
        let data = encode_text(&self.text, self.format);
        // Write-then-rename so a crash mid-save can't truncate the lyrics.
        let tmp = path.with_extension("lyrictools-tmp");
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
        true
    }

    fn display_name(&self) -> String {
        self.path
            .as_deref()
            .and_then(Path::file_name)
            .map_or_else(|| "Untitled".to_string(), |n| n.to_string_lossy().into_owned())
    }

    // --- modes & lookups -----------------------------------------------------------

    fn set_mode(&mut self, ctx: &egui::Context, mode: Mode) {
        self.settings.mode = mode;
        self.save_settings();
        self.flash(ctx, format!("Highlight a word for {}", mode.title().to_lowercase()));
        // Already sitting on a whole word? Show it in the new mode right away.
        self.last_word_signature = None;
        self.sel_evaluated = false;
    }

    fn request(&mut self, ctx: &egui::Context, word: &str, start: usize, end: usize, mode: Mode) {
        self.close_popup();
        let id = self.lookups.request(word, mode);
        self.pending = Some(Pending { id, start, end });
        if mode.is_network() {
            self.flash(ctx, format!("{} for \u{201C}{word}\u{201D}: searching online...", mode.title()));
        }
    }

    fn cancel_lookup(&mut self) {
        self.lookups.cancel();
        self.pending = None;
        self.close_popup();
    }

    fn close_popup(&mut self) {
        self.popup = None;
    }

    fn poll_lookups(&mut self, ctx: &egui::Context) {
        let Some(done) = self.lookups.poll() else { return };
        let Some(pending) = self.pending.take().filter(|p| p.id == done.id) else { return };
        let (mode, word) = (done.mode, done.word);
        let groups = match done.result {
            Ok(groups) => groups,
            Err(err) => {
                self.flash(
                    ctx,
                    format!("{} for \u{201C}{word}\u{201D} failed ({err}). Offline rhymes still work.", mode.title()),
                );
                return;
            }
        };
        // The text may have changed while we waited; only anchor the popup
        // if the word is still exactly where it was.
        if pending.start != pending.end && char_slice(&self.text, pending.start, pending.end) != word {
            return;
        }
        match Popup::new(groups, mode == Mode::Report, pending.start, pending.end, word.clone()) {
            Some(popup) => {
                if mode.is_network() {
                    self.flash(
                        ctx,
                        format!("{} for \u{201C}{word}\u{201D}: {} found", mode.title(), popup.word_count()),
                    );
                }
                self.popup = Some(popup);
            }
            None if mode.is_network() => {
                self.flash(ctx, format!("No {} found for \u{201C}{word}\u{201D}", mode.title().to_lowercase()))
            }
            None => {}
        }
    }

    fn pick(&mut self, ctx: &egui::Context, choice: String) {
        let Some(popup) = self.popup.take() else { return };
        let (start, end) = (popup.start, popup.end);
        if start != end && char_slice(&self.text, start, end) != popup.word {
            return;
        }
        let replacement = if start != end { match_case(&popup.word, &choice) } else { choice };
        let (b0, b1) = (crate::textutil::byte_index(&self.text, start), crate::textutil::byte_index(&self.text, end));
        self.text.replace_range(b0..b1, &replacement);
        self.dirty = true;
        let caret = start + replacement.chars().count();
        self.set_cursor = Some(CCursorRange::one(CCursor::new(caret)));
        self.last_word_signature = None;
        ctx.memory_mut(|m| m.request_focus(Id::unique(EDITOR_ID)));
    }

    /// Selection if any, else the word at the caret.
    fn word_for_lookup(&self) -> Option<(String, usize, usize)> {
        if let Some((start, end)) = self.last_sel.filter(|(s, e)| s != e) {
            let text = char_slice(&self.text, start, end);
            let trimmed = text.trim();
            return (!trimmed.is_empty() && !text.contains('\n')).then(|| (trimmed.to_string(), start, end));
        }
        let (_, line_start, line) = line_at(&self.text, self.caret);
        let (s, e) = word_span_at(line, self.caret - line_start)?;
        let word: String = line.chars().skip(s).take(e - s).collect();
        Some((word, line_start + s, line_start + e))
    }

    /// Synonyms / Deep Search: run once for the selection, the word at the
    /// caret, or a typed word. Never armed for future highlights.
    fn one_shot(&mut self, ctx: &egui::Context, mode: Mode) {
        match self.word_for_lookup() {
            Some((word, start, end)) => self.request(ctx, &word, start, end, mode),
            None => self.prompt = Some((mode, String::new())),
        }
    }

    fn rhyme_at_caret(&mut self, ctx: &egui::Context) {
        if let Some((word, start, end)) = self.word_for_lookup() {
            let mode = self.settings.mode;
            self.request(ctx, &word, start, end, mode);
        }
    }

    /// Runs once the selection has settled.
    fn evaluate_selection(&mut self, ctx: &egui::Context) {
        let mode = self.settings.mode;
        let picked = self.last_sel.filter(|(s, e)| s != e).and_then(|(start, end)| {
            let (_, line_start, line) = line_at(&self.text, start);
            let len = line.chars().count();
            if end - line_start > len || !is_whole_word_selection(line, start - line_start, end - line_start) {
                return None;
            }
            Some((start, end, char_slice(&self.text, start, end).to_string(), mode))
        });
        match picked {
            None => {
                if self.last_word_signature.take().is_some() {
                    self.cancel_lookup();
                }
            }
            Some(sig) if Some(&sig) != self.last_word_signature.as_ref() => {
                let (start, end, word, mode) = sig.clone();
                self.last_word_signature = Some(sig);
                self.request(ctx, &word, start, end, mode);
            }
            Some(_) => {}
        }
    }
}

// --- drawing ------------------------------------------------------------------------

fn gutter_bg(dark: bool) -> Color32 {
    if dark { Color32::from_rgb(0x1b, 0x1d, 0x23) } else { Color32::from_rgb(0xf0, 0xf0, 0xf0) }
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

impl LyricApp {
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let cmd = Modifiers::COMMAND;
        let shift_cmd = Modifiers::COMMAND | Modifiers::SHIFT;
        let pressed = |m: Modifiers, k: Key| ctx.input_mut(|i| i.consume_key(m, k));

        // Dropdown navigation first, so the editor never sees these keys.
        if self.popup.is_some() {
            let popup = self.popup.as_mut().unwrap();
            if pressed(Modifiers::NONE, Key::ArrowDown) {
                popup.step(1);
            }
            if pressed(Modifiers::NONE, Key::ArrowUp) {
                popup.step(-1);
            }
            if pressed(Modifiers::NONE, Key::PageDown) {
                popup.step(10);
            }
            if pressed(Modifiers::NONE, Key::PageUp) {
                popup.step(-10);
            }
            if pressed(Modifiers::NONE, Key::Escape) {
                self.close_popup();
            } else if (pressed(Modifiers::NONE, Key::Enter) || pressed(Modifiers::NONE, Key::Tab))
                && let Some(word) = popup.selected_word().map(str::to_string)
            {
                self.pick(ctx, word);
            }
        }

        if pressed(shift_cmd, Key::S) {
            self.save_as(ctx);
        }
        if pressed(cmd, Key::S) {
            self.save(ctx);
        }
        if pressed(cmd, Key::N) {
            self.new_file(ctx);
        }
        if pressed(cmd, Key::O) {
            self.open_dialog(ctx);
        }
        if pressed(cmd, Key::Q) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        for (key, mode) in [(Key::Num1, Mode::Perfect), (Key::Num2, Mode::Near), (Key::Num3, Mode::Report)] {
            if pressed(cmd, key) {
                self.set_mode(ctx, mode);
            }
        }
        if pressed(Modifiers::NONE, Key::F6) {
            self.rhyme_at_caret(ctx);
        }
        if pressed(Modifiers::NONE, Key::F7) {
            self.one_shot(ctx, Mode::Synonyms);
        }
        if pressed(Modifiers::NONE, Key::F8) {
            self.one_shot(ctx, Mode::Deep);
        }
        if pressed(cmd, Key::Equals) || pressed(cmd, Key::Plus) {
            self.zoom(1.0);
        }
        if pressed(cmd, Key::Minus) {
            self.zoom(-1.0);
        }
        if pressed(cmd, Key::Num0) {
            self.zoom(0.0);
        }
    }

    fn zoom(&mut self, delta: f32) {
        self.settings.font_size = if delta == 0.0 {
            Settings::default().font_size
        } else {
            (self.settings.font_size + delta).clamp(8.0, 48.0)
        };
        self.save_settings();
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.add(egui::Button::new("New").shortcut_text("Ctrl+N")).clicked() { self.new_file(&ctx); }
                if ui.add(egui::Button::new("Open...").shortcut_text("Ctrl+O")).clicked() { self.open_dialog(&ctx); }
                ui.menu_button("Open Recent", |ui| {
                    let recent = self.settings.recent.clone();
                    if recent.is_empty() {
                        ui.label("(none)");
                    }
                    for path in recent {
                        if ui.button(path.display().to_string()).clicked() { self.open_path(&ctx, &path); }
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
                if ui.add(egui::Button::new("Save").shortcut_text("Ctrl+S")).clicked() { self.save(&ctx); }
                if ui.add(egui::Button::new("Save As...").shortcut_text("Ctrl+Shift+S")).clicked() { self.save_as(&ctx); }
                ui.separator();
                if ui.add(egui::Button::new("Exit").shortcut_text("Ctrl+Q")).clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Rhymes", |ui| {
                for (mode, label, key) in [
                    (Mode::Perfect, "Perfect Rhymes", "Ctrl+1"),
                    (Mode::Near, "Near Rhymes", "Ctrl+2"),
                    (Mode::Report, "Rhyme Report (Perfect + Near)", "Ctrl+3"),
                ] {
                    let on = self.settings.mode == mode;
                    if ui.add(egui::Button::selectable(on, label).shortcut_text(key)).clicked() { self.set_mode(&ctx, mode); }
                }
                ui.separator();
                if ui.add(egui::Button::new("Synonyms (online)").shortcut_text("F7")).clicked() { self.one_shot(&ctx, Mode::Synonyms); }
                if ui.add(egui::Button::new("Deep Search (online)").shortcut_text("F8")).clicked() { self.one_shot(&ctx, Mode::Deep); }
                if ui.add(egui::Button::new("Rhymes for Word at Caret").shortcut_text("F6")).clicked() { self.rhyme_at_caret(&ctx); }
            });
            ui.menu_button("View", |ui| {
                if ui.add(egui::Button::new("Bigger Text").shortcut_text("Ctrl+=")).clicked() { self.zoom(1.0); }
                if ui.add(egui::Button::new("Smaller Text").shortcut_text("Ctrl+-")).clicked() { self.zoom(-1.0); }
                if ui.add(egui::Button::new("Reset Text Size").shortcut_text("Ctrl+0")).clicked() { self.zoom(0.0); }
                ui.separator();
                if ui.checkbox(&mut self.settings.wrap, "Word Wrap").changed() { self.save_settings(); }
                if ui.checkbox(&mut self.settings.dark, "Dark Theme").changed() { self.save_settings(); }
            });
            ui.menu_button("Help", |ui| {
                ui.label(format!("{APP_NAME} {}", env!("CARGO_PKG_VERSION")));
                ui.label("Highlight a word for rhymes. Rhymes and syllables are offline\n(CMU Pronouncing Dictionary); Synonyms and Deep Search use\nthe free Datamuse API.");
                ui.label("MIT licensed. Built with Rust + egui.");
            });
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let icons = self.icons.clone();
        ui.horizontal(|ui| {
            let tool = |ui: &mut egui::Ui, icon: &str, on: bool, tip: &str| -> bool {
                let size = Vec2::splat(22.0);
                let button = match icons.get(icon) {
                    Some(tex) => egui::Button::image(egui::Image::new((tex.id(), size))),
                    None => egui::Button::new(&icon[..1]),
                };
                ui.add(button.selected(on).min_size(Vec2::splat(30.0))).on_hover_text(tip).clicked()
            };
            let mode = self.settings.mode;
            if tool(ui, "rhymes", mode == Mode::Perfect, "Perfect rhymes (Ctrl+1)") {
                self.set_mode(&ctx, Mode::Perfect);
            }
            if tool(ui, "nearrhymes", mode == Mode::Near, "Near / slant rhymes (Ctrl+2)") {
                self.set_mode(&ctx, Mode::Near);
            }
            if tool(ui, "rhymereport", mode == Mode::Report, "Perfect + near rhymes (Ctrl+3)") {
                self.set_mode(&ctx, Mode::Report);
            }
            ui.separator();
            if tool(ui, "synonyms", false, "Synonyms for the selected word, online (F7)") {
                self.one_shot(&ctx, Mode::Synonyms);
            }
            if tool(ui, "deepsearch", false, "Deep Search: offline + Datamuse rhymes, online (F8)") {
                self.one_shot(&ctx, Mode::Deep);
            }
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let now = ui.input(|i| i.time);
        let (line_no, line_start, line) = line_at(&self.text, self.caret);
        let col = self.caret - line_start + 1;
        let line = line.to_string();
        let syl = if core::is_section_header(&line) { 0 } else { self.line_syllables(&line) };
        ui.horizontal(|ui| {
            if let Some((msg, until)) = &self.status
                && now < *until
            {
                ui.label(msg);
                ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(until - now));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("Mode: {}", self.settings.mode.title()));
                ui.separator();
                ui.label(format!("{syl} syllable{}", if syl == 1 { "" } else { "s" }));
                ui.separator();
                ui.label(format!("Ln {}, Col {col}", line_no + 1));
            });
        });
    }

    fn editor(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let id = Id::unique(EDITOR_ID);
        if let Some(state) = self.pending_state.take() {
            state.store(&ctx, id);
        }
        if let Some(range) = self.set_cursor.take() {
            let mut state = egui::text_edit::TextEditState::load(&ctx, id).unwrap_or_default();
            state.cursor.set_char_range(Some(range));
            state.store(&ctx, id);
        }

        let font = FontId::monospace(self.settings.font_size);
        let digit_w = ctx.fonts_mut(|f| f.glyph_width(&font, '8'));
        let gutter_w = (digit_w * 3.0 + 18.0).min(120.0);
        let wrap = self.settings.wrap;
        let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap_width: f32| {
            let color = ui.visuals().text_color();
            let job = egui::text::LayoutJob::simple(
                buf.as_str().to_owned(),
                FontId::monospace(self.settings.font_size),
                color,
                if wrap { wrap_width } else { f32::INFINITY },
            );
            ui.fonts_mut(|f| f.layout_job(job))
        };

        let scroll = if wrap { egui::ScrollArea::vertical() } else { egui::ScrollArea::both() };
        let output = scroll
            .id_salt("editor_scroll")
            .auto_shrink(false)
            .show(ui, |ui| {
                egui::TextEdit::multiline(&mut self.text)
                    .id(id)
                    .font(font.clone())
                    .frame(egui::Frame::NONE.inner_margin(Margin { left: gutter_w as i8, right: 8, top: 6, bottom: 6 }))
                    .desired_width(f32::INFINITY)
                    .min_size(ui.available_size())
                    .lock_focus(true)
                    .layouter(&mut layouter)
                    .show(ui)
            })
            .inner;

        if output.response.changed() {
            self.dirty = true;
            self.close_popup();
        }
        let sel = output.cursor_range.map(|r| {
            let range = r.as_sorted_char_range();
            (range.start.0, range.end.0)
        });
        if let Some(r) = output.cursor_range {
            self.caret = r.primary.index.0;
        }

        // Syllable gutter: one colored count per line (not per wrapped row).
        let galley = Arc::clone(&output.galley);
        let clip = ui.clip_rect();
        let painter = ui.painter().with_clip_rect(clip);
        let left = output.response.rect.left();
        painter.rect_filled(
            Rect::from_x_y_ranges(left..=left + gutter_w - 6.0, clip.y_range()),
            0.0,
            gutter_bg(self.settings.dark),
        );
        let lines: Vec<String> = self.text.split('\n').map(str::to_string).collect();
        let mut line_idx = 0;
        let mut at_line_start = true;
        let bold = FontId::monospace(self.settings.font_size * 0.92);
        for row in &galley.rows {
            if at_line_start {
                let rect = row.rect().translate(output.galley_pos.to_vec2());
                if rect.bottom() >= clip.top() && rect.top() <= clip.bottom() {
                    let text = lines.get(line_idx).map(String::as_str).unwrap_or("");
                    let count = if core::is_section_header(text) { 0 } else { self.line_syllables(text) };
                    if let Some((r, g, b)) = color_for_count(count) {
                        let pos = Pos2::new(left + gutter_w - 14.0, rect.center().y);
                        let color = Color32::from_rgb(r, g, b);
                        painter.text(pos, Align2::RIGHT_CENTER, count.to_string(), bold.clone(), color);
                        // Fake bold: a second pass offset by one pixel.
                        painter.text(
                            pos + Vec2::new(0.6, 0.0),
                            Align2::RIGHT_CENTER,
                            count.to_string(),
                            bold.clone(),
                            color,
                        );
                    }
                }
            }
            at_line_start = row.ends_with_newline;
            if row.ends_with_newline {
                line_idx += 1;
            }
        }

        // Highlight-to-rhyme trigger: act once the selection has settled.
        let now = ctx.input(|i| i.time);
        if sel != self.last_sel {
            self.last_sel = sel;
            self.sel_changed_at = now;
            self.sel_evaluated = false;
            if let Some(p) = &self.popup
                && sel != Some((p.start, p.end))
                && !(p.start == p.end && sel.is_some_and(|(a, b)| a == b && a == p.start))
            {
                self.close_popup();
            }
        }
        if !self.sel_evaluated {
            let dragging = ctx.input(|i| i.pointer.any_down());
            if !dragging && now - self.sel_changed_at >= SELECTION_SETTLE_SECS {
                self.sel_evaluated = true;
                self.evaluate_selection(&ctx);
            } else {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(SELECTION_SETTLE_SECS));
            }
        }

        // Dropdown, anchored under the word.
        if let Some(popup) = &self.popup {
            let anchor = galley.pos_from_cursor(CCursor::new(popup.start)).translate(output.galley_pos.to_vec2());
            self.show_popup(&ctx, Pos2::new(anchor.left(), anchor.bottom() + 2.0));
        }
    }

    fn show_popup(&mut self, ctx: &egui::Context, pos: Pos2) {
        let dark = self.settings.dark;
        let mut chosen = None;
        let Some(popup) = self.popup.as_mut() else { return };
        let area = egui::Area::new(Id::unique("rhyme_popup"))
            .order(Order::Foreground)
            .fixed_pos(pos)
            .constrain(true)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).inner_margin(4).show(ui, |ui| {
                    ui.set_width(POPUP_WIDTH);
                    egui::ScrollArea::vertical().max_height(POPUP_MAX_HEIGHT).show(ui, |ui| {
                        let row_h = ui.text_style_height(&egui::TextStyle::Body) + 6.0;
                        for (i, row) in popup.rows.iter().enumerate() {
                            match row {
                                PopupRow::Header(h) => {
                                    ui.add_space(2.0);
                                    ui.label(egui::RichText::new(h.to_uppercase()).small().strong().weak());
                                }
                                PopupRow::Word(r) => {
                                    let (rect, resp) =
                                        ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());
                                    if i == popup.selected || resp.hovered() {
                                        let fill = if i == popup.selected {
                                            ui.visuals().selection.bg_fill
                                        } else {
                                            ui.visuals().widgets.hovered.weak_bg_fill
                                        };
                                        ui.painter().rect_filled(rect, 3.0, fill);
                                    }
                                    let text_color = ui.visuals().text_color();
                                    let body = egui::TextStyle::Body.resolve(ui.style());
                                    ui.painter().text(
                                        rect.left_center() + Vec2::new(8.0, 0.0),
                                        Align2::LEFT_CENTER,
                                        &r.word,
                                        body.clone(),
                                        text_color,
                                    );
                                    if let Some((cr, cg, cb)) = color_for_count(r.syllables) {
                                        let mut c = Color32::from_rgb(cr, cg, cb);
                                        if !dark {
                                            c = c.gamma_multiply(0.85);
                                        }
                                        ui.painter().text(
                                            rect.right_center() - Vec2::new(10.0, 0.0),
                                            Align2::RIGHT_CENTER,
                                            r.syllables.to_string(),
                                            body,
                                            c,
                                        );
                                    }
                                    if i == popup.selected && popup.scroll_to_selected {
                                        ui.scroll_to_rect(rect, None);
                                    }
                                    if resp.clicked() {
                                        chosen = Some(r.word.clone());
                                    }
                                }
                            }
                        }
                    });
                });
            });
        popup.scroll_to_selected = false;
        if let Some(word) = chosen {
            self.pick(ctx, word);
            return;
        }
        // Click anywhere else closes it, leaving the text untouched.
        let clicked_outside = ctx.input(|i| {
            i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p))
        });
        if clicked_outside {
            self.close_popup();
        }
    }

    fn word_prompt(&mut self, ctx: &egui::Context) {
        let Some((mode, mut word)) = self.prompt.take() else { return };
        let mut submit = false;
        let mut cancel = false;
        let modal = egui::Modal::new(Id::unique("word_prompt")).show(ctx, |ui| {
            ui.heading(mode.title());
            ui.label("Word:");
            let resp = ui.text_edit_singleline(&mut word);
            resp.request_focus();
            if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                submit = true;
            }
            ui.horizontal(|ui| {
                submit |= ui.button("Search").clicked();
                cancel |= ui.button("Cancel").clicked();
            });
        });
        if modal.should_close() {
            cancel = true;
        }
        let trimmed = word.trim().to_string();
        if submit && !trimmed.is_empty() {
            let caret = self.caret;
            self.request(ctx, &trimmed, caret, caret, mode);
        } else if !cancel {
            self.prompt = Some((mode, word));
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = format!("{}{} \u{2014} {APP_NAME}", if self.dirty { "*" } else { "" }, self.display_name());
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
            self.handle_shortcuts(&ctx);
        }

        egui::Panel::top("menu").show(ui, |ui| {
            self.menu_bar(ui);
            self.toolbar(ui);
        });
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        let bg = ctx.global_style().visuals.extreme_bg_color;
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(bg)).show(ui, |ui| self.editor(ui));

        self.word_prompt(&ctx);
        self.update_title(&ctx);
        self.handle_close(&ctx);
    }
}
