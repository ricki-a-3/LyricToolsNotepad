"""Main window: file handling, modes, toolbar, and the rhyme dropdown wiring."""
import os
from importlib import resources

from PySide6.QtCore import QSettings, Qt
from PySide6.QtGui import QAction, QActionGroup, QFont, QFontDatabase, QIcon, QKeySequence
from PySide6.QtWidgets import (QApplication, QFileDialog, QFontDialog, QInputDialog, QLabel,
                               QMainWindow, QMessageBox, QToolBar)

from . import __version__, core
from .editor import LyricEditor
from .lookup import (ARMED_MODES, MODE_DEEP, MODE_NEAR, MODE_PERFECT, MODE_REPORT,
                     MODE_SYNONYM, MODE_TITLES, LookupService)
from .popup import SuggestionPopup
from .textutil import decode_text, encode_text, match_case
from .theme import apply_theme

APP_NAME = "LyricTools++"
MAX_RECENT = 10
FILE_FILTER = "Lyrics / text (*.txt *.lyrics *.md);;All files (*)"


def icon(name):
    return QIcon(str(resources.files("lyrictools") / "resources" / (name + ".ico")))


class MainWindow(QMainWindow):
    def __init__(self, settings: QSettings):
        super().__init__()
        self.settings = settings
        self.path = None
        self.newline = os.linesep if os.linesep in ("\r\n", "\n") else "\n"
        self.encoding = "utf-8"
        self.theme = settings.value("theme", "dark")
        self.mode = settings.value("mode", MODE_REPORT)
        if self.mode not in ARMED_MODES:
            self.mode = MODE_REPORT

        self.editor = LyricEditor(self)
        self.setCentralWidget(self.editor)
        self.setWindowIcon(icon("start"))
        self.setAcceptDrops(True)

        self.lookups = LookupService(self)
        self.lookups.finished.connect(self._on_lookup_finished)
        self.popup = SuggestionPopup(self)
        self.popup.picked.connect(self._on_pick)
        self.popup.dismissed.connect(self._on_popup_dismissed)
        self.popup.typed.connect(self._forward_key)
        self._pending = None  # (request_id, start, end) of the lookup in flight
        self._popup_span = None

        self.editor.word_selected.connect(self._on_word_selected)
        self.editor.selection_cleared.connect(self._cancel_lookup)
        self.editor.document().modificationChanged.connect(self._update_title)
        self.editor.cursorPositionChanged.connect(self._update_status)
        self.editor.textChanged.connect(self._update_status)

        self._build_actions()
        self._build_menus()
        self._build_toolbar()
        self._build_status()
        self._load_font()
        apply_theme(QApplication.instance(), self.editor, self.theme)
        self._set_mode(self.mode, announce=False)

        geometry = settings.value("geometry")
        if geometry is not None:
            self.restoreGeometry(geometry)
        else:
            self.resize(900, 700)
        self._update_title()
        self._update_status()

    # --- actions ----------------------------------------------------------------

    def _action(self, text, slot, shortcut=None, icon_name=None, tip=None, checkable=False):
        act = QAction(text, self)
        if icon_name:
            act.setIcon(icon(icon_name))
        if shortcut:
            act.setShortcut(QKeySequence(shortcut))
        if tip:
            act.setStatusTip(tip)
            act.setToolTip("%s (%s)" % (tip, act.shortcut().toString()) if shortcut else tip)
        act.setCheckable(checkable)
        act.triggered.connect(slot)
        return act

    def _build_actions(self):
        S = QKeySequence.StandardKey
        self.act_new = self._action("&New", self.new_file, S.New)
        self.act_open = self._action("&Open...", self.open_dialog, S.Open)
        self.act_save = self._action("&Save", self.save, S.Save)
        self.act_save_as = self._action("Save &As...", self.save_as, S.SaveAs)
        self.act_quit = self._action("E&xit", self.close, "Ctrl+Q")

        self.mode_group = QActionGroup(self)
        self.mode_actions = {}
        for mode, text, key, icon_name, tip in (
            (MODE_PERFECT, "&Perfect Rhymes", "Ctrl+1", "rhymes", "Highlight a word for perfect rhymes"),
            (MODE_NEAR, "&Near Rhymes", "Ctrl+2", "nearrhymes", "Highlight a word for near / slant rhymes"),
            (MODE_REPORT, "Rhyme &Report (Perfect + Near)", "Ctrl+3", "rhymereport",
             "Highlight a word for perfect + near rhymes"),
        ):
            act = self._action(text, lambda _=False, m=mode: self._set_mode(m), key, icon_name, tip, True)
            self.mode_group.addAction(act)
            self.mode_actions[mode] = act

        self.act_synonyms = self._action("&Synonyms", lambda: self.one_shot(MODE_SYNONYM), "F7",
                                         "synonyms", "Synonyms for the selected word (online)")
        self.act_deep = self._action("&Deep Search", lambda: self.one_shot(MODE_DEEP), "F8",
                                     "deepsearch", "Biggest rhyme list: offline + Datamuse (online)")
        self.act_rhyme_here = self._action("Rhymes for Word at &Caret", self.rhyme_at_caret,
                                           "Ctrl+Space", tip="Rhymes for the word at the caret")

        self.act_font = self._action("&Font...", self.choose_font)
        self.act_zoom_in = self._action("Zoom &In", lambda: self.editor.zoom(1), S.ZoomIn)
        self.act_zoom_out = self._action("Zoom &Out", lambda: self.editor.zoom(-1), S.ZoomOut)
        self.act_wrap = self._action("&Word Wrap", self.toggle_wrap, checkable=True)
        self.act_wrap.setChecked(self.settings.value("wrap", True, type=bool))
        self.act_dark = self._action("&Dark Theme", self.toggle_theme, checkable=True)
        self.act_dark.setChecked(self.theme == "dark")
        self.act_about = self._action("&About LyricTools++", self.about)

    def _build_menus(self):
        bar = self.menuBar()
        m = bar.addMenu("&File")
        m.addActions([self.act_new, self.act_open])
        self.recent_menu = m.addMenu("Open &Recent")
        self._rebuild_recent_menu()
        m.addSeparator()
        m.addActions([self.act_save, self.act_save_as])
        m.addSeparator()
        m.addAction(self.act_quit)

        e = bar.addMenu("&Edit")
        for text, slot, key in (("&Undo", self.editor.undo, QKeySequence.StandardKey.Undo),
                                ("&Redo", self.editor.redo, QKeySequence.StandardKey.Redo),
                                (None, None, None),
                                ("Cu&t", self.editor.cut, QKeySequence.StandardKey.Cut),
                                ("&Copy", self.editor.copy, QKeySequence.StandardKey.Copy),
                                ("&Paste", self.editor.paste, QKeySequence.StandardKey.Paste),
                                (None, None, None),
                                ("Select &All", self.editor.selectAll, QKeySequence.StandardKey.SelectAll)):
            if text is None:
                e.addSeparator()
            else:
                e.addAction(self._action(text, slot, key))

        r = bar.addMenu("&Rhymes")
        r.addActions(list(self.mode_actions.values()))
        r.addSeparator()
        r.addActions([self.act_synonyms, self.act_deep, self.act_rhyme_here])

        v = bar.addMenu("&View")
        v.addActions([self.act_font, self.act_zoom_in, self.act_zoom_out])
        v.addSeparator()
        v.addActions([self.act_wrap, self.act_dark])

        h = bar.addMenu("&Help")
        h.addAction(self.act_about)

    def _build_toolbar(self):
        tb = QToolBar("Tools", self)
        tb.setObjectName("MainToolbar")
        tb.setMovable(False)
        tb.setToolButtonStyle(Qt.ToolButtonStyle.ToolButtonTextOnly)
        for mode, label in ((MODE_PERFECT, "Perfect"), (MODE_NEAR, "Near"), (MODE_REPORT, "Report")):
            self.mode_actions[mode].setIconText(label)
        self.act_synonyms.setIconText("Synonyms")
        self.act_deep.setIconText("Deep Search")
        tb.addActions(list(self.mode_actions.values()))
        tb.addSeparator()
        tb.addActions([self.act_synonyms, self.act_deep])
        self.addToolBar(tb)

    def _build_status(self):
        self.status_mode = QLabel()
        self.status_pos = QLabel()
        self.statusBar().addPermanentWidget(self.status_pos)
        self.statusBar().addPermanentWidget(self.status_mode)

    def _load_font(self):
        font = QFont()
        saved = self.settings.value("font")
        if not (saved and font.fromString(saved)):
            font = QFontDatabase.systemFont(QFontDatabase.SystemFont.FixedFont)
            for family in ("Cascadia Mono", "Consolas"):
                if family in QFontDatabase.families():
                    font = QFont(family)
                    break
            font.setPointSize(13)
        self.editor.set_editor_font(font)
        self.editor.setLineWrapMode(self.editor.LineWrapMode.WidgetWidth if self.act_wrap.isChecked()
                                    else self.editor.LineWrapMode.NoWrap)

    # --- modes & lookups ------------------------------------------------------------

    def _set_mode(self, mode, announce=True):
        self.mode = mode
        self.mode_actions[mode].setChecked(True)
        self.settings.setValue("mode", mode)
        self.status_mode.setText("  Mode: %s  " % MODE_TITLES[mode])
        if announce:
            self.statusBar().showMessage("Highlight a word for %s" % MODE_TITLES[mode].lower(), 3000)
        # Already sitting on a whole word? Show it in the new mode right away.
        self.editor.recheck_selection()

    def _on_word_selected(self, word, start, end):
        self._request(word, start, end, self.mode)

    def _request(self, word, start, end, mode):
        self.popup.hide()
        request_id = self.lookups.request(word, mode)
        self._pending = (request_id, start, end)
        if mode == MODE_SYNONYM or mode == MODE_DEEP:
            self.statusBar().showMessage("%s for \u201c%s\u201d: searching online..." % (MODE_TITLES[mode], word))

    def _cancel_lookup(self):
        self.lookups.cancel()
        self._pending = None
        self.popup.hide()

    def _on_lookup_finished(self, request_id, word, mode, groups, error):
        if self._pending is None or self._pending[0] != request_id or not self.lookups.is_current(request_id):
            return  # superseded: the user already moved on
        _, start, end = self._pending
        self._pending = None
        if error:
            self.statusBar().showMessage(
                "%s for \u201c%s\u201d failed (%s). Offline rhyme modes still work." % (MODE_TITLES[mode], word, error),
                8000)
            return
        # The document may have changed while we waited; only anchor the
        # popup if the word is still exactly where it was.
        if start != end and self.editor.text_range(start, end) != word:
            return
        count = self.popup.set_groups(groups, show_headers=(mode == MODE_REPORT))
        if mode in (MODE_SYNONYM, MODE_DEEP):
            self.statusBar().showMessage("%s for \u201c%s\u201d: %d found" % (MODE_TITLES[mode], word, count), 4000)
        if not count:
            if mode in (MODE_SYNONYM, MODE_DEEP):
                self.statusBar().showMessage("No %s found for \u201c%s\u201d" % (MODE_TITLES[mode].lower(), word), 4000)
            return
        self._popup_span = (start, end, word)
        rect = self.editor.cursor_rect_at(start)
        top_left = self.editor.viewport().mapToGlobal(rect.topLeft())
        rect.moveTopLeft(top_left)
        self.popup.show_at(rect)

    def _on_pick(self, choice):
        if self._popup_span is None:
            return
        start, end, original = self._popup_span
        self._popup_span = None
        if start != end and self.editor.text_range(start, end) != original:
            return
        replacement = match_case(original, choice) if start != end else choice
        self.editor.replace_range(start, end, replacement)
        self.editor.setFocus()

    def _on_popup_dismissed(self):
        # Esc / click-away: drop the popup, keep the user's selection as-is.
        self._popup_span = None
        self.editor.setFocus()

    def _forward_key(self, event):
        self._popup_span = None
        self.editor.setFocus()
        self.editor.keyPressEvent(event)

    def one_shot(self, mode):
        """Synonyms / Deep Search: run once for the selected word, the word at
        the caret, or a typed word. Never armed for future highlights."""
        picked = self.editor.word_for_lookup()
        if picked is None:
            word, ok = QInputDialog.getText(self, MODE_TITLES[mode], "Word:")
            word = word.strip()
            if not ok or not word:
                return
            pos = self.editor.textCursor().position()
            picked = (word, pos, pos)  # pick inserts at the caret
        self._request(*picked, mode)

    def rhyme_at_caret(self):
        picked = self.editor.word_for_lookup()
        if picked is not None:
            self._request(*picked, self.mode)

    # --- files ----------------------------------------------------------------------

    def maybe_save(self):
        if not self.editor.document().isModified():
            return True
        answer = QMessageBox.warning(
            self, APP_NAME, "Save changes to %s?" % self._display_name(),
            QMessageBox.StandardButton.Save | QMessageBox.StandardButton.Discard
            | QMessageBox.StandardButton.Cancel, QMessageBox.StandardButton.Save)
        if answer == QMessageBox.StandardButton.Save:
            return self.save()
        return answer == QMessageBox.StandardButton.Discard

    def new_file(self):
        if not self.maybe_save():
            return
        self.editor.setPlainText("")
        self.path = None
        self.encoding = "utf-8"
        self.editor.document().setModified(False)
        self._update_title()

    def open_dialog(self):
        if not self.maybe_save():
            return
        start_dir = os.path.dirname(self.path) if self.path else self.settings.value("last_dir", "")
        path, _ = QFileDialog.getOpenFileName(self, "Open", start_dir, FILE_FILTER)
        if path:
            self.load(path)

    def load(self, path):
        try:
            with open(path, "rb") as fh:
                text, newline, encoding = decode_text(fh.read())
        except OSError as exc:
            QMessageBox.critical(self, APP_NAME, "Couldn't open %s:\n%s" % (path, exc))
            self._remove_recent(path)
            return False
        self.editor.setPlainText(text)
        self.path, self.newline, self.encoding = os.path.abspath(path), newline, encoding
        self.editor.document().setModified(False)
        self._remember(self.path)
        self._update_title()
        return True

    def save(self):
        if not self.path:
            return self.save_as()
        return self._write(self.path)

    def save_as(self):
        start = self.path or os.path.join(self.settings.value("last_dir", ""), "lyrics.txt")
        path, _ = QFileDialog.getSaveFileName(self, "Save As", start, FILE_FILTER)
        if not path:
            return False
        if not os.path.splitext(path)[1]:
            path += ".txt"
        return self._write(path)

    def _write(self, path):
        data = encode_text(self.editor.toPlainText(), self.newline, self.encoding)
        tmp = path + ".tmp"
        try:
            # Write-then-rename so a crash mid-save can't truncate the lyrics.
            with open(tmp, "wb") as fh:
                fh.write(data)
            os.replace(tmp, path)
        except OSError as exc:
            QMessageBox.critical(self, APP_NAME, "Couldn't save %s:\n%s" % (path, exc))
            return False
        self.path = os.path.abspath(path)
        if self.encoding not in ("utf-8", "utf-8-sig"):
            self.encoding = "utf-8"
        self.editor.document().setModified(False)
        self._remember(self.path)
        self._update_title()
        self.statusBar().showMessage("Saved %s" % self.path, 3000)
        return True

    def _recent(self):
        value = self.settings.value("recent", [])
        if isinstance(value, str):
            value = [value]
        return [p for p in (value or []) if p]

    def _remember(self, path):
        recent = [path] + [p for p in self._recent() if os.path.normcase(p) != os.path.normcase(path)]
        self.settings.setValue("recent", recent[:MAX_RECENT])
        self.settings.setValue("last_dir", os.path.dirname(path))
        self._rebuild_recent_menu()

    def _remove_recent(self, path):
        self.settings.setValue("recent", [p for p in self._recent() if p != path])
        self._rebuild_recent_menu()

    def _rebuild_recent_menu(self):
        self.recent_menu.clear()
        recent = self._recent()
        for i, path in enumerate(recent, 1):
            act = self.recent_menu.addAction("&%d  %s" % (i % 10, path))
            act.triggered.connect(lambda _=False, p=path: self._open_recent(p))
        if recent:
            self.recent_menu.addSeparator()
            self.recent_menu.addAction("Clear List", self._clear_recent)
        self.recent_menu.setEnabled(bool(recent))

    def _open_recent(self, path):
        if self.maybe_save():
            self.load(path)

    def _clear_recent(self):
        self.settings.setValue("recent", [])
        self._rebuild_recent_menu()

    # --- view -----------------------------------------------------------------------

    def choose_font(self):
        ok, font = QFontDialog.getFont(self.editor.font(), self, "Editor Font")
        if ok:
            self.editor.set_editor_font(font)

    def toggle_wrap(self, on):
        self.editor.setLineWrapMode(self.editor.LineWrapMode.WidgetWidth if on
                                    else self.editor.LineWrapMode.NoWrap)
        self.settings.setValue("wrap", on)

    def toggle_theme(self, dark):
        self.theme = "dark" if dark else "light"
        self.settings.setValue("theme", self.theme)
        apply_theme(QApplication.instance(), self.editor, self.theme)

    def about(self):
        QMessageBox.about(
            self, "About " + APP_NAME,
            "<b>%s</b> %s<br><br>Live syllable counts and highlight-to-rhyme suggestions "
            "for lyric writing.<br><br>Rhymes and syllables are offline (CMU Pronouncing "
            "Dictionary). Synonyms and Deep Search use the free Datamuse API.<br><br>"
            "MIT licensed. Built with Qt for Python (PySide6, LGPL)." % (APP_NAME, __version__))

    # --- status / title ---------------------------------------------------------------

    def _display_name(self):
        return os.path.basename(self.path) if self.path else "Untitled"

    def _update_title(self, *_):
        dirty = "*" if self.editor.document().isModified() else ""
        self.setWindowTitle("%s%s \u2014 %s" % (dirty, self._display_name(), APP_NAME))

    def _update_status(self):
        cursor = self.editor.textCursor()
        text = cursor.block().text()
        syl = 0 if core.is_section_header(text) else self.editor.line_syllables(text)
        self.status_pos.setText("  Ln %d, Col %d   |   %d syllable%s  " % (
            cursor.blockNumber() + 1, cursor.positionInBlock() + 1, syl, "" if syl == 1 else "s"))

    # --- window events ----------------------------------------------------------------

    def dragEnterEvent(self, event):
        if event.mimeData().hasUrls():
            event.acceptProposedAction()

    def dropEvent(self, event):
        urls = [u.toLocalFile() for u in event.mimeData().urls() if u.isLocalFile()]
        if urls and self.maybe_save():
            self.load(urls[0])

    def closeEvent(self, event):
        if not self.maybe_save():
            event.ignore()
            return
        self.settings.setValue("geometry", self.saveGeometry())
        self.settings.setValue("font", self.editor.font().toString())
        self.lookups.shutdown()
        event.accept()
