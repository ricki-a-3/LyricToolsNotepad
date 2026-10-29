# LyricTools++

Free, open-source (MIT) Windows lyric notepad. **100% Rust** (egui/eframe +
wgpu), single self-contained exe. Released as v1.0.0 (installer + portable
zip). Successor to a Notepad++ PythonScript plugin (v2.6.0).

## Layout
- `src/core/` — lyric logic: `mod.rs` syllables/headers/tokens, `dict.rs`
  embedded CMUdict + indexes, `rhymes.rs` offline perfect/near + spelling
  fallback, `difflib.rs` exact port of Python `SequenceMatcher.ratio`,
  `datamuse.rs` Synonyms/Deep Search.
- `src/app.rs` shell: views (Home/Editor/Settings), menus, files, prompts,
  fonts, `shortcut()` (exact-modifier key matching — egui's `consume_key`
  ignores extra Shift, so Ctrl+Shift+N would also fire Ctrl+N).
- `src/editor.rs` editor view: styled layout, gutter + fold arrows, rhyme
  dropdown, formatting commands. `src/home.rs` dashboard. `src/settings_view.rs`.
- `src/markup.rs` pure markup logic (inline styles, headings, sections/folds,
  edit commands). `src/library.rs` lyrics-folder scan. `src/lookup.rs`
  background lookups. `src/settings.rs`, `src/textutil.rs`, `src/winshell.rs`
  (HKCU right-click entry; same key as the installer).
- `installer/LyricToolsPP.nsi` — NSIS, per-user, no admin.
- `tests/parity.rs` + `tests/fixtures/parity.json` — Rust core must match the
  Python reference exactly. `tools/gen_parity.py` regenerates the fixture from
  `legacy/python-prototype/` (only if core behavior changes on purpose).
- `legacy/` — the Notepad++ plugin and the PySide6 prototype. Reference only.

## Commands
- `cargo test` · `cargo clippy --release -- -D warnings` · `cargo fmt --check` (CI runs all three)
- `build_release.bat` (Windows) / `./package_from_linux.sh` → `dist/`
- Release: bump `Cargo.toml` version, tag `vX.Y.Z`, push tag → GitHub Actions.

## Behaviors to preserve (hard-won — don't regress)
1. **Syllable gutter**: per-line count, 12-color golden-angle palette (hue
   starts 0.02, += 0.618, S=0.85, V=0.95; count N → index (N-1) % 12). Only
   the number is colored. Headers (`[Verse]`, `Chorus:`, `# Title`) get none.
2. **Highlight-to-rhyme dropdown**: selecting exactly one whole word opens it
   at the word; pick replaces exactly that word (case carried over); Esc /
   click away / deselect closes it, nothing left behind.
3. **Modes**: Perfect / Near / Near + Perfect (internally `Report`, default)
   stay armed. Synonyms and Deep Search are one-shot.
4. **Never block the UI thread** — all lookups run on worker threads.
5. **Deep Search ranking is tiered**, not score-blended: offline perfect →
   offline near → Datamuse perfect → Datamuse near.
6. egui only reports the TextEdit selection while it has focus. Keep the last
   known selection when focus moves to a toolbar button (otherwise one-shot
   lookups cancel themselves).
7. Files are plain text; formatting is markers (`**b**`, `*i*`, `__u__`, `#`).

## User preferences
Concise, direct. Iterative and test-driven. Push back when an approach seems off.
Wants the app 100% Rust.
