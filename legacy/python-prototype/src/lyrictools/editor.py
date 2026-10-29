"""The lyric editor: a QPlainTextEdit with a syllable-count gutter."""
from collections import OrderedDict

from PySide6.QtCore import QRect, QSize, Qt, QTimer, Signal
from PySide6.QtGui import QColor, QFont, QPainter, QTextCursor, QTextFormat
from PySide6.QtWidgets import QApplication, QPlainTextEdit, QTextEdit, QWidget

from . import core
from .textutil import color_for_count, is_whole_word_selection, word_span_at

GUTTER_PADDING = 8
SELECTION_SETTLE_MS = 180  # wait for drag-selects / double-clicks to finish


class _CountCache:
    """Line text -> syllable count. Counting is cheap, but the gutter repaints
    on every scroll and keystroke, so don't recount unchanged lines."""

    def __init__(self, limit=5000):
        self._data = OrderedDict()
        self._limit = limit

    def get(self, text):
        count = self._data.get(text)
        if count is None:
            count = 0 if not text.strip() else core.syllables_for_line(text)
            self._data[text] = count
            if len(self._data) > self._limit:
                self._data.popitem(last=False)
        else:
            self._data.move_to_end(text)
        return count


class SyllableGutter(QWidget):
    def __init__(self, editor):
        super().__init__(editor)
        self._editor = editor

    def sizeHint(self):
        return QSize(self._editor.gutter_width(), 0)

    def paintEvent(self, event):
        self._editor.paint_gutter(event)


