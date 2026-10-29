# LyricTools++

A free, open-source lyric notepad for Windows, built for rap and songwriting:
live syllable counts on every line, rhymes for any word you highlight, and
titled sections you can fold away while you work on the next verse.

![Platform](https://img.shields.io/badge/platform-Windows-blue)
![License](https://img.shields.io/badge/license-MIT-lightgrey)
![Built with Rust](https://img.shields.io/badge/built%20with-Rust-orange)

## Download

Grab the latest from the **Releases** page:

- **`LyricToolsPP-<version>-setup.exe`**: the installer. Per-user, no admin
  rights needed. Adds a Start Menu shortcut and (optional) a right-click
  **Open with LyricTools++** entry for `.txt` files.
- **`LyricToolsPP-<version>-portable-win64.zip`**: unzip and run
  `LyricToolsPP.exe`. Settings stay in that folder.

The app isn't code-signed yet, so Windows SmartScreen may warn on first run:
click **More info → Run anyway**.

## What it does

- **Syllable counts**: each line's count sits in the left gutter, color-coded
  so lines that should scan the same visibly match. Knows rap spelling:
  `droppin'`, `finna`, `y'all`, `bitch'll`, and more.
- **Highlight-to-rhyme**: double-click a word for a dropdown of rhymes
  (Perfect, Near, or Near + Perfect). ↑/↓ and Enter, or click, to swap the
  word in. Esc or click away to close. Works offline.
- **Synonyms & Deep Search** (F7 / F8): online lookups via the free
  [Datamuse API](https://www.datamuse.com/api/) for when you want more options.
- **Home page**: every lyric in your lyrics folder as a card (title,
  sections, first lines, last edited), with search.
- **Sections you can fold**: `[Verse 1]`, `[Chorus]`… get a fold arrow;
  collapse what you're not working on.
- **Formatting**: bold, italic, underline and headings, stored as plain-text
  markers so your files still open anywhere:

  | Looks like | Typed as | Shortcut |
  |---|---|---|
  | **bold** | `**bold**` | Ctrl+B |
  | *italic* | `*italic*` | Ctrl+I |
  | <u>underline</u> | `__underline__` | Ctrl+U |
  | Song title | `# Song title` | Ctrl+Shift+H |
  | Section | `[Chorus]` | Ctrl+Shift+N |

## Shortcuts

| | |
|---|---|
| Ctrl+N / Ctrl+O / Ctrl+S | New lyric / Open / Save |
| Ctrl+1 / 2 / 3 | Perfect / Near / Near + Perfect rhymes |
| F6 | Rhymes for the word at the cursor |
| F7 / F8 | Synonyms / Deep Search (online) |
| Ctrl+= / Ctrl+- / Ctrl+0 | Text size |

## Building from source

Needs [Rust](https://rustup.rs).

```
cargo run              # run it
cargo test             # tests
build_release.bat      # Windows: dist\ portable zip + installer (installer needs NSIS 3)
./package_from_linux.sh  # same, cross-compiled from Linux
```

## Releasing

Bump `version` in `Cargo.toml`, commit, then tag and push:

```
git tag v1.0.1
git push origin v1.0.1
```

GitHub Actions (`.github/workflows/release.yml`) builds the installer and the
portable zip on Windows and attaches them to a new release.

## License & credits

MIT, see [`LICENSE`](LICENSE).
Bundled: the [CMU Pronouncing Dictionary](data/CMUDICT_LICENSE.txt) and
[JetBrains Mono](assets/fonts/FONT_LICENSE.txt). Built with
[egui](https://github.com/emilk/egui). The original Notepad++ plugin
lives in [`legacy/notepadpp-plugin`](legacy/notepadpp-plugin).
