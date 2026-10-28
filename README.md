# LyricTools++

A free, open-source Windows app for rap and lyric writing: live per-line
syllable counts and highlight-to-rhyme suggestions.

> **Status:** the standalone app (v3) is in development. The original
> Notepad++ plugin (v2.6.0) still works and lives in
> [`legacy/notepadpp-plugin`](legacy/notepadpp-plugin) with its own
> installer and README.

## Build a portable exe (Windows)

Needs Python 3.12+ from python.org. From the repo folder:

```
build_portable.bat
```

Output: `dist\LyricToolsPP\LyricToolsPP.exe` (run it in place) and a zip
of that folder to share. The portable copy keeps its settings in
`settings.ini` next to the exe.

## Using it

- Type lyrics; each line's syllable count shows in the left gutter.
- Double-click (or drag-select) one word to get rhymes in a dropdown.
  Enter/click replaces the word; Esc or clicking away closes it.
- **Ctrl+1 / 2 / 3** — Perfect / Near / Report (both) rhyme mode.
- **F7** Synonyms, **F8** Deep Search (online, one-shot).
- **Ctrl+Space** — rhymes for the word at the caret.

## Development

```
pip install -e ".[dev]"
python -m pytest -q
python -m lyrictools
```

## License

MIT — see [`LICENSE`](LICENSE). The bundled CMU Pronouncing Dictionary is
separately licensed — see `src/lyrictools/data/CMUDICT_LICENSE.txt`.