class LyricEditor(QPlainTextEdit):
    """Emits ``word_selected(word, start, end)`` once the selection settles on
    exactly one whole word, and ``selection_cleared()`` when it no longer does.
    Positions are absolute document offsets."""

    word_selected = Signal(str, int, int)
    selection_cleared = Signal()

    def __init__(self, parent=None):
        super().__init__(parent)
        self._counts = _CountCache()
        self._gutter = SyllableGutter(self)
        self._gutter_bg = QColor("#1b1d23")
        self._gutter_fg = QColor("#6b7280")
        self._current_line_bg = QColor(255, 255, 255, 12)
        self._last_word_signature = None

        self.setLineWrapMode(QPlainTextEdit.LineWrapMode.WidgetWidth)
        self.setCenterOnScroll(False)

        self.blockCountChanged.connect(self._update_gutter_width)
        self.updateRequest.connect(self._update_gutter)
        self.cursorPositionChanged.connect(self._highlight_current_line)

        self._settle = QTimer(self)
        self._settle.setSingleShot(True)
        self._settle.setInterval(SELECTION_SETTLE_MS)
        self._settle.timeout.connect(self._check_selection)
        self.selectionChanged.connect(self._settle.start)

        self._update_gutter_width()
        self._highlight_current_line()

    # --- appearance ---------------------------------------------------------

    def set_colors(self, gutter_bg, gutter_fg, current_line_bg):
        self._gutter_bg = QColor(gutter_bg)
        self._gutter_fg = QColor(gutter_fg)
        self._current_line_bg = QColor(current_line_bg)
        self._highlight_current_line()
        self._gutter.update()

    def set_editor_font(self, font):
        self.setFont(font)
        self._update_gutter_width()
        self._gutter.update()

    def zoom(self, delta):
        font = QFont(self.font())
        font.setPointSizeF(max(6.0, min(72.0, font.pointSizeF() + delta)))
        self.set_editor_font(font)

    # --- gutter ---------------------------------------------------------------

    def gutter_width(self):
        metrics = self.fontMetrics()
        return GUTTER_PADDING * 2 + metrics.horizontalAdvance("888")

    def _update_gutter_width(self, *_):
        self.setViewportMargins(self.gutter_width(), 0, 0, 0)

    def _update_gutter(self, rect, dy):
        if dy:
            self._gutter.scroll(0, dy)
        else:
            self._gutter.update(0, rect.y(), self._gutter.width(), rect.height())
        if rect.contains(self.viewport().rect()):
            self._update_gutter_width()

    def resizeEvent(self, event):
        super().resizeEvent(event)
        cr = self.contentsRect()
        self._gutter.setGeometry(QRect(cr.left(), cr.top(), self.gutter_width(), cr.height()))

    def line_syllables(self, text):
        if core.is_section_header(text):
            return 0
        return self._counts.get(text)

    def paint_gutter(self, event):
        painter = QPainter(self._gutter)
        painter.fillRect(event.rect(), self._gutter_bg)
        bold = QFont(self.font())
        bold.setBold(True)
        painter.setFont(bold)
        width = self._gutter.width() - GUTTER_PADDING
        line_height = self.fontMetrics().height()

        block = self.firstVisibleBlock()
        top = round(self.blockBoundingGeometry(block).translated(self.contentOffset()).top())
        bottom = top + round(self.blockBoundingRect(block).height())
        while block.isValid() and top <= event.rect().bottom():
            if block.isVisible() and bottom >= event.rect().top():
                count = self.line_syllables(block.text())
                color = color_for_count(count)
                if color is not None:
                    painter.setPen(QColor(*color))
                    painter.drawText(0, top, width, line_height,
                                     Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter,
                                     str(count))
            block = block.next()
            top = bottom
            bottom = top + round(self.blockBoundingRect(block).height())
        painter.end()

    def _highlight_current_line(self):
        if self.isReadOnly():
            self.setExtraSelections([])
            return
        sel = QTextEdit.ExtraSelection()
        sel.format.setBackground(self._current_line_bg)
        sel.format.setProperty(QTextFormat.Property.FullWidthSelection, True)
        sel.cursor = self.textCursor()
        sel.cursor.clearSelection()
        self.setExtraSelections([sel])

    # --- word selection -------------------------------------------------------

    def _check_selection(self):
        # Still dragging: wait until the mouse is released.
        if QApplication.mouseButtons() != Qt.MouseButton.NoButton:
            self._settle.start()
            return
        picked = self.selected_whole_word()
        if picked is None:
            if self._last_word_signature is not None:
                self._last_word_signature = None
                self.selection_cleared.emit()
            return
        if picked == self._last_word_signature:
            return
        self._last_word_signature = picked
        self.word_selected.emit(*picked)

    def forget_selection(self):
        """Let the same word re-trigger (e.g. after the mode changes)."""
        self._last_word_signature = None

    def recheck_selection(self):
        self.forget_selection()
        self._check_selection()

    def selected_whole_word(self):
        """(word, start, end) if the selection is exactly one whole word."""
        cursor = self.textCursor()
        if not cursor.hasSelection():
            return None
        start, end = cursor.selectionStart(), cursor.selectionEnd()
        block = self.document().findBlock(start)
        if self.document().findBlock(end).blockNumber() != block.blockNumber():
            return None  # spans lines
        offset = block.position()
        if not is_whole_word_selection(block.text(), start - offset, end - offset):
            return None
        return block.text()[start - offset:end - offset], start, end

    def word_for_lookup(self):
        """For one-shot commands: the selection if any, else the word at the
        caret. Returns (word, start, end) or None."""
        cursor = self.textCursor()
        if cursor.hasSelection():
            text = cursor.selectedText().strip()
            if text and " " not in text:
                return text, cursor.selectionStart(), cursor.selectionEnd()
            return None
        block = cursor.block()
        span = word_span_at(block.text(), cursor.positionInBlock())
        if span is None:
            return None
        start, end = span
        return block.text()[start:end], block.position() + start, block.position() + end

    def text_range(self, start, end):
        cursor = QTextCursor(self.document())
        cursor.setPosition(start)
        cursor.setPosition(end, QTextCursor.MoveMode.KeepAnchor)
        return cursor.selectedText()

    def replace_range(self, start, end, text):
        """Replace [start, end) as one undo step and put the caret after it."""
        cursor = QTextCursor(self.document())
        cursor.beginEditBlock()
        cursor.setPosition(start)
        cursor.setPosition(end, QTextCursor.MoveMode.KeepAnchor)
        cursor.insertText(text)
        cursor.endEditBlock()
        self.forget_selection()
        self.setTextCursor(cursor)

    def cursor_rect_at(self, pos):
        cursor = QTextCursor(self.document())
        cursor.setPosition(pos)
        return self.cursorRect(cursor)
