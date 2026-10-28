# Copy the LyricTool folder directly into your PythonScript scripts folder
# (next to Samples), then put this file in your PythonScript startup
# location only after manual testing succeeds. Note: Install_LyricToolsPP.bat
# sets up autostart for you automatically using a relative-path version of
# this same idea (_lyrictools_autostart_snippet.py) -- this file is only
# needed if you're doing everything by hand.
import os, sys
SCRIPT_DIR = r"C:\Program Files\Notepad++\plugins\PythonScript\scripts\LyricTool"
if SCRIPT_DIR not in sys.path:
    sys.path.insert(0, SCRIPT_DIR)
import lyrictools_plugin
lyrictools_plugin.enable(show_message=False)
