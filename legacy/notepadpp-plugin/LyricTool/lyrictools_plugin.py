# -*- coding: utf-8 -*-
"""Persistent LyricTools++ plugin module for Notepad++ PythonScript.

This module owns the live syllable counter (a color-coded number in the
margin) and the highlight-to-rhyme dropdown. Launcher scripts import this
module instead of importing each other, so rhyme tools never register
duplicate callbacks or margins.

The syllable-count margin digit is colored per count; the lyric text
itself stays whatever color the theme/lexer already gives it. Two earlier
attempts at this (margin-digit styles, then a detour through text-color
indicators on the lyric text) both silently failed to show any color at
all, for the same root cause: PythonScript's colour setters
(styleSetFore/Back, indicSetFore, etc.) want a plain (r, g, b) tuple, not
a packed 0xRRGGBB/0xBBGGRR int the way raw Scintilla messages do. Passing
an int let the TYPE/style-number assignment go through fine while the
colour itself was silently dropped, defaulting to whatever's already
there. Now fixed throughout -- see _ensure_syllable_styles below.

Rhyme/synonym lookups never open a second document, pane, or window. Fully
highlighting (double-clicking or drag-selecting) a single whole word in the
editor pops open a native Scintilla autocomplete-style dropdown listing
suggestions for whichever rhyme mode is currently armed (Rhymes / Near
Rhymes / Rhyme Report). Picking an entry replaces the highlighted word.
Pressing Escape, clicking elsewhere, or de-selecting just closes it -- no
trace is left behind.

Synonyms are the one exception: they're fetched from Datamuse over the
network, so they are a one-shot lookup you trigger deliberately (run
LyricTools_Synonyms once per word) rather than something that re-fires
automatically every time you highlight a new word -- that auto-firing is
what was blocking the UI thread and freezing the editor.
"""
from __future__ import unicode_literals

import colorsys
import os
import sys

_SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
if _SCRIPT_DIR not in sys.path:
    sys.path.insert(0, _SCRIPT_DIR)

from Npp import editor, editor1, editor2, notepad, console, SCINTILLANOTIFICATION, NOTIFICATION
import lyrictools_core as core

SC_MARGIN_TEXT = 4
STYLE_DEFAULT = 32
STYLE_LINENUMBER = 33
MARGIN_INDEX = 4
MARGIN_WIDTH_PX = 42
AUTOC_SEPARATOR = 32  # ASCII space
DROPDOWN_MAX_HEIGHT = 10
DROPDOWN_RESULTS_PER_CATEGORY = 60
DEEP_SEARCH_MAX_RESULTS = 150

# Syllable-count color coding, applied to the margin digit. Styles
# 0-39 are Scintilla's own byte-style range and predefined constants
# (STYLE_DEFAULT=32 through STYLE_LASTPREDEFINED=39); 40+ is free for
# scripts, and unlike indicators, Notepad++ doesn't claim any of that
# range for its own built-in features, so there's no collision risk here.
SYLLABLE_STYLE_BASE = 40
SYLLABLE_STYLE_COUNT = 12

# Stepping the hue wheel by the golden angle (instead of evenly slicing it
# in count order) means NEIGHBOURING syllable counts -- the comparison that
# actually matters when eyeballing a verse -- land on close-to-opposite
# hues instead of near-identical ones, so every count-to-count jump reads
# as a clearly different color rather than a subtle shade shift.
_GOLDEN_ANGLE = 0.6180339887498949


def _build_syllable_palette(n):
    colors = []
    hue = 0.02
    for _ in range(n):
        r, g, b = colorsys.hsv_to_rgb(hue, 0.85, 0.95)
        colors.append((int(r * 255), int(g * 255), int(b * 255)))
        hue = (hue + _GOLDEN_ANGLE) % 1.0
    return colors


SYLLABLE_COLORS = _build_syllable_palette(SYLLABLE_STYLE_COUNT)


_enabled = False
_busy = False
_callbacks_registered = False
_last_line_count = None
_last_buffer_id = None
_last_line_signature = None

