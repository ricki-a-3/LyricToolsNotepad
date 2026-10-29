"""Tests for the app-layer helpers (no Qt needed)."""
import colorsys

import pytest

from lyrictools import textutil


def test_palette_matches_plugin_formula():
    assert len(textutil.SYLLABLE_COLORS) == 12
    r, g, b = colorsys.hsv_to_rgb(0.02, 0.85, 0.95)
    assert textutil.SYLLABLE_COLORS[0] == (int(r * 255), int(g * 255), int(b * 255))
    hue = (0.02 + textutil.GOLDEN_ANGLE) % 1.0
    r, g, b = colorsys.hsv_to_rgb(hue, 0.85, 0.95)
    assert textutil.SYLLABLE_COLORS[1] == (int(r * 255), int(g * 255), int(b * 255))


def test_color_for_count_wraps_every_12():
    assert textutil.color_for_count(0) is None
    assert textutil.color_for_count(1) == textutil.SYLLABLE_COLORS[0]
    assert textutil.color_for_count(13) == textutil.SYLLABLE_COLORS[0]
    assert textutil.color_for_count(12) == textutil.SYLLABLE_COLORS[11]


@pytest.mark.parametrize("line,start,end,expected", [
    ("I been droppin' bars", 7, 15, True),    # with trailing apostrophe
    ("I been droppin' bars", 7, 14, True),    # Qt double-click drops the '
    ("I been droppin' bars", 7, 12, False),   # partial word
    ("I been droppin' bars", 2, 14, False),   # two words
    ("night, light", 0, 5, True),
    ("night, light", 0, 6, False),            # includes the comma
    ("it's bitch'll", 5, 13, True),
    ("x", 0, 0, False),
])
def test_whole_word_selection(line, start, end, expected):
    assert textutil.is_whole_word_selection(line, start, end) is expected


def test_word_span_at():
    assert textutil.word_span_at("hey there", 5) == (4, 9)
    assert textutil.word_span_at("hey there", 3) == (0, 3)  # caret just after a word
    assert textutil.word_span_at("   ", 1) is None


@pytest.mark.parametrize("original,choice,expected", [
    ("night", "light", "light"),
    ("Night", "light", "Light"),
    ("NIGHT", "light", "LIGHT"),
    ("I", "eye", "Eye"),
    ("night", "Paris", "Paris"),
])
def test_match_case(original, choice, expected):
    assert textutil.match_case(original, choice) == expected


def test_decode_round_trip_preserves_crlf_and_bom():
    raw = "﻿[Verse]\r\nline one\r\n".encode("utf-8")
    text, newline, encoding = textutil.decode_text(raw)
    assert text == "[Verse]\nline one\n"
    assert (newline, encoding) == ("\r\n", "utf-8-sig")
    assert textutil.encode_text(text, newline, encoding) == raw


def test_decode_falls_back_to_cp1252():
    text, _, encoding = textutil.decode_text("caf\xe9 \x93quote\x94".encode("latin-1"))
    assert text == "café “quote”"
    assert encoding == "cp1252"
    assert textutil.encode_text(text, "\n", encoding) == text.encode("utf-8")


def test_no_bom_is_added_to_plain_utf8():
    text, newline, encoding = textutil.decode_text(b"line\n")
    assert encoding == "utf-8"
    assert textutil.encode_text(text, newline, encoding) == b"line\n"
