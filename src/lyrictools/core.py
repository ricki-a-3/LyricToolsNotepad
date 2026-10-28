# -*- coding: utf-8 -*-
"""Core logic for LyricTools++ (Python 2.7 / PythonScript compatible).

Word coverage, in priority order:
1. LYRIC_EXCEPTIONS -- a hand-picked list of common rap/informal
   vocabulary (contractions, ad-libs, dropped consonants) that either
   isn't in CMUdict at all or that CMUdict would get wrong for how it's
   actually spoken in lyrics.
2. CMUdict -- the bundled offline pronunciation dictionary, ~134,000
   words, used for both syllable counts and real phonetic rhyme matching.
3. G-dropping normalization -- an open-class rule (not a word list) that
   maps informal "-in'"/"-in" verb spellings back to their "-ing"
   dictionary form (droppin' -> dropping, spittin -> spitting, ...) so
   syllable counts AND rhymes work for any verb in this extremely common
   rap-lyric pattern, not just ones explicitly listed somewhere.
4. A vowel-counting heuristic for syllables, and a spelling-suffix
   fallback for rhymes, when nothing above finds a match -- lower
   confidence than real phonetic data, but still useful for made-up
   words, unusual slang, and proper nouns instead of returning nothing.

Online word suggestions (Datamuse) are used only for synonyms, and only
when explicitly requested -- rhymes stay fully offline so they're always
instant and never depend on a network connection.
"""
from __future__ import unicode_literals

import io
import json
import os
import re
from difflib import SequenceMatcher

try:
    # Python 2.7 (Notepad++ PythonScript 2.x)
    from urllib import urlencode
    from urllib2 import Request, urlopen
except ImportError:
    # Also keep the core testable under modern Python.
    from urllib.parse import urlencode
    from urllib.request import Request, urlopen

try:
    text_type = unicode
except NameError:
    text_type = str

_WORD_RE = re.compile(u"[A-Za-z]+(?:['\u2019][A-Za-z]+)*(?:-[A-Za-z]+(?:['\u2019][A-Za-z]+)*)*")
_VOWEL_GROUP_RE = re.compile(u"[aeiouy]+")
_VARIANT_RE = re.compile(u"\\(\\d+\\)$")
_CMU_COUNTS = None
_CMU_PRONS = None
_PERFECT_RHYME_INDEX = None
_NEAR_LAST_INDEX = None
_NEAR_VOWEL_INDEX = None
_SPELLING_SUFFIX_INDEX = None
_API_CACHE = {}
_LOCAL_RHYME_CACHE = {}

LYRIC_EXCEPTIONS = {
    # Interjections / ad-libs
    u"yeah": 1, u"yea": 1, u"nah": 1, u"uh": 1, u"uhh": 1,
    u"oh": 1, u"ooh": 1, u"ayy": 1, u"ay": 1, u"ayo": 2, u"yo": 1,
    u"bro": 1, u"bruh": 1, u"fam": 1, u"mane": 1, u"skrrt": 1,
    u"woo": 1, u"yah": 1, u"nuh": 1,

    # "to/have/of" reductions -- closed class, worth listing outright
    # rather than trying to rule-match (unlike the open-class "-in'" verb
    # pattern below, which covers unlimited words on its own).
    u"gon": 1, u"gonna": 2, u"wanna": 2, u"gotta": 2, u"kinda": 2,
    u"sorta": 2, u"lemme": 2, u"gimme": 2, u"tryna": 2, u"finna": 2,
    u"imma": 2, u"boutta": 2, u"outta": 2, u"lotta": 2, u"hafta": 2,
    u"needa": 2, u"useta": 2, u"oughta": 2,
    u"shoulda": 2, u"coulda": 2, u"woulda": 2, u"musta": 2, u"mighta": 2,

    # Dropped/relaxed consonants
    u"wit": 1, u"dat": 1, u"dis": 1, u"dem": 1, u"dey": 1, u"deez": 1,
    u"doe": 1, u"nuff": 1, u"prolly": 2, u"lil": 1, u"ol": 1,

    # Common contractions, with AND without the apostrophe -- lyrics are
    # rarely typed with consistent punctuation, and CMUdict generally only
    # has the properly-punctuated form.
    u"ain't": 1, u"aint": 1, u"cause": 1, u"cuz": 1, u"coz": 1,
    u"bout": 1, u"em": 1, u"yall": 1, u"y'all": 1, u"im": 1, u"i'm": 1,
    u"dont": 1, u"don't": 1, u"wont": 1, u"won't": 1, u"cant": 1,
    u"can't": 1, u"wasnt": 2, u"wasn't": 2, u"isnt": 2, u"isn't": 2,
    u"arent": 1, u"aren't": 1, u"werent": 1, u"weren't": 1,
    u"hasnt": 2, u"hasn't": 2, u"hadnt": 2, u"hadn't": 2,
    u"doesnt": 2, u"doesn't": 2, u"didnt": 2, u"didn't": 2,
    u"couldnt": 2, u"couldn't": 2, u"wouldnt": 2, u"wouldn't": 2,
    u"shouldnt": 2, u"shouldn't": 2, u"til": 1, u"'til": 1,
}