# Highlight-to-rhyme dropdown state.
_live_lookup_mode = None          # None | 'rhyme' | 'near' | 'report' | 'synonym'
_last_dropdown_signature = None
_lookup_busy = False
_dropdown_active = False
_dropdown_span = None             # (start, end) of the word the open dropdown covers
_dropdown_word = None

_styled_editors = set()  # ids of editor objects that already have the palette set up


def _npp_str(value):
    """Return the byte-string type expected by PythonScript's C++ wrappers."""
    if value is None:
        return ""
    try:
        unicode_type = unicode
    except NameError:
        unicode_type = str
    if isinstance(value, unicode_type):
        try:
            return value.encode("utf-8", "replace")
        except Exception:
            return str(value)
    return str(value)


def _notif_text(args):
    """Pull the chosen item's text out of an AUTOCSELECTION notification."""
    text = None
    try:
        text = args.text
    except AttributeError:
        try:
            text = args["text"]
        except Exception:
            text = None
    if text is None:
        return u""
    if isinstance(text, bytes):
        try:
            return text.decode("utf-8", "replace")
        except Exception:
            return u""
    return text


def _message(text, title="LyricTools++"):
    return notepad.messageBox(_npp_str(text), _npp_str(title))


def _prompt(text, title="LyricTools++", default=""):
    return notepad.prompt(_npp_str(text), _npp_str(title), _npp_str(default))


def _active_view_number():
    try:
        return int(notepad.getCurrentView())
    except Exception:
        return 0


def _active_editor():
    return editor1 if _active_view_number() == 0 else editor2


def _ensure_margin(ed):
    """Configure one fixed text margin so repeated launches cannot add columns."""
    try:
        ed.setMarginTypeN(MARGIN_INDEX, SC_MARGIN_TEXT)
        ed.setMarginWidthN(MARGIN_INDEX, MARGIN_WIDTH_PX if _enabled else 0)
        try:
            ed.setMarginSensitiveN(MARGIN_INDEX, False)
        except Exception:
            pass
    except Exception as exc:
        console.show()
        console.write("LyricTools++ margin setup error: %s\n" % exc)
        raise
    # Style definitions have to land on whichever object actually performs
    # the per-line marginSetStyle() call -- that's always the bare `editor`
    # active-document proxy (see _set_margin_text), which is not guaranteed
    # to be the same underlying object as editor1/editor2 in every
    # PythonScript build. Configure both to be safe; it's cheap and only
    # runs once per object thanks to the _styled_editors cache.
    _ensure_syllable_styles(ed)
    _ensure_syllable_styles(editor)


def _ensure_syllable_styles(ed):
    """Define the per-syllable-count colour styles once per editor view.

    Copies STYLE_LINENUMBER's background (falling back to STYLE_DEFAULT's)
    so the coloured digits still sit on whatever margin background the
    user's theme already uses; only the foreground colour (and bold)
    changes per count.
    """
    key = id(ed)
    if key in _styled_editors:
        return
    back = None
    for fallback_style in (STYLE_LINENUMBER, STYLE_DEFAULT):
        try:
            back = ed.styleGetBack(fallback_style)
            if back is not None:
                break
        except Exception:
            continue
    for i, color in enumerate(SYLLABLE_COLORS):
        style_num = SYLLABLE_STYLE_BASE + i
        try:
            ed.styleSetFore(style_num, color)
            if back is not None:
                ed.styleSetBack(style_num, back)
            ed.styleSetBold(style_num, True)
        except Exception as exc:
            try:
                console.write("LyricTools++ syllable style warning: %s\n" % exc)
            except Exception:
                pass
    _styled_editors.add(key)


def _line_text(line_num):
    try:
        return editor.getLine(line_num).rstrip("\r\n")
    except Exception:
        return ""


def _set_margin_text(line_num, label, style_num):
    editor.marginSetText(line_num, label)
    try:
        editor.marginSetStyle(line_num, style_num if style_num is not None else STYLE_LINENUMBER)
    except Exception:
        pass


def _style_for_count(count):
    if count <= 0:
        return None
    idx = (count - 1) % SYLLABLE_STYLE_COUNT
    return SYLLABLE_STYLE_BASE + idx


