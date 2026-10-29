"""Editor-independent helpers for the app layer (no Qt imports).

Kept separate from ``core`` (lyric logic) and from the Qt widgets so the
selection / casing / palette rules can be unit-tested without a display.
"""
import colorsys
import re

# --- syllable palette ------------------------------------------------------

# Stepping the hue wheel by the golden angle (instead of evenly slicing it in
# count order) means NEIGHBOURING syllable counts -- the comparison that
# actually matters when eyeballing a verse -- land on close-to-opposite hues,
# so a 6 next to a 7 reads as a clearly different color, not a subtle shade.
GOLDEN_ANGLE = 0.6180339887498949
PALETTE_SIZE = 12


def build_syllable_palette(n=PALETTE_SIZE):
    """Return ``n`` (r, g, b) tuples: hue starts 0.02, S=0.85, V=0.95."""
    colors = []
    hue = 0.02
    for _ in range(n):
        r, g, b = colorsys.hsv_to_rgb(hue, 0.85, 0.95)
        colors.append((int(r * 255), int(g * 255), int(b * 255)))
        hue = (hue + GOLDEN_ANGLE) % 1.0
    return colors


SYLLABLE_COLORS = build_syllable_palette()


def color_for_count(count):
    """Palette color for a syllable count, or None for 0 (no number shown)."""
    if count <= 0:
        return None
    return SYLLABLE_COLORS[(count - 1) % PALETTE_SIZE]


# --- whole-word selection ---------------------------------------------------

# Same word shape as core._WORD_RE, plus an optional trailing apostrophe so a
# fully selected g-dropped word like "droppin'" still counts as one word.
_SELECTED_WORD_RE = re.compile(
    "[A-Za-z]+(?:['’][A-Za-z]+)*(?:-[A-Za-z]+(?:['’][A-Za-z]+)*)*['’]?"
)
_WORD_CHAR_RE = re.compile("[A-Za-z]")


def is_whole_word_selection(line_text, start, end):
    """True when ``line_text[start:end]`` is exactly one whole word.

    Not a partial word (letters continue on either side), not several words,
    not punctuation. ``start``/``end`` are offsets within the line.
    """
    if start < 0 or end > len(line_text) or end <= start:
        return False
    if not _SELECTED_WORD_RE.fullmatch(line_text[start:end]):
        return False
    before = line_text[start - 1] if start > 0 else ""
    after = line_text[end] if end < len(line_text) else ""
    return not (_WORD_CHAR_RE.match(before) or _WORD_CHAR_RE.match(after))


def word_span_at(line_text, pos):
    """(start, end) of the word touching offset ``pos`` in a line, or None."""
    for match in _SELECTED_WORD_RE.finditer(line_text):
        if match.start() <= pos <= match.end():
            return match.start(), match.end()
    return None


# --- replacement casing -------------------------------------------------------

def match_case(original, replacement):
    """Carry the original word's capitalization over to its replacement.

    ``Night`` -> ``Light``, ``NIGHT`` -> ``LIGHT``, ``night`` -> ``light``.
    Mixed or unusual casing is left to the replacement as given.
    """
    letters = [c for c in original if c.isalpha()]
    if not letters:
        return replacement
    if len(letters) > 1 and all(c.isupper() for c in letters):
        return replacement.upper()
    if letters[0].isupper() and all(c.islower() for c in letters[1:]):
        return replacement[:1].upper() + replacement[1:]
    return replacement


# --- files -------------------------------------------------------------------

def decode_text(data):
    """Decode file bytes -> (text with \\n newlines, newline style, encoding).

    Lyrics files come from all over (Notepad, phones, old Notepad++ installs),
    so accept UTF-8 with or without BOM and fall back to Windows-1252.
    """
    first = "utf-8-sig" if data.startswith(b"\xef\xbb\xbf") else "utf-8"
    for encoding in (first, "cp1252"):
        try:
            text = data.decode(encoding)
            break
        except UnicodeDecodeError:
            continue
    else:  # cp1252 has a few undefined bytes; never refuse to open a file
        encoding = "latin-1"
        text = data.decode(encoding)
    if "\r\n" in text:
        newline = "\r\n"
    elif "\r" in text:
        newline = "\r"
    else:
        newline = "\n"
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    return text, newline, encoding


def encode_text(text, newline="\r\n", encoding="utf-8"):
    # Keep a BOM only if the file had one. Legacy-encoded files are re-saved
    # as UTF-8 so nothing typed later (curly quotes, emoji) can fail to encode.
    encoding = "utf-8-sig" if encoding == "utf-8-sig" else "utf-8"
    return text.replace("\n", newline).encode(encoding)
