"""The highlight-to-rhyme dropdown.

A frameless popup list anchored under the highlighted word. Picking an entry
emits ``picked(word)``; Esc, clicking away, or typing closes it and emits
``dismissed()``. It never edits the document itself -- the main window owns
the replace, so nothing is left behind when it closes.
"""
from PySide6.QtCore import QPoint, QRect, QSize, Qt, Signal
from PySide6.QtGui import QColor, QFont, QGuiApplication
from PySide6.QtWidgets import QListWidget, QListWidgetItem, QStyledItemDelegate

from .textutil import color_for_count

WORD_ROLE = Qt.ItemDataRole.UserRole
SYLLABLE_ROLE = Qt.ItemDataRole.UserRole + 1
HEADER_ROLE = Qt.ItemDataRole.UserRole + 2
MAX_VISIBLE_ROWS = 12
POPUP_WIDTH = 260


class _ItemDelegate(QStyledItemDelegate):
    """Word on the left, its syllable count on the right in the gutter's
    color for that count, so a suggestion's fit is readable at a glance."""

    def paint(self, painter, option, index):
        if index.data(HEADER_ROLE):
            painter.save()
            font = QFont(option.font)
            font.setBold(True)
            font.setPointSizeF(font.pointSizeF() * 0.85)
            painter.setFont(font)
            painter.setPen(option.palette.placeholderText().color())
            painter.drawText(option.rect.adjusted(8, 0, -8, 0),
                             Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignBottom,
                             index.data(Qt.ItemDataRole.DisplayRole).upper())
            painter.restore()
            return

        super().paint(painter, option, index)
        syllables = index.data(SYLLABLE_ROLE) or 0
        color = color_for_count(syllables)
        if color is None:
            return
        painter.save()
        font = QFont(option.font)
        font.setBold(True)
        painter.setFont(font)
        painter.setPen(QColor(*color))
        painter.drawText(option.rect.adjusted(0, 0, -10, 0),
                         Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter,
                         str(syllables))
        painter.restore()

    def sizeHint(self, option, index):
        size = super().sizeHint(option, index)
        if index.data(HEADER_ROLE):
            return QSize(size.width(), int(size.height() * 1.3))
        return QSize(size.width(), int(size.height() * 1.15))


class SuggestionPopup(QListWidget):
    picked = Signal(str)
    dismissed = Signal()
    typed = Signal(object)  # a key event the editor should handle instead

    def __init__(self, parent=None):
        super().__init__(parent)
        self.setWindowFlags(Qt.WindowType.Popup | Qt.WindowType.FramelessWindowHint)
        self.setItemDelegate(_ItemDelegate(self))
        self.setUniformItemSizes(False)
        self.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        self.setObjectName("SuggestionPopup")
        self.itemActivated.connect(self._activate)
        self.itemClicked.connect(self._activate)
        self._chosen = False

    # --- content ------------------------------------------------------------

    def set_groups(self, groups, show_headers):
        """``groups`` is [(heading, [result dicts])]. Words already listed in
        an earlier group are skipped."""
        self.clear()
        seen = set()
        for heading, results in groups:
            fresh = [r for r in results if r["word"].lower() not in seen]
            if not fresh:
                continue
            if show_headers:
                header = QListWidgetItem(heading)
                header.setData(HEADER_ROLE, True)
                header.setFlags(Qt.ItemFlag.NoItemFlags)
                self.addItem(header)
            for result in fresh:
                seen.add(result["word"].lower())
                item = QListWidgetItem(result["word"])
                item.setData(WORD_ROLE, result["word"])
                item.setData(SYLLABLE_ROLE, result["syllables"])
                self.addItem(item)
        for row in range(self.count()):
            if self.item(row).flags() & Qt.ItemFlag.ItemIsSelectable:
                self.setCurrentRow(row)
                break
        return len(seen)

    def words(self):
        return [self.item(r).data(WORD_ROLE) for r in range(self.count())
                if self.item(r).data(WORD_ROLE)]

    # --- placement ------------------------------------------------------------

    def show_at(self, anchor_rect_global):
        """Open just below the word (or above it if there's no room)."""
        rows = min(self.count(), MAX_VISIBLE_ROWS)
        height = sum(self.sizeHintForRow(r) for r in range(rows)) + 2 * self.frameWidth() + 2
        size = QSize(POPUP_WIDTH, height)
        pos = QPoint(anchor_rect_global.left(), anchor_rect_global.bottom() + 2)
        screen = QGuiApplication.screenAt(pos) or QGuiApplication.primaryScreen()
        avail = screen.availableGeometry() if screen else QRect(0, 0, 10 ** 5, 10 ** 5)
        if pos.y() + size.height() > avail.bottom():
            pos.setY(anchor_rect_global.top() - size.height() - 2)
        pos.setX(max(avail.left(), min(pos.x(), avail.right() - size.width())))
        self.resize(size)
        self.move(pos)
        self._chosen = False
        self.show()
        self.setFocus()

    # --- interaction ----------------------------------------------------------

    def _activate(self, item):
        word = item.data(WORD_ROLE) if item else None
        if not word:
            return
        self._chosen = True
        self.hide()
        self.picked.emit(word)

    def keyPressEvent(self, event):
        key = event.key()
        if key == Qt.Key.Key_Escape:
            self.hide()
            return
        if key in (Qt.Key.Key_Return, Qt.Key.Key_Enter, Qt.Key.Key_Tab):
            self._activate(self.currentItem())
            return
        if key in (Qt.Key.Key_Up, Qt.Key.Key_Down, Qt.Key.Key_PageUp, Qt.Key.Key_PageDown,
                   Qt.Key.Key_Home, Qt.Key.Key_End):
            super().keyPressEvent(event)
            return
        # Anything else (typing, Backspace, arrows sideways...) means the user
        # went back to writing: close and hand the key to the editor.
        self.hide()
        self.typed.emit(event)

    def hideEvent(self, event):
        super().hideEvent(event)
        if not self._chosen:
            self.dismissed.emit()
        self._chosen = False
