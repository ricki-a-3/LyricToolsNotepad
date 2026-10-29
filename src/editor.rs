//! The editor view: styled lyric text, syllable gutter with fold arrows,
//! the highlight-to-rhyme dropdown, and formatting commands.

use std::collections::HashSet;
use std::sync::Arc;

use eframe::egui::{
    self, Align2, Color32, FontFamily, FontId, Id, Key, Margin, Modifiers, Order, Pos2, Rect, Sense, Shape, Stroke,
    Vec2,
    text::{CCursor, LayoutJob, TextFormat},
    text_selection::CCursorRange,
};

use crate::app::{FONT_BOLD, FONT_BOLD_ITALIC, FONT_ITALIC, LyricApp, Prompt, shortcut};
use crate::core::{self, WordResult};
use crate::lookup::{Groups, Mode};
use crate::markup::{self, Block, Edit, LineKind};
use crate::settings::Settings;
use crate::textutil::{
    byte_index, char_slice, color_for_count, is_whole_word_selection, line_at, match_case, word_span_at,
};

const EDITOR_ID: &str = "lyric_editor";
/// Wait this long after the selection stops changing (and the mouse is up)
/// before treating it as "a word was highlighted".
const SELECTION_SETTLE_SECS: f64 = 0.18;
const POPUP_WIDTH: f32 = 260.0;
const POPUP_MAX_HEIGHT: f32 = 300.0;
/// Folded lines are laid out this small (and invisible), so they take up
/// no visible space while staying in the text.
const HIDDEN_FONT_SIZE: f32 = 0.01;
const FOLD_ARROW_W: f32 = 16.0;

pub enum PopupRow {
    Header(&'static str),
    Word(WordResult),
}

pub struct Popup {
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
        let mut seen = HashSet::new();
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

pub struct Pending {
    id: u64,
    start: usize,
    end: usize,
}

// --- styled layout ------------------------------------------------------------------

struct Palette {
    text: Color32,
    marker: Color32,
    section: Color32,
    heading: Color32,
}

impl Palette {
    fn new(dark: bool, text: Color32) -> Self {
        if dark {
            Palette {
                text,
                marker: Color32::from_rgb(0x5c, 0x63, 0x70),
                section: Color32::from_rgb(0x8a, 0xb4, 0xf8),
                heading: Color32::from_rgb(0xff, 0xff, 0xff),
            }
        } else {
            Palette {
                text,
                marker: Color32::from_rgb(0xa0, 0xa6, 0xb0),
                section: Color32::from_rgb(0x1f, 0x5f, 0xc4),
                heading: Color32::from_rgb(0x10, 0x10, 0x10),
            }
        }
    }
}

fn family(bold: bool, italic: bool) -> FontFamily {
    match (bold, italic) {
        (false, false) => FontFamily::Monospace,
        (true, false) => FontFamily::Name(FONT_BOLD.into()),
        (false, true) => FontFamily::Name(FONT_ITALIC.into()),
        (true, true) => FontFamily::Name(FONT_BOLD_ITALIC.into()),
    }
}

fn format(size: f32, bold: bool, italic: bool, color: Color32) -> TextFormat {
    TextFormat { font_id: FontId::new(size, family(bold, italic)), color, ..Default::default() }
}

fn heading_scale(level: u8) -> f32 {
    match level {
        1 => 1.55,
        2 => 1.3,
        _ => 1.12,
    }
}

/// Lines (by index) hidden inside folded blocks.
fn hidden_lines(blocks: &[Block], folds: &HashSet<(String, usize)>) -> HashSet<usize> {
    blocks.iter().filter(|b| folds.contains(&b.key)).flat_map(|b| b.body_lines()).collect()
}

fn build_job(text: &str, folds: &HashSet<(String, usize)>, size: f32, pal: &Palette, wrap_width: f32) -> LayoutJob {
    let mut job = LayoutJob { text: String::new(), ..Default::default() };
    job.wrap.max_width = wrap_width;
    let lines: Vec<&str> = text.split('\n').collect();
    let hidden = hidden_lines(&markup::blocks(&lines), folds);
    let last = lines.len() - 1;
    for (i, line) in lines.iter().enumerate() {
        let newline = if i < last { "\n" } else { "" };
        if hidden.contains(&i) {
            job.append(&format!("{line}{newline}"), 0.0, format(HIDDEN_FONT_SIZE, false, false, Color32::TRANSPARENT));
            continue;
        }
        match markup::line_kind(line) {
            LineKind::Heading(level) => {
                let big = size * heading_scale(level);
                let marks = level as usize + 1;
                job.append(&line[..marks], 0.0, format(big, true, false, pal.marker));
                job.append(&line[marks..], 0.0, format(big, true, false, pal.heading));
                job.append(newline, 0.0, format(big, true, false, pal.heading));
            }
            LineKind::Section => {
                let fmt = format(size * 1.05, true, false, pal.section);
                job.append(line, 0.0, fmt.clone());
                job.append(newline, 0.0, fmt);
            }
            LineKind::Plain => {
                for (range, style) in markup::inline_spans(line) {
                    let color = if style.marker { pal.marker } else { pal.text };
                    let mut fmt = format(size, style.bold, style.italic, color);
                    if style.underline {
                        fmt.underline = Stroke::new(1.0, pal.text);
                    }
                    job.append(&line[range], 0.0, fmt);
                }
                job.append(newline, 0.0, format(size, false, false, pal.text));
            }
        }
    }
    job
}

impl LyricApp {
    // --- lookups ---------------------------------------------------------------------