def _line_label_and_style(text):
    stripped = text.strip()
    if not stripped or core.is_section_header(text):
        return "", None
    count = core.syllables_for_line(text)
    return str(count), _style_for_count(count)


def _render_line(line_num):
    if not _enabled:
        return
    if line_num < 0 or line_num >= int(editor.getLineCount()):
        return
    label, style_num = _line_label_and_style(_line_text(line_num))
    _set_margin_text(line_num, label, style_num)




def _current_buffer_id():
    try:
        return int(notepad.getCurrentBufferID())
    except Exception:
        return -1


def _reset_update_cache():
    global _last_line_count, _last_buffer_id, _last_line_signature
    _last_line_count = None
    _last_buffer_id = None
    _last_line_signature = None


def refresh_all():
    global _busy, _last_line_count, _last_buffer_id, _last_line_signature
    if not _enabled or _busy:
        return
    _busy = True
    try:
        _ensure_margin(_active_editor())
        editor.marginTextClearAll()
        line_count = int(editor.getLineCount())
        for line_num in range(line_count):
            label, style_num = _line_label_and_style(_line_text(line_num))
            _set_margin_text(line_num, label, style_num)
        editor.setMarginWidthN(MARGIN_INDEX, MARGIN_WIDTH_PX)
        _last_line_count = line_count
        _last_buffer_id = _current_buffer_id()
        pos = int(editor.getCurrentPos())
        line_num = int(editor.lineFromPosition(pos))
        _last_line_signature = (line_num, _line_text(line_num))
    finally:
        _busy = False


def _refresh_current_line(force=False):
    """Fast live refresh used while typing, deleting, undoing and pasting."""
    global _busy, _last_line_count, _last_buffer_id, _last_line_signature
    if not _enabled or _busy:
        return

    buffer_id = _current_buffer_id()
    line_count = int(editor.getLineCount())

    # Newline insertion/deletion, buffer switch, or multi-line paste: safest to
    # rebuild because Scintilla shifts line-indexed margin text with the edit.
    if (_last_buffer_id is None or buffer_id != _last_buffer_id or
            _last_line_count is None or line_count != _last_line_count):
        refresh_all()
        return

    pos = int(editor.getCurrentPos())
    line_num = int(editor.lineFromPosition(pos))
    text = _line_text(line_num)
    signature = (line_num, text)
    if not force and signature == _last_line_signature:
        return

    _busy = True
    try:
        _ensure_margin(_active_editor())
        # Refresh the active line plus its neighbor. This covers joins/splits
        # and edits made at a line boundary without recomputing the whole file.
        _render_line(line_num)
        if line_num > 0:
            _render_line(line_num - 1)
        if line_num + 1 < line_count:
            _render_line(line_num + 1)
        _last_line_signature = signature
        _last_line_count = line_count
        _last_buffer_id = buffer_id
    finally:
        _busy = False


def _on_char_added(args):
    _refresh_current_line(force=True)


def _on_modified(args):
    # Handles backspace/delete, paste, cut, undo/redo, and programmatic edits.
    _refresh_current_line(force=True)


def _on_update_ui(args):
    # Safety net for older PythonScript/Scintilla builds where MODIFIED can be
    # inconsistent. Cached signatures make this very cheap when nothing changed.
    _refresh_current_line(force=False)
    _maybe_show_word_dropdown()


def _close_dropdown():
    global _dropdown_active
    if _dropdown_active:
        try:
            if editor.autoCActive():
                editor.autoCCancel()
        except Exception:
            pass
    _dropdown_active = False


def _on_buffer_activated(args):
    global _last_dropdown_signature
    _close_dropdown()
    _last_dropdown_signature = None
    if not _enabled:
        return
    _reset_update_cache()
    try:
        _ensure_margin(_active_editor())
    except Exception:
        pass
    refresh_all()


