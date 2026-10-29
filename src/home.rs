//! Home dashboard: every lyric file in the lyrics folder as a card.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, CornerRadius, Rect, RichText, Sense, Stroke, Vec2};

use crate::app::{LyricApp, Prompt, View};
use crate::library::{self, LyricFile};

const CARD_W: f32 = 290.0;
const CARD_H: f32 = 132.0;
const GAP: f32 = 12.0;
const RESCAN_SECS: f64 = 5.0;

/// Open a folder (or select a file) in Explorer / the file manager.
pub fn reveal(path: &Path) {
    #[cfg(windows)]
    {
        let mut cmd = std::process::Command::new("explorer");
        if path.is_file() {
            cmd.arg(format!("/select,{}", path.display()));
        } else {
            cmd.arg(path);
        }
        let _ = cmd.spawn();
    }
    #[cfg(not(windows))]
    {
        let dir = if path.is_file() { path.parent().unwrap_or(path) } else { path };
        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
    }
}

fn matches(file: &LyricFile, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let q = query.to_lowercase();
    file.title.to_lowercase().contains(&q)
        || file.path.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase().contains(&q))
        || file.sections.iter().any(|s| s.to_lowercase().contains(&q))
        || file.preview.iter().any(|l| l.to_lowercase().contains(&q))
}

impl LyricApp {
    fn rescan_if_needed(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if self.library_stale || now - self.library_scanned_at > RESCAN_SECS {
            self.library = library::scan(&self.settings.lyrics_dir());
            self.library_stale = false;
            self.library_scanned_at = now;
        }
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(RESCAN_SECS));
    }

    pub(crate) fn home(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.rescan_if_needed(&ctx);
        let dir = self.settings.lyrics_dir();

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.heading(RichText::new("Your Lyrics").size(26.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("⟳ Refresh").clicked() {
                    self.library_stale = true;
                }
                if ui.button("Open File...").clicked() {
                    self.open_dialog(&ctx);
                }
                let new = egui::Button::new(RichText::new("+ New Lyric").strong().size(15.0))
                    .fill(ui.visuals().selection.bg_fill)
                    .min_size(Vec2::new(0.0, 30.0));
                if ui.add(new).on_hover_text("Ctrl+N").clicked() {
                    self.prompt = Some(Prompt::NewLyric(String::new()));
                }
            });
        });
        ui.horizontal(|ui| {
            if ui.link(format!("📁 {}", dir.display())).on_hover_text("Open this folder").clicked() {
                let _ = std::fs::create_dir_all(&dir);
                reveal(&dir);
            }
            if ui.small_button("Change...").clicked() {
                self.view = View::Settings;
            }
        });
        ui.add_space(6.0);

        if self.has_document() {
            let name =
                format!("Continue editing: {}{}", self.display_name(), if self.dirty { " (unsaved)" } else { "" });
            if ui.button(name).clicked() {
                self.view = View::Editor;
                self.focus_editor = true;
            }
            ui.add_space(4.0);
        }

        ui.add(
            egui::TextEdit::singleline(&mut self.home_search)
                .hint_text("🔍 Search titles, sections and lines...")
                .desired_width(f32::INFINITY),
        );
        ui.add_space(8.0);

        let shown: Vec<LyricFile> =
            self.library.iter().filter(|f| matches(f, self.home_search.trim())).cloned().collect();
        if self.library.is_empty() {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("No lyrics here yet").size(18.0));
                ui.label(RichText::new(format!("Lyric files (.txt) in {} show up here.", dir.display())).weak());
                ui.add_space(8.0);
                if ui.button("+ Write your first lyric").clicked() {
                    self.prompt = Some(Prompt::NewLyric(String::new()));
                }
            });
            return;
        }
        if shown.is_empty() {
            ui.label(RichText::new("Nothing matches your search.").weak());
            return;
        }

        let mut open: Option<PathBuf> = None;
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            let cols = (((ui.available_width() + GAP) / (CARD_W + GAP)).floor() as usize).max(1);
            for row in shown.chunks(cols) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = GAP;
                    for file in row {
                        if self.card(ui, file) {
                            open = Some(file.path.clone());
                        }
                    }
                });
                ui.add_space(GAP);
            }
            ui.label(
                RichText::new(format!("{} lyric file{}", shown.len(), if shown.len() == 1 { "" } else { "s" }))
                    .small()
                    .weak(),
            );
        });
        if let Some(path) = open {
            self.open_path(&ctx, &path);
        }
    }

    /// One file card. Returns true when clicked.
    fn card(&self, ui: &mut egui::Ui, file: &LyricFile) -> bool {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(CARD_W, CARD_H), Sense::click());
        let visuals = ui.visuals().clone();
        let is_open = self.path.as_deref() == Some(file.path.as_path());
        let fill = if resp.hovered() { visuals.widgets.hovered.weak_bg_fill } else { visuals.extreme_bg_color };
        let stroke = if is_open {
            Stroke::new(1.5, visuals.selection.bg_fill)
        } else if resp.hovered() {
            visuals.widgets.hovered.bg_stroke
        } else {
            Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color)
        };
        ui.painter().rect(rect, CornerRadius::same(8), fill, stroke, egui::StrokeKind::Inside);

        let inner = rect.shrink2(Vec2::new(12.0, 10.0));
        let mut child =
            ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
        child.set_clip_rect(inner.intersect(ui.clip_rect()));
        child.spacing_mut().item_spacing.y = 3.0;
        // Card text must not swallow the click meant for the card.
        child.style_mut().interaction.selectable_labels = false;
        child.add(egui::Label::new(RichText::new(&file.title).size(16.0).strong()).truncate());
        let sections =
            if file.sections.is_empty() { "No sections yet".to_string() } else { file.sections.join(" · ") };
        child.add(
            egui::Label::new(RichText::new(sections).small().color(Color32::from_rgb(0x8a, 0xb4, 0xf8))).truncate(),
        );
        for line in &file.preview {
            child.add(egui::Label::new(RichText::new(line).italics().weak()).truncate());
        }
        let name = file.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let footer = format!("{name} · {}", library::relative_time(file.modified));
        let footer_rect = Rect::from_min_max(egui::pos2(inner.left(), inner.bottom() - 14.0), inner.max);
        ui.painter().with_clip_rect(footer_rect).text(
            footer_rect.left_center(),
            egui::Align2::LEFT_CENTER,
            footer,
            egui::FontId::proportional(11.0),
            visuals.weak_text_color(),
        );

        let path = file.path.clone();
        resp.context_menu(|ui| {
            if ui.button("Show in folder").clicked() {
                reveal(&path);
                ui.close();
            }
        });
        resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
    }
}