_LETTER_SYLLABLES = {
    u"A": 1, u"B": 1, u"C": 1, u"D": 1, u"E": 1, u"F": 1, u"G": 1,
    u"H": 1, u"I": 1, u"J": 1, u"K": 1, u"L": 1, u"M": 1, u"N": 1,
    u"O": 1, u"P": 1, u"Q": 1, u"R": 1, u"S": 1, u"T": 1, u"U": 1,
    u"V": 1, u"W": 3, u"X": 1, u"Y": 1, u"Z": 1,
}


def _to_unicode(value):
    if value is None:
        return u""
    if isinstance(value, text_type):
        return value
    try:
        return value.decode("utf-8", "replace")
    except AttributeError:
        return text_type(value)


def _default_dict_path():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "data", "cmudict.dict")


def load_cmudict(path=None):
    """Load a word -> syllable-count mapping from the bundled CMUdict."""
    global _CMU_COUNTS
    if _CMU_COUNTS is not None:
        return _CMU_COUNTS

    path = path or _default_dict_path()
    counts = {}
    try:
        with io.open(path, "r", encoding="utf-8", errors="replace") as handle:
            for raw in handle:
                raw = raw.strip()
                if not raw or raw.startswith(u";;;"):
                    continue
                parts = raw.split()
                if len(parts) < 2:
                    continue
                word = _VARIANT_RE.sub(u"", parts[0].lower())
                if word in counts:
                    continue
                syllables = sum(1 for phone in parts[1:] if phone and phone[-1].isdigit())
                if syllables:
                    counts[word] = syllables
    except (IOError, OSError):
        counts = {}

    _CMU_COUNTS = counts
    return _CMU_COUNTS


def load_cmu_pronunciations(path=None):
    """Load all CMU pronunciations for offline rhyme lookup."""
    global _CMU_PRONS
    if _CMU_PRONS is not None:
        return _CMU_PRONS

    path = path or _default_dict_path()
    prons = {}
    try:
        with io.open(path, "r", encoding="utf-8", errors="replace") as handle:
            for raw in handle:
                raw = raw.strip()
                if not raw or raw.startswith(u";;;"):
                    continue
                parts = raw.split()
                if len(parts) < 2:
                    continue
                word = _VARIANT_RE.sub(u"", parts[0].lower())
                # Avoid punctuation-only oddities but keep normal apostrophes.
                if not re.search(u"[a-z]", word):
                    continue
                phones = tuple(parts[1:])
                bucket = prons.setdefault(word, [])
                if phones not in bucket:
                    bucket.append(phones)
    except (IOError, OSError):
        prons = {}

    _CMU_PRONS = prons
    return _CMU_PRONS


def _phone_base(phone):
    return re.sub(u"[0-9]", u"", _to_unicode(phone))


def _rhyme_tail(phones):
    """Return the final stressed-vowel rhyme portion, without stress digits."""
    if not phones:
        return tuple()
    start = None
    # Prefer the last primary/secondary stressed vowel.
    for i in range(len(phones) - 1, -1, -1):
        ph = _to_unicode(phones[i])
        if ph and ph[-1:] in (u"1", u"2"):
            start = i
            break
    # Fall back to the last vowel of any stress level.
    if start is None:
        for i in range(len(phones) - 1, -1, -1):
            ph = _to_unicode(phones[i])
            if ph and ph[-1:].isdigit():
                start = i
                break
    if start is None:
        start = max(0, len(phones) - 2)
    return tuple(_phone_base(ph) for ph in phones[start:])