def _on_autoc_selection(args):
    """Replace the highlighted word ourselves instead of Scintilla's default insert.

    Cancelling the list here (inside the notification) stops Scintilla from
    doing its own insertion afterwards, so we get full control over exactly
    which text gets replaced.
    """
    global _dropdown_active, _dropdown_span, _last_dropdown_signature

    span = _dropdown_span
    was_active = _dropdown_active

    try:
        editor.autoCCancel()
    except Exception:
        pass
    _dropdown_active = False
    _last_dropdown_signature = None

    if not was_active or span is None:
        return

    text = _notif_text(args)
    if not text:
        return

    start, end = span
    npp_text = _npp_str(text)
    try:
        editor.setSel(start, end)
        editor.replaceSel(npp_text)
        editor.gotoPos(start + len(npp_text))
    except Exception as exc:
        try:
            console.write("LyricTools++ replace warning: %s\n" % exc)
        except Exception:
            pass


def _on_autoc_cancelled(args):
    global _dropdown_active, _last_dropdown_signature
    _dropdown_active = False
    _last_dropdown_signature = None


def _register_callbacks():
    """Register live hooks on PythonScript's active-editor proxy.

    PythonScript routes Scintilla notifications through the special
    ``editor`` proxy.  Registering callbacks on editor1/editor2 can appear to
    succeed but not receive normal edit notifications on some builds.
    """
    global _callbacks_registered
    if _callbacks_registered:
        return

    ok = True

    # UPDATEUI is the primary live hook.  It fires after typing, deletion,
    # paste, undo/redo and caret/selection updates, so document text and the
    # selection are already in their new state when we read them.  The
    # signature caches make no-change UPDATEUI notifications essentially free.
    try:
        result = editor.callback(_on_update_ui, [SCINTILLANOTIFICATION.UPDATEUI])
        if result is False:
            ok = False
    except Exception as exc:
        console.write("LyricTools++ UPDATEUI callback warning: %s\n" % exc)
        ok = False

    # MODIFIED is a backup for builds where UPDATEUI behaviour differs, and
    # catches programmatic edits.  _busy/cache prevents duplicate work.
    try:
        result = editor.callback(_on_modified, [SCINTILLANOTIFICATION.MODIFIED])
        if result is False:
            ok = False
    except Exception as exc:
        console.write("LyricTools++ MODIFIED callback warning: %s\n" % exc)
        ok = False

    # CHARADDED gives immediate feedback for ordinary typing on old builds.
    try:
        result = editor.callback(_on_char_added, [SCINTILLANOTIFICATION.CHARADDED])
        if result is False:
            ok = False
    except Exception as exc:
        console.write("LyricTools++ CHARADDED callback warning: %s\n" % exc)
        ok = False

    try:
        result = notepad.callback(_on_buffer_activated, [NOTIFICATION.BUFFERACTIVATED])
        if result is False:
            ok = False
    except Exception as exc:
        console.write("LyricTools++ buffer callback warning: %s\n" % exc)
        ok = False

    # AUTOCSELECTION/AUTOCCANCELLED drive the highlight-to-rhyme dropdown.
    try:
        result = editor.callback(_on_autoc_selection, [SCINTILLANOTIFICATION.AUTOCSELECTION])
        if result is False:
            ok = False
    except Exception as exc:
        console.write("LyricTools++ AUTOCSELECTION callback warning: %s\n" % exc)
        ok = False

    try:
        result = editor.callback(_on_autoc_cancelled, [SCINTILLANOTIFICATION.AUTOCCANCELLED])
        if result is False:
            ok = False
    except Exception as exc:
        console.write("LyricTools++ AUTOCCANCELLED callback warning: %s\n" % exc)
        ok = False

    _callbacks_registered = True
    console.write("LyricTools++ v2.6.0: live syllables + highlight-to-rhyme dropdown callbacks registered.\n")
    if not ok:
        console.show()
        console.write("LyricTools++: one or more live callbacks could not be registered.\n")


def enable(show_message=False):
    global _enabled, _live_lookup_mode
    _enabled = True
    if _live_lookup_mode is None:
        # Sensible default so highlighting a word works immediately without
        # having to run one of the mode-picking scripts first.
        _live_lookup_mode = "report"
    _register_callbacks()
    _reset_update_cache()
    _ensure_margin(editor1)
    _ensure_margin(editor2)
    refresh_all()
    if show_message:
        _message("Live syllable counts are ON.")


