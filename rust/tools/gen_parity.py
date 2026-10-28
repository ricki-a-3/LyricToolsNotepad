"""Dump the Python reference core's output as a fixture for tests/parity.rs.

Dev-time only (the app itself is 100% Rust). Run from the repo root:
    python rust/tools/gen_parity.py
"""
import json
import os
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
sys.path.insert(0, os.path.join(ROOT, "src"))
from lyrictools import core  # noqa: E402

FAKE = {
    "rel_rhy": [{"word": "night", "score": 900}, {"word": "sleight", "score": 700},
                {"word": "yeezynight", "score": 50}, {"word": "Bright Light", "score": 40}],
    "rel_nry": [{"word": "time", "score": 600}, {"word": "wine", "score": 500},
                {"word": "night", "score": 300}, {"word": "", "score": 1}],
    "rel_syn": [{"word": "evening", "score": 3000, "numSyllables": 2},
                {"word": "dark", "score": 2000, "numSyllables": 1},
                {"word": "Night", "score": 1500, "numSyllables": 1},
                {"word": "nighttime", "score": 1400},
                {"word": "dark", "score": 100, "numSyllables": 1},
                {"word": "after hours", "score": 900}],
}


def fake_fetch(params, timeout=8):
    for key, rows in FAKE.items():
        if key in params:
            return rows
    return []


core._fetch_datamuse = fake_fetch

words = sorted(core.LYRIC_EXCEPTIONS)
words += sorted(core.load_cmudict())[::37]
words += """droppin' droppin spittin flexin trippin win pin sin bitch'll trick'll it'll he'll we'll
I'll you'll NYC ASAP DJ WWE MCs yeezy skrrrt blorptastic Kanye's boss's judge's rich's
dog's cat's night-time ice-cold rock-n-roll don’t y’all Night NIGHT "quoted" (paren) ...
hated faded jumped rolled guitar radio video idea quiet stuff'll cookie'd biebs
zxcvb aeiou y rhythm crypt fly'n '' ' - -- -x- a-""".split()

lines = [
    "My bitch'll pop a cap", "[Verse 1]", "Chorus:", "Pre-Chorus:", "pre chorus 2:",
    "Hook (x2):", "  [Bridge]  ", "I been droppin' bars since the late night",
    "Y'all be flexin' on the gram but it never gon' last", "", "   ", "NYC to LA, ASAP",
    "Shoulda, coulda, woulda -- I ain't trippin'", "rock-n-roll all night", "don’t stop",
    "verse one is not a header", "[not] a header",
]

anchors = """night bite light droppin' droppin spittin yeezy orange purple money flow
cash fire love time rhyme heart street dream gold king soul blue fly trippin
blorptastic skrrrt biebs Kanye NIGHT Night stuff rhythm month silver hated
radio crypt glitch aalto torque cookie""".split()

fixture = {
    "syllables": {w: core.syllables_for_word(w) for w in words},
    "headers": {l: core.is_section_header(l) for l in lines},
    "lines": {l: core.syllables_for_line(l) for l in lines},
    "tokens": {l: core.tokenize_line(l) for l in lines},
    "perfect": {a: core.find_words_local(a, "rhyme", 80) for a in anchors},
    "near": {a: core.find_words_local(a, "near", 80) for a in anchors},
    "deep": {a: core.find_words_deep(a, 150) for a in ["yeezy", "bite", "night", "orange"]},
    "synonyms": {a: core.find_words(a, "synonym", 60) for a in ["night", "Night", "money"]},
}
out = os.path.join(ROOT, "rust", "tests", "fixtures", "parity.json")
with open(out, "w", encoding="utf-8") as fh:
    json.dump({"fake_datamuse": FAKE, **fixture}, fh, ensure_ascii=False, indent=0, sort_keys=True)
print(out, sum(len(v) for v in fixture.values()), "cases")