def _build_rhyme_indexes():
    global _PERFECT_RHYME_INDEX, _NEAR_LAST_INDEX, _NEAR_VOWEL_INDEX
    if _PERFECT_RHYME_INDEX is not None:
        return
    perfect = {}
    by_last = {}
    by_vowel = {}
    for word, variants in load_cmu_pronunciations().items():
        for phones in variants:
            tail = _rhyme_tail(phones)
            if not tail:
                continue
            perfect.setdefault(tail, set()).add(word)
            by_last.setdefault(tail[-1], set()).add(word)
            by_vowel.setdefault(tail[0], set()).add(word)
    _PERFECT_RHYME_INDEX = perfect
    _NEAR_LAST_INDEX = by_last
    _NEAR_VOWEL_INDEX = by_vowel


def _build_spelling_index():
    """Reverse index of dictionary words by their last 2-4 letters, used
    only as a last resort when a word has no phonetic data at all (not in
    CMUdict, and not a recognizable g-dropped verb form). Spelling isn't
    pronunciation, so these are lower-confidence than real phonetic
    matches -- but for made-up words, unusual slang, or proper nouns,
    "probably close" beats returning nothing."""
    global _SPELLING_SUFFIX_INDEX
    if _SPELLING_SUFFIX_INDEX is not None:
        return
    index = {}
    for word in load_cmudict():
        clean = re.sub(u"[^a-z]", u"", word)
        if len(clean) < 2:
            continue
        for n in (4, 3, 2):
            if len(clean) >= n:
                index.setdefault((n, clean[-n:]), set()).add(word)
    _SPELLING_SUFFIX_INDEX = index


def _orthographic_rhymes(anchor, max_results):
    clean = re.sub(u"[^a-z]", u"", anchor)
    if len(clean) < 2:
        return []
    _build_spelling_index()
    anchor_syl = syllables_for_word(anchor)
    words = set()
    for n in (4, 3, 2):
        if len(clean) >= n:
            words = _SPELLING_SUFFIX_INDEX.get((n, clean[-n:]), set())
            if words:
                break
    out = []
    for word in words:
        if word == clean or word == anchor:
            continue
        out.append({
            "word": word,
            "syllables": syllables_for_word(word),
            # Deliberately low relative to the 100s-1000s phonetic scores
            # elsewhere in this module -- these are spelling guesses, not
            # verified pronunciation matches.
            "score": 1,
        })
    out.sort(key=lambda item: (abs(item["syllables"] - anchor_syl), item["word"]))
    return out[:max_results]