def disable(show_message=False):
    global _enabled
    _enabled = False
    for ed in (editor1, editor2):
        try:
            ed.marginTextClearAll()
            ed.setMarginWidthN(MARGIN_INDEX, 0)
        except Exception:
            pass
    _reset_update_cache()
    if show_message:
        _message("Live syllable counts are OFF.")


def toggle_syllables():
    if _enabled:
        disable(show_message=True)
    else:
        enable(show_message=True)


def count_current_line():
    pos = editor.getCurrentPos()
    line_num = editor.lineFromPosition(pos)
    text = _line_text(line_num)
    count = core.syllables_for_line(text)
    _message(
        "Line %d: %d syllable%s" % (line_num + 1, count, "" if count == 1 else "s")
    )
    return count


def _normalize_lookup_word(word):
    try:
        return word.lower().replace(u"\u2019", u"'")
    except Exception:
        return word


def _live_mode_title(mode):
    if mode == "rhyme":
        return u"Perfect Rhymes"
    if mode == "near":
        return u"Near / Slant Rhymes"
    if mode == "report":
        return u"Perfect + Near Rhymes"
    if mode == "synonym":
        return u"Synonyms"
    if mode == "deep":
        return u"Deep Search (offline + Datamuse)"
    return u"Rhymes"


def _collect_dropdown_words(word, mode):
    """Look up suggestions for the dropdown. Rhyme modes are offline/local;
    synonyms and deep search are the two that touch the network."""
    if mode == "rhyme":
        results = core.find_words_local(word, mode="rhyme", max_results=DROPDOWN_RESULTS_PER_CATEGORY)
        return sorted(set(item["word"] for item in results))
    if mode == "near":
        results = core.find_words_local(word, mode="near", max_results=DROPDOWN_RESULTS_PER_CATEGORY)
        return sorted(set(item["word"] for item in results))
    if mode == "report":
        perfect = core.find_words_local(word, mode="rhyme", max_results=DROPDOWN_RESULTS_PER_CATEGORY)
        near = core.find_words_local(word, mode="near", max_results=DROPDOWN_RESULTS_PER_CATEGORY)
        combined = set(item["word"] for item in perfect) | set(item["word"] for item in near)
        return sorted(combined)
    if mode == "synonym":
        results = core.find_words(word, mode="synonym", max_results=DROPDOWN_RESULTS_PER_CATEGORY, timeout=6)
        return sorted(set(item["word"] for item in results))
    if mode == "deep":
        results = core.find_words_deep(word, max_results=DEEP_SEARCH_MAX_RESULTS, timeout=6)
        return sorted(set(item["word"] for item in results))
    return []


def _show_word_dropdown(word, start, end, mode):
    """Pop a native Scintilla autocomplete-style dropdown over the highlighted word."""
    global _dropdown_active, _dropdown_span, _dropdown_word

    try:
        words = _collect_dropdown_words(word, mode)
    except Exception as exc:
        try:
            console.write("LyricTools++ dropdown lookup warning: %s\n" % exc)
        except Exception:
            pass
        return

    if not words:
        # Nothing found offline for this word -- leave the editor untouched,
        # no popup, no message box.
        _close_dropdown()
        return

    item_string = u" ".join(words)

    try:
        if editor.autoCActive():
            editor.autoCCancel()
        editor.autoCSetSeparator(AUTOC_SEPARATOR)
        editor.autoCSetIgnoreCase(True)
        editor.autoCSetOrder(1)  # let Scintilla sort/filter safely
        editor.autoCSetMaxHeight(DROPDOWN_MAX_HEIGHT)
        editor.autoCShow(0, _npp_str(item_string))
    except Exception as exc:
        try:
            console.write("LyricTools++ dropdown display warning: %s\n" % exc)
        except Exception:
            pass
        return

    _dropdown_active = True
    _dropdown_span = (start, end)
    _dropdown_word = word


