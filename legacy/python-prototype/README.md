# Python prototype (reference only)

The PySide6 version of LyricTools++ built before the Rust rewrite. It's kept
because `tools/gen_parity.py` uses its `core.py` to generate
`tests/fixtures/parity.json`, which the Rust core is tested against.

```
pip install -e ".[dev]"
python -m pytest -q
```
