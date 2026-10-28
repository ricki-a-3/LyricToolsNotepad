
# --- LyricTools++ autostart (added by installer) ---
try:
    import os, sys
    _ltpp_dir = os.path.join(os.path.dirname(os.path.abspath(__file__)), "LyricTool")
    if _ltpp_dir not in sys.path:
        sys.path.insert(0, _ltpp_dir)
    import lyrictools_plugin as _ltpp
    _ltpp.enable(show_message=False)
except Exception as _ltpp_exc:
    try:
        from Npp import console as _ltpp_console
        _ltpp_console.write("LyricTools++ autostart failed: %s\n" % _ltpp_exc)
    except Exception:
        pass
# --- end LyricTools++ autostart ---