def _local_rhymes(anchor, near=False, max_results=80):
    """Offline perfect/slant rhymes from bundled CMU phonemes."""
    anchor = _to_unicode(anchor).lower().replace(u"\u2019", u"'").strip(u"'\".,!?;:()[]{}")
    cache_key = (anchor, bool(near), int(max_results))
    if cache_key in _LOCAL_RHYME_CACHE:
        return [dict(item) for item in _LOCAL_RHYME_CACHE[cache_key]]

    pron_map = load_cmu_pronunciations()
    variants = pron_map.get(anchor)
    lookup_word = anchor  # the dictionary word whose phonemes anchor the search

    if not variants:
        # Try "droppin'" -> "dropping" etc. before giving up on phonetics
        # entirely -- the normalized form is a real dictionary word whose
        # pronunciation is a legitimate stand-in for the slang spelling.
        gdropped = _gdrop_normalize(anchor)
        if gdropped:
            gvariants = pron_map.get(gdropped)
            if gvariants:
                variants = gvariants
                lookup_word = gdropped

    if not variants:
        # No phonetic data available at all -- fall back to a pure
        # spelling match rather than returning nothing.
        result = _orthographic_rhymes(anchor, max_results)
        _LOCAL_RHYME_CACHE[cache_key] = [dict(item) for item in result]
        return result

    _build_rhyme_indexes()

    anchor_syl = syllables_for_word(anchor)
    anchor_tails = [_rhyme_tail(p) for p in variants if _rhyme_tail(p)]
    scores = {}

    if not near:
        for tail in anchor_tails:
            for word in _PERFECT_RHYME_INDEX.get(tail, ()):
                if word != lookup_word and word != anchor:
                    scores[word] = max(scores.get(word, 0), 1000)
    else:
        candidates = set()
        for tail in anchor_tails:
            candidates.update(_NEAR_LAST_INDEX.get(tail[-1], ()))
            candidates.update(_NEAR_VOWEL_INDEX.get(tail[0], ()))

        for word in candidates:
            if word == lookup_word or word == anchor:
                continue
            best = 0.0
            for atail in anchor_tails:
                for phones in pron_map.get(word, ()):
                    ctail = _rhyme_tail(phones)
                    if not ctail or ctail == atail:
                        continue  # Keep the near list focused on slant rhymes.
                    ratio = SequenceMatcher(None, atail, ctail).ratio()
                    score = ratio
                    if atail[-1] == ctail[-1]:
                        score += 0.18
                    if atail[0] == ctail[0]:
                        score += 0.14
                    if len(atail) >= 2 and len(ctail) >= 2 and atail[-2:] == ctail[-2:]:
                        score += 0.10
                    syl_diff = abs(syllables_for_word(word) - anchor_syl)
                    score -= min(0.18, syl_diff * 0.04)
                    if score > best:
                        best = score
            if best >= 0.60:
                scores[word] = int(best * 1000)

    out = []
    for word, score in scores.items():
        out.append({
            "word": word,
            "syllables": syllables_for_word(word),
            "score": int(score),
        })
    out.sort(key=lambda item: (-item["score"], abs(item["syllables"] - anchor_syl), item["word"]))
    result = out[:max_results]
    _LOCAL_RHYME_CACHE[cache_key] = [dict(item) for item in result]
    return result


def _heuristic_syllables(word):
    original = _to_unicode(word)
    word = original.lower().replace(u"\u2019", u"'")
    word = re.sub(u"[^a-z']", u"", word)
    word = word.strip(u"'")
    if not word:
        return 0

    if word in LYRIC_EXCEPTIONS:
        return LYRIC_EXCEPTIONS[word]

    if word.endswith(u"'s") and len(word) > 2:
        base = word[:-2]
        base_count = syllables_for_word(base)
        return base_count + (1 if re.search(u"(?:s|z|sh|ch|x|j|ge)$", base) else 0)

    letters_only = re.sub(u"[^A-Za-z]", u"", original)
    if letters_only.isupper() and 2 <= len(letters_only) <= 6:
        return sum(_LETTER_SYLLABLES.get(ch, 1) for ch in letters_only)

    count = len(_VOWEL_GROUP_RE.findall(word))

    # Contraction endings like "'ll" have no vowel letter for the scan above
    # to catch, but "bitch'll", "trick'll", "prison'll" etc. still add a
    # spoken syllable (a syllabic /l/) when the base word ends in a
    # consonant -- you can't glide straight from a consonant into "ll"
    # without a vowel sound. Words ending in a vowel don't need this: "he'll"
    # / "we'll" / "I'll" stay one syllable because the glide absorbs it.
    if word.endswith(u"'ll") and len(word) > 3:
        before_apostrophe = word[:-3]
        if before_apostrophe and before_apostrophe[-1] not in u"aeiouy":
            count += 1

    if len(word) > 3 and word.endswith(u"e") and not re.search(u"[^aeiou]le$", word):
        count -= 1

    if len(word) > 4 and word.endswith(u"ed") and not re.search(u"(?:ted|ded)$", word):
        before = word[:-2]
        if _VOWEL_GROUP_RE.findall(before):
            count -= 1

    for pattern in (u"ia", u"io", u"eo", u"ua"):
        if re.search(pattern, word):
            count += 1

    return max(1, count)


def _gdrop_normalize(word):
    """Map a dropped-g slang spelling back to its dictionary form.

    Covers the single most common rap/informal pattern -- any verb ending
    in "-in" or "-in'" instead of "-ing" (droppin', spittin, flexin,
    trippin...). This is only ever tried as a fallback AFTER a direct
    dictionary lookup has already failed, so real short words that happen
    to end in "in" (win, pin, sin...) are unaffected -- they succeed on
    the direct lookup and never reach this function. Returns None when the
    word doesn't look like a candidate for this pattern.
    """
    w = word[:-1] if word.endswith(u"'") else word
    if len(w) > 3 and w.endswith(u"in") and not w.endswith(u"ing"):
        return w + u"g"
    return None


