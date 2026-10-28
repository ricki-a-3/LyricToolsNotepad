# -*- coding: utf-8 -*-
from __future__ import unicode_literals
import os, sys
_SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
if _SCRIPT_DIR not in sys.path:
    sys.path.insert(0, _SCRIPT_DIR)
import lyrictools_plugin
lyrictools_plugin.enable(show_message=False)
