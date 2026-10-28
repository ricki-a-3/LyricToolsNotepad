"""Regression tests for lyrictools.core.

Each case here corresponds to a bug or behavior settled during the
Notepad++ plugin's history -- see CLAUDE.md.
"""
import pytest

from lyrictools import core


@pytest.fixture
def no_network(monkeypatch):
    """Fail loudly if anything tries to reach Datamuse."""
    def _boom(*args, **kwargs):
        raise AssertionError("unexpected network call")
    monkeypatch.setattr(core, "_fetch_datamuse", _boom)


@pytest.fixture
def fake_datamuse(monkeypatch):
    def _fake(params, timeout=8):
        if "rel_rhy" in params:
            return [{"word": "night", "score": 900}, {"word": "sleight", "score": 700},
                    {"word": "yeezynight", "score": 50}]
        if "rel_nry" in params:
            return [{"word": "time", "score": 600}, {"word": "wine", "score": 500},
                    {"word": "night", "score": 300}]
        return []
    monkeypatch.setattr(core, "_fetch_datamuse", _fake)


# --- syllables ------------------------------------------------------------

@pytest.mark.parametrize("word,expected", [
    # 'll after a consonant is its own syllable; after a vowel it glides
    ("bitch'll", 2), ("trick'll", 2), ("it'll", 2),
    ("he'll", 1), ("we'll", 1), ("I'll", 1), ("you'll", 1),
    # g-dropping rule
    ("droppin'", 2), ("droppin", 2), ("spittin", 2), ("flexin", 2),
    # slang exceptions list
    ("shoulda", 2), ("outta", 2), ("didnt", 2), ("dont", 1), ("wit", 1),
    # real words ending in -in must not be touched by g-dropping
    ("win", 1), ("pin", 1), ("sin", 1),
])
def test_syllables_for_word(word, expected):
    assert core.syllables_for_word(word) == expected


def test_syllables_for_line():
    assert core.syllables_for_line("My bitch'll pop a cap") == 6


@pytest.mark.parametrize("header", ["[Verse 1]", "[Chorus]", "Chorus:", "Pre-Chorus:"])
def test_section_headers_have_no_count(header):
    assert core.is_section_header(header)
    assert core.syllables_for_line(header) == 0


# --- offline rhymes -------------------------------------------------------

def test_perfect_rhymes_for_gdropped_word(no_network):
    words = {r["word"] for r in core.find_words_local("droppin'", mode="rhyme", max_results=80)}
    assert {"stopping", "popping"} <= words


def test_spelling_fallback_for_unknown_word(no_network):
    words = {r["word"] for r in core.find_words_local("yeezy", mode="rhyme", max_results=20)}
    assert "breezy" in words


def test_offline_rhymes_never_include_anchor(no_network):
    for mode in ("rhyme", "near"):
        words = {r["word"].lower() for r in core.find_words_local("night", mode=mode)}
        assert "night" not in words


# --- deep search ----------------------------------------------------------

def test_deep_search_excludes_anchor_and_dedups(fake_datamuse):
    words = [r["word"].lower() for r in core.find_words_deep("yeezy", max_results=150)]
    assert "yeezy" not in words
    assert len(words) == len(set(words))
    assert words.count("night") == 1


def test_deep_search_includes_online_only_words(fake_datamuse):
    words = [r["word"].lower() for r in core.find_words_deep("yeezy", max_results=150)]
    assert "yeezynight" in words


def test_deep_search_ranks_offline_perfect_first(fake_datamuse):
    results = [r["word"].lower() for r in core.find_words_deep("bite", max_results=150)]
    perfect = {r["word"].lower() for r in core.find_words_local("bite", mode="rhyme", max_results=150)}
    head = results[:len(perfect)]
    assert all(w in perfect for w in head), "a lower tier outranked an offline perfect rhyme"


def test_deep_search_survives_network_failure(monkeypatch):
    def _fail(*a, **k):
        raise OSError("no connection")
    monkeypatch.setattr(core, "_fetch_datamuse", _fail)
    assert core.find_words_deep("night", max_results=20), "offline results lost on network error"
