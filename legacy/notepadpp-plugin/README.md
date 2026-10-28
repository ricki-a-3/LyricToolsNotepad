# LyricTools++

A Notepad++ plugin for rap/lyric songwriting: live syllable counts and
rhyme/synonym suggestions, right in your editor, with no extra windows or
panes cluttering your workspace.

![Platform](https://img.shields.io/badge/platform-Windows-blue)
![Requires](https://img.shields.io/badge/requires-Notepad%2B%2B%20%2B%20PythonScript-green)
![License](https://img.shields.io/badge/license-MIT-lightgrey)

## Overview

Notepad++ is a fast, no-frills text editor — which is exactly why a lot of
writers use it for lyrics instead of a bloated word processor. But it has
no idea what a syllable is, and it has no concept of rhyme. Getting that
kind of feedback normally means alt-tabbing out to a rhyming dictionary
website or a separate app, breaking your train of thought every time you
want to check a line. LyricTools++ exists to close that gap without
turning Notepad++ into something it isn't.

The plugin does two things, both built to stay out of your way:

1. **It counts syllables as you type.** Every line gets a live count in
   the margin, color-coded so your eye can catch a mismatched bar without
   having to read the numbers one by one — two lines that are supposed to
   scan the same way will visibly match, and one that doesn't will
   visibly stand out. This updates on every keystroke, not just when you
   ask for it.

2. **It suggests rhymes and synonyms on demand.** Fully highlight a word
   — double-click it, or drag-select it — and a dropdown of suggestions
   appears right at that spot, the same way code autocomplete would. Pick
   one and it replaces the word; hit Escape or click elsewhere and it's
   gone with no trace. There's no persistent panel to manage, no second
   document cluttering your tab bar, and no separate window competing for
   screen space. You can switch between three rhyme modes (perfect, near,
   or both combined) depending on how strict you want the matches to be,
   and pull up synonyms on the side whenever you need them.

Everything that can run offline does: syllable counting and both rhyme
modes are built entirely from a bundled pronunciation dictionary, so
there's no lag and no dependency on an internet connection for the core
writing flow. The two features that do reach out to the network
(synonyms, and an optional "Deep Search" for when you want the widest
possible rhyme pool) are deliberately kept as manual, one-at-a-time
lookups rather than something that fires automatically, so a slow
connection can never stall you mid-verse.

It also understands the way rap and hip-hop actually get written, not
just how a dictionary thinks English is spoken — contractions like
`bitch'll`, `finna`, `gotta`, and `y'all` count correctly instead of
tripping up a plain syllable-counting algorithm.

## Features

- **Live syllable counter** — a running count in the margin next to every
  line, colored per count (12 deliberately spread-out colors, so a 6 and a
  7 next to each other are obviously different, not just a slightly
  different shade).
- **Highlight-to-rhyme dropdown** — fully highlight (double-click, or
  drag-select) any word in your lyrics and a native dropdown pops up right
  there with suggestions. Pick one to swap it in, or press Escape to
  dismiss. No sidebar, no second document, no floating window — nothing is
  left behind when you're done.
- **Three rhyme modes** — Perfect rhymes, Near/slant rhymes, or both
  combined, all generated **offline** from the bundled CMU Pronouncing
  Dictionary, so looking up a word never makes a network request.
- **Synonyms** — a one-shot lookup (Datamuse API) for whenever you want
  them, kept separate from the offline modes so it can't slow anything
  down while you're writing.
- **Deep Search** — another one-shot, on-demand lookup: combines the
  offline engine with Datamuse's dedicated perfect- and near-rhyme data
  for the biggest possible pool of suggestions on a single word, for when
  the always-on offline dropdown doesn't turn up enough on its own. Same
  deliberate, non-automatic design as Synonyms, for the same reason.
- **Slang-aware, and it extends itself** — handles rap-specific
  contractions (`bitch'll`, `finna`, `tryna`, `y'all`, etc.) that a plain
  dictionary lookup gets wrong, and automatically maps any `-in'`/`-in`
  verb (`droppin'`, `spittin`, `flexin`...) to its dictionary form for
  both syllable counts and rhymes — one rule that covers that entire
  pattern, not a list that only knows the words someone thought to add.
  Anything still unrecognized falls back to a spelling-based rhyme guess
  instead of coming up empty.

## Requirements

- Windows (Notepad++'s PythonScript plugin is Windows-only)
- [Notepad++](https://notepad-plus-plus.org/)
- The **PythonScript** plugin for Notepad++ — if you don't have it yet,
  the installer below will tell you exactly how to get it

## Installation

### Option A — Automatic installer (recommended)

1. Go to the [Releases](../../releases) page (or download this repo as a
   ZIP via the green **Code** button) and unzip it somewhere on your
   computer.
2. Double-click **`Install_LyricToolsPP.bat`**.
   - Windows will likely show a blue **"Windows protected your PC"**
     screen the first time. This is normal for any script that isn't
     signed by a large company — it doesn't mean anything is wrong.
     Click **More info**, then **Run anyway**.
3. The installer will:
   - Look for your Notepad++ PythonScript folder in the usual places
     (per-user AppData, Program Files, Program Files (x86), and the
     newer per-user LocalAppData install location) — you don't need to
     know or type any paths yourself.
   - If it can't find PythonScript at all, it'll print the exact steps
     to install that plugin first, then ask you to run the installer
     again.
   - Copy the `LyricTool` folder into place (replacing any older version
     it finds).
   - Ask one question: **start LyricTools++ automatically every time
     Notepad++ opens?** Press Enter to accept the default (yes) — this
     adds a small, clearly-marked block to PythonScript's `startup.py`
     so you never have to remember to turn it on. Running the installer
     again won't duplicate this block.
4. When it says **Done!**, close Notepad++ if it was open, then start it
   again.
5. Fully highlight (double-click) any word in your lyrics — a dropdown of
   rhymes should appear.

If anything goes wrong, the installer prints what happened rather than
failing silently. Common causes:

| Symptom | Fix |
|---|---|
| "Something went wrong copying the files" | Close Notepad++ completely (a script running inside it can lock files) and run the installer again. |
| Still fails after closing Notepad++ | Right-click `Install_LyricToolsPP.bat` → **Run as administrator**. |
| "Could not find a PythonScript folder" | The PythonScript plugin isn't installed yet. Open Notepad++ → **Plugins** → **Plugins Admin** → search "PythonScript" → check its box → **Install** → let Notepad++ restart → run the installer again. |

See [`HOW_TO_INSTALL.txt`](HOW_TO_INSTALL.txt) for the same steps written
for someone who's never run a `.bat` file before.

### Option B — Manual installation

If you'd rather do it by hand, or the installer isn't working for you:

1. Close Notepad++ completely.
2. Find your PythonScript **scripts** folder. It's one of:
   ```
   %AppData%\Notepad++\plugins\config\PythonScript\scripts
   %ProgramFiles%\Notepad++\plugins\PythonScript\scripts
   %ProgramFiles(x86)%\Notepad++\plugins\PythonScript\scripts
   %LocalAppData%\Programs\Notepad++\plugins\PythonScript\scripts
   ```
   (You'll know you're in the right place if you see a `Samples` folder
   there.)
3. Copy this repo's `LyricTool` folder directly into that `scripts`
   folder, next to `Samples` — **not** into a subfolder.
4. If you also want it to start automatically, open (or create)
   `startup.py` in that same `scripts` folder and paste in the contents
   of [`_lyrictools_autostart_snippet.py`](_lyrictools_autostart_snippet.py)
   from this repo.
5. Start Notepad++.
6. Run **Plugins → PythonScript → Scripts → LyricTool → LyricToolsPP**
   once. (Skip this if you set up autostart in step 4.)

Full details, including how to switch which rhyme mode the dropdown uses,
are in [`INSTALL_FOR_YOUR_NOTEPAD.txt`](INSTALL_FOR_YOUR_NOTEPAD.txt) and
[`LyricTool/README.txt`](LyricTool/README.txt).

### Updating to a newer version

Just run the installer again — it detects and replaces the old
`LyricTool` folder automatically. If you're doing it manually, delete the
old `LyricTool` folder before copying in the new one (an old
`__pycache__` subfolder inside it can otherwise cause confusing errors).

### Uninstalling

1. Delete the `LyricTool` folder from your PythonScript `scripts` folder
   (see the paths listed under Option B above).
2. If you enabled autostart, open `startup.py` in that same folder and
   delete the block between the `# --- LyricTools++ autostart ---` and
   `# --- end LyricTools++ autostart ---` comments.

## Usage

- **Syllable counter**: turns on automatically once `LyricToolsPP` has
  run (either at startup, or manually from the Scripts menu). Toggle it
  off/on with `LyricTools_ToggleSyllables`.
- **Rhyme dropdown**: pick a mode once by running `LyricTools_Rhymes`
  (perfect only), `LyricTools_NearRhymes` (near/slant only), or
  `LyricTools_RhymeReport` (both combined — this is the default). After
  that, every time you fully highlight a word, the dropdown shows that
  mode's suggestions.
- **Synonyms**: highlight a word, then run `LyricTools_Synonyms`. Unlike
  the rhyme modes, this doesn't stay "on" — run it again each time you
  want synonyms for a different word (it's one of two modes that use the
  internet, so it's kept deliberately manual).
- **Deep Search**: highlight a word, then run `LyricTools_DeepSearch` for
  the widest possible pool of rhymes — offline results plus Datamuse's
  perfect- and near-rhyme data merged together. Same deal as Synonyms:
  run it again per word, it doesn't stay armed.
- **Line syllable count as a popup**: `LyricTools_CountLine` shows a
  message box with the current line's count.
- **Toolbar icons**: a matching set is bundled in
  [`LyricTool/toolbar_icons`](LyricTool/toolbar_icons) for assigning to
  Notepad++'s toolbar via Plugins → PythonScript → Configuration.
  *(Fuller walkthrough coming soon.)*

## How it works (for the curious)

- Syllable counting and rhyme-finding are pure offline logic in
  [`lyrictools_core.py`](LyricTool/lyrictools_core.py) — it loads the
  bundled CMU Pronouncing Dictionary once, builds a rhyme index from it,
  then layers a hand-picked slang list, a rule that normalizes `-in'`
  verb spellings back to their dictionary form, and a spelling-based
  fallback on top, in that order, for anything the dictionary alone
  doesn't recognize.
- Deep Search merges that same offline result set with Datamuse's
  `rel_rhy`/`rel_nry` relations. The two sources use unrelated scoring
  scales (and near-rhyme's own similarity bonuses can even score higher
  than a flat perfect-rhyme match), so results are ranked by explicit
  source tier — offline perfect, then offline near, then Datamuse perfect,
  then Datamuse near — rather than by comparing raw scores directly,
  which would let a lower-confidence match outrank a verified one.
- Everything editor-facing (the margin, the dropdown, live callbacks) is
  in [`lyrictools_plugin.py`](LyricTool/lyrictools_plugin.py), built on
  [PythonScript](https://github.com/bruderstein/PythonScript)'s Scintilla
  bindings.
- The margin's syllable-count color comes from Scintilla per-line text
  styles; picking a word from the dropdown replaces it by intercepting
  the autocomplete-selection event rather than letting Scintilla insert
  it automatically, which is what makes it possible to replace exactly
  the highlighted word.

## Known limitations

- Windows only — this is inherent to Notepad++'s PythonScript plugin.
- Rhymes are built from the bundled CMU Pronouncing Dictionary (~134,000
  words) plus two offline fallbacks: a rule that maps informal "-in'"
  verb spellings to their dictionary form (`droppin'` → `dropping`), and
  a last-resort spelling-based match for words with no phonetic data at
  all. Real pronunciation data still beats spelling guesses, so very
  obscure slang with no dictionary neighbor may get thinner or
  lower-confidence suggestions than a common word would — that's what
  Deep Search is for.
- Synonyms and Deep Search both require an internet connection (Datamuse
  API); everything else works fully offline.

## Credits

- Rhymes and syllable counts are built on the
  [CMU Pronouncing Dictionary](http://www.speech.cs.cmu.edu/cgi-bin/cmudict)
  (Carnegie Mellon University) — license in
  [`LyricTool/data/CMUDICT_LICENSE.txt`](LyricTool/data/CMUDICT_LICENSE.txt).
- Synonyms and Deep Search via the
  [Datamuse API](https://www.datamuse.com/api/).
- Built on [PythonScript](https://github.com/bruderstein/PythonScript) for
  Notepad++.

## License

This project is licensed under the MIT License — see
[`LICENSE`](LICENSE) for the full text. The bundled CMU Pronouncing
Dictionary is separately licensed — see the credits section above.
