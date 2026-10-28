# LyricTools++ — Standalone App

## Goal
Turn LyricTools++ (a Notepad++ PythonScript plugin, last released as v2.6.0)
into a standalone Windows `.exe` lyric-writing app. Free and open source (MIT),
installable by non-technical users, no dependency on Notepad++ or PythonScript.

## Repo layout
- `src/lyrictools/core.py` — pure logic, no editor dependencies: syllable
  counting, offline perfect/near rhymes (CMUdict), slang exceptions list,
  g-dropping rule (`droppin'` -> `dropping`), spelling-suffix fallback,
  Datamuse synonyms, `find_words_deep()` (tiered offline + Datamuse merge).
  Ported from the plugin's `lyrictools_core.py`; still Python 2/3 compatible
  but the app only needs to support Python 3.12+.
- `src/lyrictools/data/` — `cmudict.dict` + its license. Bundle as-is.
- `tests/` — pytest regression suite against `core`.
- `legacy/notepadpp-plugin/` — the original plugin, untouched and still
  installable via its own `Install_LyricToolsPP.bat`. Its
  `lyrictools_plugin.py` is a **behavior reference only** — don't port it
  line by line, it's Scintilla-specific glue. Its `lyrictools_core.py` is a
  separate copy; fixes to `src/lyrictools/core.py` are not mirrored there.
- `legacy/notepadpp-plugin/LyricTool/toolbar_icons/` — 256px icons, reuse
  for the app toolbar.

## Stack
- **Python 3.12+ + PySide6** (LGPL). `QPlainTextEdit` with a custom gutter
  widget for syllable counts; a popup list for the rhyme dropdown.
- **Not** PyQt6/QScintilla — GPL, which conflicts with shipping an MIT exe.
- Package with **PyInstaller** (`--onedir` first: faster startup, fewer AV
  false positives), then **Inno Setup** for a real installer.

## Behaviors to preserve (hard-won — don't regress these)
1. **Syllable gutter**: per-line count, updates on every edit. Number is
   colored from a **12-color golden-angle palette** (hue starts 0.02,
   += 0.618 per step, S=0.85, V=0.95; count N uses index (N-1) % 12). Only
   the number is colored — lyric text keeps its normal color.
2. **Highlight-to-rhyme dropdown**: fully selecting exactly one whole word
   opens a dropdown at that word; picking an entry replaces exactly that
   word; Esc / click away / deselect closes it and leaves nothing behind.
   No side panes, no second document.
3. **Modes**: Perfect / Near / Report (perfect + near, the default) are
   armed and stay armed. Synonyms and Deep Search are one-shot commands.
4. **Never block the UI thread on the network.** In the plugin, a network
   call on passive highlight froze the editor. Run all Datamuse calls on a
   worker thread; passive highlighting stays offline-only.
5. **Deep Search ranking is tiered, not score-blended**: offline perfect ->
   offline near -> Datamuse perfect -> Datamuse near. Raw scores across
   sources aren't comparable (near-rhyme scores can exceed 1000).
6. Section headers (`[Verse]`, `Chorus:`) get no syllable count — see
   `is_section_header()`.

## New app features needed
- Open / save / save-as `.txt`, recent files, unsaved-changes prompt
- Dark theme by default
- Toolbar + keyboard shortcuts for each mode, Synonyms, Deep Search

## App layout
- `app.py` entry + settings location (portable if `portable.txt` sits next
  to the exe -> `settings.ini` beside it; else per-user INI).
- `mainwindow.py` menus/toolbar/files/modes; `editor.py` QPlainTextEdit +
  gutter + whole-word selection detection (debounced 180ms, waits for mouse
  release); `popup.py` dropdown; `lookup.py` worker threads (offline and
  network executors kept separate); `textutil.py` pure helpers; `theme.py`.
- Shortcuts: Ctrl+1/2/3 modes, F7 Synonyms, F8 Deep Search, Ctrl+Space
  rhymes at caret.

## Commands
- Tests: `python -m pytest -q` (from repo root; GUI tests use pytest-qt,
  set `QT_QPA_PLATFORM=offscreen` when headless)
- Portable build (Windows): `build_portable.bat` -> `dist\LyricToolsPP\`
  + `dist\LyricToolsPP-<ver>-portable-win64.zip`
- Run from source: `run_dev.bat` or `python -m lyrictools`

## Milestones
1. ~~Scaffold, move core into a package, pytest suite passing~~
2. ~~Editor window, open/save, syllable gutter with colors~~
3. ~~Highlight-to-rhyme dropdown + mode switching~~
4. ~~Async Synonyms + Deep Search~~
5. PyInstaller portable build (`build_portable.bat`) done; Inno Setup installer next
6. README.md + GitHub release workflow

## User preferences
Concise, direct. Iterative and test-driven. Push back when an approach
seems off.
