# LyricTools++

A free, open-source Windows app for rap and lyric writing: live per-line
syllable counts and highlight-to-rhyme suggestions.

> **Status:** the standalone app (v3) is in development. The original
> Notepad++ plugin (v2.6.0) still works and lives in
> [`legacy/notepadpp-plugin`](legacy/notepadpp-plugin) with its own
> installer and README.

## Development

```
pip install -e ".[dev]"
python -m pytest -q
```

## License

MIT — see [`LICENSE`](LICENSE). The bundled CMU Pronouncing Dictionary is
separately licensed — see `src/lyrictools/data/CMUDICT_LICENSE.txt`.