def syllables_for_word(word):
    if not word:
        return 0

    normalized = _to_unicode(word).lower().replace(u"\u2019", u"'").strip(u"'\".,!?;:()[]{}")
    if not normalized:
        return 0

    if normalized in LYRIC_EXCEPTIONS:
        return LYRIC_EXCEPTIONS[normalized]

    counts = load_cmudict()
    if normalized in counts:
        return counts[normalized]

    if u"-" in normalized:
        parts = [p for p in normalized.split(u"-") if p]
        if parts:
            return sum(syllables_for_word(p) for p in parts)

    stripped = re.sub(u"[^a-z']", u"", normalized)
    if stripped in counts:
        return counts[stripped]

    gdropped = _gdrop_normalize(stripped)
    if gdropped and gdropped in counts:
        return counts[gdropped]

    return _heuristic_syllables(word)


def tokenize_line(text):
    return _WORD_RE.findall(_to_unicode(text))


def is_section_header(text):
    stripped = _to_unicode(text).strip()
    return bool(
        re.match(u"^\\[[^\\]]+\\]$", stripped) or
        re.match(u"^(verse|chorus|hook|bridge|intro|outro|pre[- ]?chorus)\\b.*:$", stripped, re.I)
    )


def syllables_for_line(text):
    text = _to_unicode(text)
    if not text or is_section_header(text):
        return 0
    return sum(syllables_for_word(token) for token in tokenize_line(text))


def _fetch_datamuse(params, timeout=8):
    key = tuple(sorted((text_type(k), text_type(v)) for k, v in params.items()))
    if key in _API_CACHE:
        return _API_CACHE[key]

    encoded = {}
    for key_name, value in params.items():
        if isinstance(value, text_type):
            encoded[key_name] = value.encode("utf-8")
        else:
            encoded[key_name] = value

    url = "https://api.datamuse.com/words?" + urlencode(encoded)
    request = Request(url, headers={"User-Agent": "LyricToolsPP/3 (+https://www.datamuse.com/api/)"})

    # Python 3.12+ verifies TLS against the OS certificate store; no
    # unverified-context retry (that was a workaround for PythonScript 2.7).
    with urlopen(request, timeout=timeout) as response:
        payload = response.read()

    if not isinstance(payload, text_type):
        payload = payload.decode("utf-8", "replace")
    data = json.loads(payload)
    if not isinstance(data, list):
        data = []
    _API_CACHE[key] = data
    return data


def _normalize_results(data, anchor, max_results):
    anchor_l = _to_unicode(anchor).lower()
    anchor_syl = syllables_for_word(anchor)
    out = []
    seen = set()
    for row in data:
        word = _to_unicode(row.get("word", u"")).strip()
        if not word or word.lower() == anchor_l or word.lower() in seen:
            continue
        seen.add(word.lower())
        syllables = row.get("numSyllables")
        if not isinstance(syllables, int):
            syllables = syllables_for_line(word)
        try:
            score = int(row.get("score", 0) or 0)
        except (TypeError, ValueError):
            score = 0
        out.append({"word": word, "syllables": syllables, "score": score})

    out.sort(key=lambda item: (abs(item["syllables"] - anchor_syl), -item["score"], item["word"].lower()))
    return out[:max_results]


def find_words_local(anchor, mode="rhyme", max_results=80):
    """Fast, network-free lookup used by the live rhyme follower."""
    anchor = _to_unicode(anchor).strip()
    if not anchor:
        return []
    if mode == "rhyme":
        return _local_rhymes(anchor, near=False, max_results=max_results)
    if mode == "near":
        return _local_rhymes(anchor, near=True, max_results=max_results)
    raise ValueError("Local lookup only supports rhyme/near modes: %s" % mode)


