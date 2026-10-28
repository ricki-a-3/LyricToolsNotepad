# LyricTools++ (Rust)

The 100% Rust rewrite of LyricTools++: a lyric notepad with live per-line
syllable counts and highlight-to-rhyme suggestions. One self-contained
`LyricToolsPP.exe` (~18 MB) — the pronunciation dictionary is compiled in.

## Run / build

| What | Command (from `rust/`) |
|---|---|
| Run from source | `cargo run` |
| Tests | `cargo test` |
| Portable zip on Windows | `build_portable.bat` |
| Portable zip from Linux (cross-compile) | `./package_from_linux.sh` |

Output: `dist/LyricToolsPP-<version>-portable-win64/` + `.zip`. The
`portable.txt` marker beside the exe keeps settings in `settings.json` in
that folder; without it, settings go to `%APPDATA%\LyricToolsPP\`.

## Using it

- Type lyrics; each line's syllable count shows in the gutter, colored so
  matching bars match visually. Section headers (`[Verse]`, `Chorus:`) get no count.
- Double-click (or drag-select) one whole word for rhymes in a dropdown.
  ↑/↓ + Enter (or click) replaces the word; Esc / click away closes it.
- **Ctrl+1 / 2 / 3** — Perfect / Near / Report (both) rhyme mode.
- **F6** rhymes for the word at the caret, **F7** Synonyms, **F8** Deep
  Search (F7/F8 go online to Datamuse).
- **Ctrl+= / Ctrl+- / Ctrl+0** — text size. Drag a file onto the window to open it.

## Layout

- `src/core/` — lyric logic, a behavioral port of the Python
  `lyrictools.core`: `mod.rs` (syllables, headers, tokens), `dict.rs`
  (embedded CMUdict + indexes), `rhymes.rs` (offline perfect/near +
  spelling fallback), `difflib.rs` (exact port of Python's
  `SequenceMatcher.ratio`), `datamuse.rs` (Synonyms, Deep Search).
- `src/app.rs` — egui window: editor + gutter, dropdown, menus, files.
- `src/lookup.rs` — background lookups (never on the UI thread).
- `src/textutil.rs`, `src/settings.rs` — helpers, settings.
- `tests/parity.rs` — checks the Rust core against
  `tests/fixtures/parity.json`, produced by the Python core via
  `tools/gen_parity.py` (dev-time only; regenerate if core behavior is
  changed on purpose).
