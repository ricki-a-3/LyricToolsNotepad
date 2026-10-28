LyricTools++ v2.6.0 - HIGHLIGHT-TO-RHYME DROPDOWN
==================================================

v2.6.0 addition: new LyricTools_DeepSearch command for maximum rhyme
coverage. Combines the full offline engine (CMUdict + slang list +
g-dropping + spelling fallback) with Datamuse's dedicated perfect-rhyme
and near-rhyme relations, which draw from a much larger word list than
CMUdict alone. Highlight a word, run it once, get the biggest pool this
tool can produce for that word.

Deliberately built the same way Synonyms is, not the way the automatic
highlight-trigger works: it's a one-shot command you run yourself, never
wired into passive highlighting. That keeps the instant offline dropdown
exactly as reliable as it's always been for everyday writing -- Deep
Search is the tool you reach for specifically when you want the widest
possible net and don't mind a moment's wait.

v2.5.0 change: much wider rhyme/syllable coverage for slang, still fully
offline. Three additions, in the order they're tried:
1. The informal-word exceptions list grew a lot (contractions with and
   without apostrophes, dropped consonants, "to/have/of" reductions like
   shoulda/coulda/outta).
2. New: any "-in'"/"-in" verb (droppin', spittin, flexin...) is
   automatically matched to its real "-ing" dictionary form for BOTH
   syllable counting and rhyme-finding. This is a rule, not a word list,
   so it covers that entire pattern at once instead of needing every verb
   listed by hand.
3. New: words with no phonetic data at all (not in CMUdict, not a
   g-dropped verb) now fall back to a pure spelling-based match instead
   of returning nothing -- lower-confidence than real pronunciation
   matching, but still useful for made-up words and unusual slang.
None of this touches the network -- it's all still instant, offline
lookups, same as before.

v2.4.0 change: color moved back to the margin digit -- the lyric text
itself no longer changes color. (v2.3.0 had moved coloring onto the lyric
text based on an earlier reading of "I want the text to change colors";
turns out that meant the margin number specifically.) The margin-coloring
approach failed twice before because of the tuple-vs-int bug described in
v2.3.1's entry below, not because of any real Notepad++ limitation -- with
that fixed, there was no reason not to go back to it. The 12-color
golden-angle palette from v2.3.2 carries over unchanged.

v2.3.2 fix + tweak: two things.
1. Colors are down to 12 instead of 16, and the range moved from
   indicators 8-23 to 9-20. Indicator 8 is hard-coded inside Notepad++
   itself for underlining URLs, so anything landing on it kept getting
   silently reset -- that's why 1-syllable lines showed no color at all.
   21+ overlaps Notepad++'s Mark styles, Find Mark, and Smart Highlighting.
   9-20 is a clean, unclaimed dozen. (Superseded by v2.4.0 above -- this
   was for the text-indicator approach, no longer used.)
2. The color order changed from a plain rainbow sweep to a golden-angle
   step, so NEIGHBOURING syllable counts land on close-to-opposite hues
   instead of adjacent shades (which is why 5, 6, and 7 all looked like
   "basically the same green" before) -- every count-to-count jump should
   now read as a clearly different color.

v2.3.1 fix: found the actual root cause behind every color attempt failing
(this one, and the two margin-coloring attempts before it). PythonScript's
color setters (styleSetFore/Back, indicSetFore, callTipSetFore/Back, etc.)
all want a plain (r, g, b) tuple, not a packed 0xRRGGBB/0xBBGGRR integer
the way raw Scintilla messages do. Every color call was silently failing on
the wrong type -- v2.3.0's indicator TYPE (INDIC_TEXTFORE) still applied
since that's a plain int, which is why text rendered in a flat color
(black, Scintilla's built-in default) instead of staying uncolored. Now
passing proper tuples throughout, so the actual rainbow should show.


v2.3.0 change: syllable-count coloring moved from the margin digit to the
lyric TEXT itself. Two attempts at coloring the margin number never
actually rendered -- Notepad++ appears to force every text-margin to use
its single "Line number margin" global style regardless of what per-line
style is requested. Coloring the words directly uses Scintilla's text-color
indicators instead (INDIC_TEXTFORE), the same mechanism spell-checkers use,
which is far more reliably supported. The margin still shows the plain
number, unchanged since day one -- only the coloring moved.

v2.2.1 fix: slang contractions like "bitch'll", "trick'll" were undercounted
by one syllable (the "'ll" has no vowel letter for the counter to see, but
it's still a spoken syllable when it follows a consonant). Fixed -- see
lyrictools_core.py if you're curious how.

Core features:
- Live syllable counts in a dedicated Notepad++ margin, color-coded by
  number across 12 deliberately-spread colors (neighbouring counts get
  maximally different hues, not adjacent shades) so lines with matching
  or differing syllable counts jump out at a glance. The lyric text
  itself is untouched -- only the margin digit changes color.
- Perfect rhymes generated offline from bundled CMU pronunciation data.
- Near/slant rhymes generated offline from phonetic ending similarity.
- Deep Search: a one-shot command that adds Datamuse's much larger word
  list on top of the offline engine for maximum rhyme coverage, without
  ever touching the network during normal typing/highlighting.
- Rhyme (and synonym) suggestions appear as a native dropdown -- no side
  pane, no second document, no docked or floating window of any kind.

How the rhyme dropdown works:
- Fully highlight a single word in your lyrics (double-click it, or
  drag-select exactly that word) and a dropdown pops up right there with
  suggestions.
- Pick an entry (arrow keys + Enter/Tab, or double-click it) and it replaces
  the highlighted word.
- Press Escape, click elsewhere, or just deselect and the dropdown closes on
  its own. Nothing is left behind -- no extra tab, no extra file.

Choosing which list the rhyme dropdown shows:
- LyricTools_Rhymes.py       -> dropdown shows PERFECT RHYMES
- LyricTools_NearRhymes.py   -> dropdown shows NEAR / SLANT RHYMES
- LyricTools_RhymeReport.py  -> dropdown shows PERFECT + NEAR combined

Run one of those scripts once to pick the mode; after that, every time you
fully highlight a word it shows that same category, until you run a
different one of the three scripts. All three are fully offline, so
highlighting a word never triggers a web request.

Turning on LyricToolsPP.py already arms PERFECT + NEAR (Report) by default,
so the rhyme dropdown works right away without running anything else first.

Synonyms and Deep Search work differently on purpose:
- LyricTools_Synonyms.py and LyricTools_DeepSearch.py are ONE-SHOT
  lookups, not armed live modes.
- Highlight a word (or just park the caret in one), then run one of them.
  Synonyms fetches synonyms from Datamuse once; Deep Search combines the
  full offline engine with Datamuse's perfect + near rhyme relations for
  the biggest possible pool of rhymes. Either way you get the same kind
  of dropdown for that single word.
- Neither stays "on" for future highlights. Earlier versions tried to
  auto-fire a web request every time you highlighted a new word, which
  blocked the editor and made it look like it crashed. Run the command
  again each time you want it for a different word.

Other scripts:
- LyricToolsPP.py                 Enable live syllable counts + the dropdown
- LyricTools_DeepSearch.py        One-shot maximum-coverage rhyme search
- LyricTools_CountLine.py         Show current-line syllable count
- LyricTools_ToggleSyllables.py   Toggle live syllable margin on/off
- LyricTools_Diagnostics.py       Diagnostic information

Toolbar buttons:
Adding scripts to the Notepad++ toolbar is a built-in PythonScript feature,
not something in these files. A matching set of icons is bundled in the
toolbar_icons folder right next to this one, for exactly that -- see
toolbar_icons/README.txt for how to assign them (Plugins > PythonScript >
Configuration).
