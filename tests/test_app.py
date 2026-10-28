"""GUI tests (pytest-qt, run headless with QT_QPA_PLATFORM=offscreen)."""
import os

import pytest

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
pytest.importorskip("pytestqt")

from PySide6.QtCore import QSettings, Qt  # noqa: E402
from PySide6.QtGui import QTextCursor  # noqa: E402

from lyrictools import core, lookup  # noqa: E402
from lyrictools.mainwindow import MainWindow  # noqa: E402


@pytest.fixture
def window(qtbot, tmp_path, monkeypatch):
    monkeypatch.setattr(core, "_fetch_datamuse", lambda *a, **k: (_ for _ in ()).throw(OSError("offline")))
    settings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    win = MainWindow(settings)
    qtbot.addWidget(win)
    win.show()
    yield win
    win.editor.document().setModified(False)


def select(editor, start, end):
    cursor = editor.textCursor()
    cursor.setPosition(start)
    cursor.setPosition(end, QTextCursor.MoveMode.KeepAnchor)
    editor.setTextCursor(cursor)


def test_gutter_counts_and_headers(window):
    ed = window.editor
    ed.setPlainText("[Verse 1]\nMy bitch'll pop a cap\n\nChorus:")
    blocks = [ed.document().findBlockByNumber(i).text() for i in range(4)]
    assert [ed.line_syllables(t) for t in blocks] == [0, 6, 0, 0]


def test_highlight_word_opens_dropdown_and_pick_replaces(window, qtbot):
    ed = window.editor
    ed.setPlainText("Out late at Night, yeah")
    start = ed.toPlainText().index("Night")
    select(ed, start, start + 5)
    qtbot.waitUntil(window.popup.isVisible, timeout=15000)
    words = window.popup.words()
    assert "light" in words and "night" not in [w.lower() for w in words]

    window.popup._activate(window.popup.findItems("light", Qt.MatchFlag.MatchExactly)[0])
    assert ed.toPlainText() == "Out late at Light, yeah"   # case carried over
    ed.undo()
    assert ed.toPlainText() == "Out late at Night, yeah"   # one undo step


def test_escape_closes_and_leaves_text_alone(window, qtbot):
    ed = window.editor
    ed.setPlainText("we stay droppin' bars")
    select(ed, 8, 15)
    qtbot.waitUntil(window.popup.isVisible, timeout=15000)
    qtbot.keyClick(window.popup, Qt.Key.Key_Escape)
    assert not window.popup.isVisible()
    assert ed.toPlainText() == "we stay droppin' bars"


def test_partial_selection_does_not_open(window, qtbot):
    ed = window.editor
    ed.setPlainText("midnight")
    select(ed, 3, 8)
    qtbot.wait(600)
    assert not window.popup.isVisible()


def test_modes_persist(window, tmp_path):
    window.mode_actions[lookup.MODE_NEAR].trigger()
    assert window.mode == lookup.MODE_NEAR
    assert window.settings.value("mode") == lookup.MODE_NEAR


def test_deep_search_offline_still_shows_results(window, qtbot):
    ed = window.editor
    ed.setPlainText("night")
    select(ed, 0, 5)
    qtbot.waitUntil(window.popup.isVisible, timeout=15000)
    window.popup.hide()
    window.act_deep.trigger()
    qtbot.waitUntil(window.popup.isVisible, timeout=15000)
    assert "light" in window.popup.words()


def test_save_and_reload_round_trip(window, tmp_path):
    path = tmp_path / "song.txt"
    path.write_bytes(b"[Hook]\r\nline\r\n")
    assert window.load(str(path))
    window.editor.appendPlainText("new bar")
    assert window.editor.document().isModified()
    assert window.save()
    assert path.read_bytes() == b"[Hook]\r\nline\r\n\r\nnew bar"
    assert not window.editor.document().isModified()
    assert window._recent()[0] == str(path)
