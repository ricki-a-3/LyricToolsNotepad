# -*- coding: utf-8 -*-
from __future__ import unicode_literals
import os, sys
_SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
if _SCRIPT_DIR not in sys.path:
    sys.path.insert(0, _SCRIPT_DIR)
from Npp import editor, notepad, console
import lyrictools_plugin as lt

console.show()
console.write("\n--- LyricTools++ diagnostics ---\n")
console.write("Version: 2.6.0\n")
console.write("Syllable margin enabled: %s\n" % getattr(lt, '_enabled', None))
console.write("Callbacks registered: %s\n" % getattr(lt, '_callbacks_registered', None))
mode = getattr(lt, '_live_lookup_mode', None)
console.write("Highlight-dropdown mode: %s\n" % (lt._live_mode_title(mode) if mode else "off (run a rhyme script to arm it)"))
console.write("Current line: %s\n" % editor.lineFromPosition(editor.getCurrentPos()))
console.write("Current text: %r\n" % editor.getLine(editor.lineFromPosition(editor.getCurrentPos())).rstrip("\r\n"))

console.write("\n-- syllable margin-digit color styles --\n")
lt._styled_editors.discard(id(editor))
lt._ensure_syllable_styles(editor)
try:
    fore0 = editor.styleGetFore(lt.SYLLABLE_STYLE_BASE)
    fore1 = editor.styleGetFore(lt.SYLLABLE_STYLE_BASE + 1)
    console.write("style %d foreground (r,g,b): %s\n" % (lt.SYLLABLE_STYLE_BASE, fore0))
    console.write("style %d foreground (r,g,b): %s\n" % (lt.SYLLABLE_STYLE_BASE + 1, fore1))
    if fore0 == fore1:
        console.write("WARNING: styles look identical -- colors are not actually being applied.\n")
    else:
        console.write("Styles differ as expected -- color definitions are in place.\n")
except Exception as exc:
    console.write("Could not read styles back (styleGetFore unsupported on this build?): %s\n" % exc)

console.write("\nForcing a full refresh now (margin numbers + colors)...\n")
lt.refresh_all()
console.write("Refresh complete. Type a character; the count and color should change immediately.\n")
console.write("To test the dropdown: double-click a word in your lyrics to fully highlight it.\n")