def find_words(anchor, mode="rhyme", max_results=80, timeout=8):
    anchor = _to_unicode(anchor).strip()
    if not anchor:
        return []

    # Rhyming is fully offline so it remains fast/reliable and does not depend
    # on Datamuse's changing relation-code set.
    if mode == "rhyme":
        return _local_rhymes(anchor, near=False, max_results=max_results)
    if mode == "near":
        local = _local_rhymes(anchor, near=True, max_results=max_results)
        if local:
            return local
        # For slang/OOV words not present in CMUdict, use Datamuse's currently
        # documented sounds-like constraint as a fallback.
        data = _fetch_datamuse({"sl": anchor, "max": max(100, min(250, max_results * 2)), "md": "s"}, timeout=timeout)
        return _normalize_results(data, anchor, max_results)
    if mode == "synonym":
        data = _fetch_datamuse({"rel_syn": anchor, "max": max(100, min(250, max_results * 2)), "md": "s"}, timeout=timeout)
        return _normalize_results(data, anchor, max_results)
    raise ValueError("Unknown mode: %s" % mode)


def find_words_deep(anchor, max_results=200, timeout=6):
    """Maximum-coverage rhyme lookup: the full offline engine (CMUdict +
    slang list + g-dropping + spelling fallback) merged with Datamuse's
    dedicated perfect-rhyme (rel_rhy) and near-rhyme (rel_nry) relations,
    which draw from a much larger word list than CMUdict alone.

    This is deliberately never called from the passive highlight-trigger
    (see rhymes()/near_rhymes()/rhyme_report() in lyrictools_plugin.py) --
    it's for the explicit, one-shot "Deep Search" command only, same as
    synonyms(). A network hiccup here costs one deliberate lookup, never
    the live typing/highlighting flow.

    Ranking uses explicit source tiers rather than comparing raw scores
    directly: offline perfect/near use one scoring formula, Datamuse
    returns its own unrelated scale, and near-rhyme scoring can go above
    the perfect-rhyme baseline once its similarity bonuses stack up. None
    of those numbers mean the same thing, so blending them naively let
    inflated near-rhyme scores outrank genuine perfect rhymes. Tiers keep
    offline-verified perfect rhymes first, then offline near, then
    Datamuse's own perfect/near, each ordered by its own score internally.
    """
    anchor = _to_unicode(anchor).strip()
    if not anchor:
        return []
    anchor_l = anchor.lower()
    anchor_syl = syllables_for_word(anchor)

    combined = {}  # word_lower -> (tier, score, display_dict)

    def _consider(word, tier, score, syllables):
        key = _to_unicode(word).strip().lower()
        if not key or key == anchor_l:
            return
        existing = combined.get(key)
        if existing is not None and existing[0] <= tier:
            return  # already have this word from an equal-or-better tier
        combined[key] = (tier, score, {
            "word": _to_unicode(word).strip(),
            "syllables": syllables,
            "score": score,
        })

    for item in _local_rhymes(anchor, near=False, max_results=max_results):
        _consider(item["word"], 0, item["score"], item["syllables"])
    for item in _local_rhymes(anchor, near=True, max_results=max_results):
        _consider(item["word"], 1, item["score"], item["syllables"])

    for tier, params in ((2, {"rel_rhy": anchor, "max": 250}), (3, {"rel_nry": anchor, "max": 250})):
        try:
            rows = _fetch_datamuse(params, timeout=timeout)
        except Exception:
            # One relation failing (timeout, no connection, API hiccup)
            # shouldn't lose the other one or the offline results already
            # in hand.
            continue
        for row in rows:
            word = _to_unicode(row.get("word", u"")).strip()
            if not word:
                continue
            try:
                score = int(row.get("score", 0) or 0)
            except (TypeError, ValueError):
                score = 0
            _consider(word, tier, score, syllables_for_line(word))

    entries = list(combined.values())  # each is (tier, score, display_dict)
    entries.sort(key=lambda e: (e[0], -e[1], abs(e[2]["syllables"] - anchor_syl), e[2]["word"]))
    return [entry[2] for entry in entries[:max_results]]


def grouped_text(results, heading="Suggestions"):
    if not results:
        return _to_unicode(heading) + u"\n(no results)"
    groups = {}
    for item in results:
        groups.setdefault(item["syllables"], []).append(item["word"])
    lines = [_to_unicode(heading)]
    for syllables in sorted(groups):
        label = u"syllable" if syllables == 1 else u"syllables"
        lines.append(u"\n%d %s:" % (syllables, label))
        lines.append(u"  " + u", ".join(groups[syllables]))
    return u"\n".join(lines)