def _maybe_show_word_dropdown():
    """Show the rhyme dropdown only when exactly one whole word is highlighted
    in the editor -- not on a bare caret, not on a partial-word or multi-word
    selection.

    Synonyms are deliberately excluded here: they are web-backed, and firing
    a blocking network request on every casual highlight (the way the
    offline rhyme modes safely do) is what was freezing the editor. Synonyms
    are a one-shot lookup instead -- see synonyms() below.
    """
    global _last_dropdown_signature, _lookup_busy

    if not _live_lookup_mode or _live_lookup_mode == "synonym" or _lookup_busy:
        return

    try:
        sel_start = int(editor.getSelectionStart())
        sel_end = int(editor.getSelectionEnd())
    except Exception:
        return

    if sel_start == sel_end:
        # No highlight -- close any dropdown we opened and wait for the next one.
        _close_dropdown()
        _last_dropdown_signature = None
        return

    lo = min(sel_start, sel_end)
    hi = max(sel_start, sel_end)

    try:
        word_start = int(editor.wordStartPosition(lo, True))
        word_end = int(editor.wordEndPosition(lo, True))
    except Exception:
        return

    if word_start != lo or word_end != hi:
        # Selection doesn't line up with exactly one whole word; ignore it.
        return

    try:
        word = editor.getTextRange(lo, hi).strip()
    except Exception:
        word = u""
    if not word:
        return

    signature = (_current_buffer_id(), lo, hi, _normalize_lookup_word(word), _live_lookup_mode)
    if signature == _last_dropdown_signature:
        return

    _last_dropdown_signature = signature
    _lookup_busy = True
    try:
        _show_word_dropdown(word, lo, hi, _live_lookup_mode)
    finally:
        _lookup_busy = False


def _arm_mode(mode):
    """Pick which category the highlight-dropdown shows from now on. If a
    whole word already happens to be highlighted, show it immediately."""
    global _live_lookup_mode, _last_dropdown_signature
    _live_lookup_mode = mode
    _last_dropdown_signature = None
    _maybe_show_word_dropdown()


def rhymes():
    _arm_mode("rhyme")


def near_rhymes():
    _arm_mode("near")


def rhyme_report():
    _arm_mode("report")


def _word_for_manual_lookup(title):
    """Shared by one-shot commands (Synonyms, Deep Search): use the
    currently highlighted word, or the word under the caret if nothing is
    highlighted, or prompt for one if neither is available. Returns
    (word, start, end) or None if the user cancelled the prompt."""
    try:
        sel_start = int(editor.getSelectionStart())
        sel_end = int(editor.getSelectionEnd())
    except Exception:
        return None

    if sel_start == sel_end:
        pos = int(editor.getCurrentPos())
        start = int(editor.wordStartPosition(pos, True))
        end = int(editor.wordEndPosition(pos, True))
    else:
        start = min(sel_start, sel_end)
        end = max(sel_start, sel_end)

    try:
        word = editor.getTextRange(start, end).strip()
    except Exception:
        word = u""
    if not word:
        word = _prompt("Word:", title, "")
        if not word:
            return None
        start = end = int(editor.getCurrentPos())

    return word, start, end


def synonyms():
    """One-shot synonym lookup for the currently highlighted word (or the
    word under the caret if nothing is highlighted).

    Unlike the rhyme modes, this deliberately does NOT stay armed for future
    highlights -- Synonyms are web-backed, and repeatedly firing a blocking
    network request every time you highlight a different word froze the
    editor. Run this once per word, whenever you actually want synonyms for
    it (menu, or a toolbar button once you've added one).
    """
    picked = _word_for_manual_lookup("LyricTools++ Synonyms")
    if picked is None:
        return
    word, start, end = picked
    _show_word_dropdown(word, start, end, "synonym")


def deep_search():
    """One-shot MAXIMUM-coverage rhyme lookup for the currently highlighted
    word (or the word under the caret if nothing is highlighted).

    Combines the full offline engine with Datamuse's dedicated perfect and
    near rhyme relations for the biggest possible pool of suggestions --
    built for when the always-on offline dropdown doesn't turn up enough
    on its own. Like Synonyms, this is deliberately a one-shot action you
    trigger yourself; it is NOT wired into the automatic highlight-trigger,
    so a slow or unreachable network can never stall ordinary typing.
    """
    picked = _word_for_manual_lookup("LyricTools++ Deep Search")
    if picked is None:
        return
    word, start, end = picked
    _show_word_dropdown(word, start, end, "deep")