    pub(crate) fn line_syllables(&mut self, line: &str) -> u32 {
        if let Some(&n) = self.counts.get(line) {
            return n;
        }
        let n = if line.trim().is_empty() || markup::line_kind(line) != LineKind::Plain {
            0
        } else {
            core::syllables_for_line(line)
        };
        if self.counts.len() > 20_000 {
            self.counts.clear();
        }
        self.counts.insert(line.to_string(), n);
        n
    }

    fn set_mode(&mut self, ctx: &egui::Context, mode: Mode) {
        self.settings.mode = mode;
        self.save_settings();
        self.flash(ctx, format!("Highlight a word for {}", mode.title().to_lowercase()));
        // Already sitting on a whole word? Show it in the new mode right away.
        self.last_word_signature = None;
        self.sel_evaluated = false;
        self.focus_editor = true;
    }

    pub(crate) fn request(&mut self, ctx: &egui::Context, word: &str, start: usize, end: usize, mode: Mode) {
        self.popup = None;
        let id = self.lookups.request(word, mode);
        self.pending = Some(Pending { id, start, end });
        self.focus_editor = true;
        if mode.is_network() {
            self.flash(ctx, format!("{} for \u{201C}{word}\u{201D}: searching online...", mode.title()));
        }
    }

    fn cancel_lookup(&mut self) {
        self.lookups.cancel();
        self.pending = None;
        self.popup = None;
    }

    pub(crate) fn poll_lookups(&mut self, ctx: &egui::Context) {
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

    fn pick(&mut self, choice: String) {
        let Some(popup) = self.popup.take() else { return };
        let (start, end) = (popup.start, popup.end);
        if start != end && char_slice(&self.text, start, end) != popup.word {
            return;
        }
        let replacement = if start != end { match_case(&popup.word, &choice) } else { choice };
        let (b0, b1) = (byte_index(&self.text, start), byte_index(&self.text, end));
        self.text.replace_range(b0..b1, &replacement);
        self.dirty = true;
        let caret = start + replacement.chars().count();
        self.set_cursor = Some((caret, caret));
        self.last_word_signature = None;
        self.focus_editor = true;
    }

    /// Selection if any, else the word at the caret.
    fn word_for_lookup(&self) -> Option<(String, usize, usize)> {
        if let Some((start, end)) = self.last_sel.filter(|(s, e)| s != e) {
            let text = char_slice(&self.text, start, end);
            let trimmed = text.trim().trim_matches(['*', '_']);
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
            None => self.prompt = Some(Prompt::Word(mode, String::new())),
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

    // --- formatting ---------------------------------------------------------------------

    fn selection(&self) -> (usize, usize) {
        self.last_sel.unwrap_or((self.caret, self.caret))
    }

    /// Apply an edit from a formatting command, keeping its selection without
    /// popping up rhymes for it.
    fn apply_edit(&mut self, edit: Edit) {
        self.text = edit.text;
        self.dirty = true;
        self.popup = None;
        let (s, e) = edit.selection;
        self.set_cursor = Some((s, e));
        self.last_sel = Some((s, e));
        self.caret = e;
        self.sel_evaluated = true;
        self.last_word_signature = Some((s, e, char_slice(&self.text, s, e).to_string(), self.settings.mode));
        self.focus_editor = true;
    }

    fn wrap_selection(&mut self, marker: &str) {
        let edit = markup::toggle_wrap(&self.text, self.selection(), marker);
        self.apply_edit(edit);
    }

    fn cycle_heading(&mut self) {
        let edit = markup::cycle_heading(&self.text, self.caret);
        self.apply_edit(edit);
    }

    pub(crate) fn insert_section(&mut self, title: &str) {
        let edit = markup::insert_section(&self.text, self.selection().0, title);
        self.apply_edit(edit);
    }

    fn fold_all(&mut self, fold: bool) {
        self.folds.clear();
        if fold {
            let lines: Vec<&str> = self.text.split('\n').collect();
            self.folds =
                markup::blocks(&lines).into_iter().filter(|b| b.end_line > b.header_line + 1).map(|b| b.key).collect();
        }
        self.focus_editor = true;
    }

    fn zoom(&mut self, delta: f32) {
        self.settings.font_size = if delta == 0.0 {
            Settings::default().font_size
        } else {
            (self.settings.font_size + delta).clamp(8.0, 48.0)
        };
        self.save_settings();
    }

    // --- chrome ----------------------------------------------------------------------

    pub(crate) fn handle_editor_shortcuts(&mut self, ctx: &egui::Context) {
        let cmd = Modifiers::COMMAND;
        let pressed = |m: Modifiers, k: Key| shortcut(ctx, m, k);

        // Dropdown navigation first, so the editor never sees these keys.
        if let Some(popup) = self.popup.as_mut() {
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
                self.popup = None;
            } else if (pressed(Modifiers::NONE, Key::Enter) || pressed(Modifiers::NONE, Key::Tab))
                && let Some(word) = popup.selected_word().map(str::to_string)
            {
                self.pick(word);
            }
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
        if pressed(cmd, Key::B) {
            self.wrap_selection("**");
        }
        if pressed(cmd, Key::I) {
            self.wrap_selection("*");
        }
        if pressed(cmd, Key::U) {
            self.wrap_selection("__");
        }
        if pressed(cmd | Modifiers::SHIFT, Key::H) {
            self.cycle_heading();
        }
        if pressed(cmd | Modifiers::SHIFT, Key::N) {
            self.prompt = Some(Prompt::Section(String::new()));
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

    pub(crate) fn editor_menus(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let item =
            |ui: &mut egui::Ui, label: &str, key: &str| ui.add(egui::Button::new(label).shortcut_text(key)).clicked();
        ui.menu_button("Rhymes", |ui| {
            for (mode, label, key) in [
                (Mode::Perfect, "Perfect Rhymes", "Ctrl+1"),
                (Mode::Near, "Near Rhymes", "Ctrl+2"),
                (Mode::Report, "Near + Perfect Rhymes", "Ctrl+3"),
            ] {
                if ui.add(egui::Button::selectable(self.settings.mode == mode, label).shortcut_text(key)).clicked() {
                    self.set_mode(&ctx, mode);
                }
            }
            ui.separator();
            if item(ui, "Synonyms (online)", "F7") {
                self.one_shot(&ctx, Mode::Synonyms);
            }
            if item(ui, "Deep Search (online)", "F8") {
                self.one_shot(&ctx, Mode::Deep);
            }
            if item(ui, "Rhymes for Word at Cursor", "F6") {
                self.rhyme_at_caret(&ctx);
            }
        });
        ui.menu_button("Format", |ui| {
            if item(ui, "Bold", "Ctrl+B") {
                self.wrap_selection("**");
            }
            if item(ui, "Italic", "Ctrl+I") {
                self.wrap_selection("*");
            }
            if item(ui, "Underline", "Ctrl+U") {
                self.wrap_selection("__");
            }
            if item(ui, "Heading (cycle # / ## / none)", "Ctrl+Shift+H") {
                self.cycle_heading();
            }
            ui.separator();
            if item(ui, "New Section...", "Ctrl+Shift+N") {
                self.prompt = Some(Prompt::Section(String::new()));
            }
            if item(ui, "Fold All Sections", "") {
                self.fold_all(true);
            }
            if item(ui, "Unfold All Sections", "") {
                self.fold_all(false);
            }
        });
        ui.menu_button("View", |ui| {
            if item(ui, "Bigger Text", "Ctrl+=") {
                self.zoom(1.0);
            }
            if item(ui, "Smaller Text", "Ctrl+-") {
                self.zoom(-1.0);
            }
            if item(ui, "Reset Text Size", "Ctrl+0") {
                self.zoom(0.0);
            }
            ui.separator();
            if ui.checkbox(&mut self.settings.wrap, "Word Wrap").changed() {
                self.save_settings();
            }
        });
    }

    pub(crate) fn editor_toolbar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ui.horizontal_wrapped(|ui| {
            let tool = |ui: &mut egui::Ui, text: egui::RichText, on: bool, tip: &str| -> bool {
                ui.add(egui::Button::selectable(on, text.size(14.0)).min_size(Vec2::new(0.0, 28.0)))
                    .on_hover_text(tip)
                    .clicked()
            };
            let t = egui::RichText::new;
            let mode = self.settings.mode;
            if tool(ui, t("Perfect"), mode == Mode::Perfect, "Perfect rhymes (Ctrl+1)") {
                self.set_mode(&ctx, Mode::Perfect);
            }
            if tool(ui, t("Near"), mode == Mode::Near, "Near / slant rhymes (Ctrl+2)") {
                self.set_mode(&ctx, Mode::Near);
            }
            if tool(ui, t("Near + Perfect"), mode == Mode::Report, "Near + perfect rhymes (Ctrl+3)") {
                self.set_mode(&ctx, Mode::Report);
            }
            ui.separator();
            if tool(ui, t("Synonyms"), false, "Synonyms for the selected word, online (F7)") {
                self.one_shot(&ctx, Mode::Synonyms);
            }
            if tool(ui, t("Deep Search"), false, "The biggest rhyme list for the selected word: offline + online (F8)")
            {
                self.one_shot(&ctx, Mode::Deep);
            }
            ui.separator();
            if tool(ui, t("B").strong(), false, "Bold (Ctrl+B)") {
                self.wrap_selection("**");
            }
            if tool(ui, t("I").italics(), false, "Italic (Ctrl+I)") {
                self.wrap_selection("*");
            }
            if tool(ui, t("U").underline(), false, "Underline (Ctrl+U)") {
                self.wrap_selection("__");
            }
            if tool(ui, t("Heading"), false, "Make this line a heading; click again for a smaller one (Ctrl+Shift+H)") {
                self.cycle_heading();
            }
            if tool(ui, t("+ Section"), false, "Add a titled, foldable section like [Chorus] (Ctrl+Shift+N)") {
                self.prompt = Some(Prompt::Section(String::new()));
            }
            ui.separator();
            if tool(ui, t("Fold All"), false, "Collapse every section to its title") {
                self.fold_all(true);
            }
            if tool(ui, t("Unfold All"), false, "Expand every section") {
                self.fold_all(false);
            }
        });
    }

    pub(crate) fn editor_status(&mut self, ui: &mut egui::Ui) {
        let (line_no, line_start, line) = line_at(&self.text, self.caret);
        let col = self.caret - line_start + 1;
        let line = line.to_string();
        let syl = self.line_syllables(&line);
        ui.label(format!("Mode: {}", self.settings.mode.title()));
        ui.separator();
        ui.label(format!("{syl} syllable{}", if syl == 1 { "" } else { "s" }));
        ui.separator();
        ui.label(format!("Ln {}, Col {col}", line_no + 1));
    }

    // --- the editor ------------------------------------------------------------------------

    pub(crate) fn editor(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let id = Id::unique(EDITOR_ID);
        if let Some(state) = self.pending_state.take() {
            state.store(&ctx, id);
        }
        if let Some((a, b)) = self.set_cursor.take() {
            let mut state = egui::text_edit::TextEditState::load(&ctx, id).unwrap_or_default();
            state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(a), CCursor::new(b))));
            state.store(&ctx, id);
        }
        // Keep asking until it sticks: a dialog that just closed still holds
        // focus for a frame and would swallow a one-off request.
        if self.focus_editor && self.prompt.is_none() {
            ctx.memory_mut(|m| m.request_focus(id));
            ctx.request_repaint();
        }

        let size = self.settings.font_size;
        let font = FontId::monospace(size);
        let digit_w = ctx.fonts_mut(|f| f.glyph_width(&font, '8'));
        let gutter_w = (FOLD_ARROW_W + digit_w * 3.0 + 14.0).min(120.0);
        let wrap = self.settings.wrap;
        let folds = self.folds.clone();
        let dark = self.settings.dark;
        let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap_width: f32| {
            let pal = Palette::new(dark, ui.visuals().text_color());
            let job = build_job(buf.as_str(), &folds, size, &pal, if wrap { wrap_width } else { f32::INFINITY });
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
                    .frame(egui::Frame::NONE.inner_margin(Margin { left: gutter_w as i8, right: 8, top: 8, bottom: 8 }))
                    .desired_width(f32::INFINITY)
                    .min_size(ui.available_size())
                    .lock_focus(true)
                    .layouter(&mut layouter)
                    .show(ui)
            })
            .inner;

        if output.response.has_focus() {
            self.focus_editor = false;
        }
        if output.response.changed() {
            self.dirty = true;
            self.popup = None;
        }
        // egui only reports the selection while the editor has focus.
        // Clicking a toolbar button takes focus away, so keep the last known
        // selection rather than treating that as "nothing selected" (which
        // used to cancel the Deep Search/Synonyms lookup the button started).
        let sel = output.cursor_range.map(|r| {
            let range = r.as_sorted_char_range();
            (range.start.0, range.end.0)
        });
        if let Some(r) = output.cursor_range {
            self.caret = r.primary.index.0;
        }

        let lines: Vec<String> = self.text.split('\n').map(str::to_string).collect();
        let line_refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        let blocks = markup::blocks(&line_refs);
        let hidden = hidden_lines(&blocks, &self.folds);

        // Typing or arrowing into a folded section opens it.
        if sel.is_some() {
            let (caret_line, _, _) = line_at(&self.text, self.caret);
            if hidden.contains(&caret_line)
                && let Some(b) = blocks.iter().find(|b| b.body_lines().contains(&caret_line))
            {
                self.folds.remove(&b.key);
            }
        }

        self.paint_gutter(ui, &output, gutter_w, &lines, &blocks, &hidden, id);

        // Highlight-to-rhyme trigger: act once the selection has settled.
        let now = ctx.input(|i| i.time);
        if sel.is_some() && sel != self.last_sel {
            self.last_sel = sel;
            self.sel_changed_at = now;
            self.sel_evaluated = false;
            if let Some(p) = &self.popup
                && sel != Some((p.start, p.end))
                && !(p.start == p.end && sel.is_some_and(|(a, b)| a == b && a == p.start))
            {
                self.popup = None;
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
            let anchor =
                output.galley.pos_from_cursor(CCursor::new(popup.start)).translate(output.galley_pos.to_vec2());
            self.show_popup(&ctx, Pos2::new(anchor.left(), anchor.bottom() + 2.0));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_gutter(
        &mut self,
        ui: &mut egui::Ui,
        output: &egui::text_edit::TextEditOutput,
        gutter_w: f32,
        lines: &[String],
        blocks: &[Block],
        hidden: &HashSet<usize>,
        id: Id,
    ) {
        let galley = Arc::clone(&output.galley);
        let clip = ui.clip_rect();
        let painter = ui.painter().with_clip_rect(clip);
        let left = output.response.rect.left();
        let dark = self.settings.dark;
        let bg = if dark { Color32::from_rgb(0x1b, 0x1d, 0x23) } else { Color32::from_rgb(0xf0, 0xf0, 0xf0) };
        let arrow_color = if dark { Color32::from_rgb(0x8a, 0x90, 0x9c) } else { Color32::from_rgb(0x70, 0x76, 0x80) };
        painter.rect_filled(Rect::from_x_y_ranges(left..=left + gutter_w - 6.0, clip.y_range()), 0.0, bg);

        let count_font = FontId::new(self.settings.font_size * 0.9, FontFamily::Name(FONT_BOLD.into()));
        let note_font = FontId::proportional(self.settings.font_size * 0.75);
        let mut toggles = Vec::new();
        let mut line_idx = 0;
        let mut at_line_start = true;
        for row in &galley.rows {
            if at_line_start && !hidden.contains(&line_idx) {
                let rect = row.rect().translate(output.galley_pos.to_vec2());
                if rect.bottom() >= clip.top() && rect.top() <= clip.bottom() {
                    let text = lines.get(line_idx).map(String::as_str).unwrap_or("");
                    let count = self.line_syllables(text);
                    if let Some((r, g, b)) = color_for_count(count) {
                        let pos = Pos2::new(left + gutter_w - 14.0, rect.center().y);
                        painter.text(
                            pos,
                            Align2::RIGHT_CENTER,
                            count.to_string(),
                            count_font.clone(),
                            Color32::from_rgb(r, g, b),
                        );
                    }
                    if let Some(block) =
                        blocks.iter().find(|b| b.header_line == line_idx && b.end_line > b.header_line + 1)
                    {
                        let folded = self.folds.contains(&block.key);
                        let c = Pos2::new(left + 9.0, rect.center().y);
                        let s = 4.5;
                        let points = if folded {
                            vec![c + Vec2::new(-s * 0.6, -s), c + Vec2::new(s * 0.9, 0.0), c + Vec2::new(-s * 0.6, s)]
                        } else {
                            vec![c + Vec2::new(-s, -s * 0.6), c + Vec2::new(s, -s * 0.6), c + Vec2::new(0.0, s * 0.9)]
                        };
                        painter.add(Shape::convex_polygon(points, arrow_color, Stroke::NONE));
                        if folded {
                            let n = block.body_lines().len();
                            let end = Pos2::new(rect.right() + 14.0, rect.center().y);
                            let note = format!("\u{2026} {n} line{}", if n == 1 { "" } else { "s" });
                            painter.text(end, Align2::LEFT_CENTER, note, note_font.clone(), arrow_color);
                        }
                        let hit = Rect::from_center_size(c, Vec2::new(FOLD_ARROW_W, rect.height().max(14.0)));
                        toggles.push((hit, block.key.clone(), line_idx));
                    }
                }
            }
            at_line_start = row.ends_with_newline;
            if row.ends_with_newline {
                line_idx += 1;
            }
        }
        // Fold arrows are added after the text, so they win the click.
        for (hit, key, line) in toggles {
            let resp = ui
                .interact(hit, id.with(("fold", line)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.clicked() && !self.folds.remove(&key) {
                self.folds.insert(key);
            }
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
                                    let body = egui::TextStyle::Body.resolve(ui.style());
                                    ui.painter().text(
                                        rect.left_center() + Vec2::new(8.0, 0.0),
                                        Align2::LEFT_CENTER,
                                        &r.word,
                                        body.clone(),
                                        ui.visuals().text_color(),
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
            self.pick(word);
            return;
        }
        // Click anywhere else closes it, leaving the text untouched.
        let clicked_outside = ctx.input(|i| {
            i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p))
        });
        if clicked_outside {
            self.popup = None;
        }
    }
}
