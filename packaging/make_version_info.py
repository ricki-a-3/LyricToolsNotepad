"""Write packaging/version_info.txt (Windows exe metadata) from __version__."""
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "src"))
from lyrictools import __version__  # noqa: E402

nums = [int(n) for n in re.findall(r"\d+", __version__)[:4]]
nums += [0] * (4 - len(nums))
tup = tuple(nums)

TEMPLATE = """VSVersionInfo(
  ffi=FixedFileInfo(filevers={tup}, prodvers={tup}),
  kids=[
    StringFileInfo([StringTable('040904B0', [
      StringStruct('CompanyName', 'LyricTools++'),
      StringStruct('FileDescription', 'LyricTools++ lyric notepad'),
      StringStruct('FileVersion', '{ver}'),
      StringStruct('InternalName', 'LyricToolsPP'),
      StringStruct('LegalCopyright', 'MIT License'),
      StringStruct('OriginalFilename', 'LyricToolsPP.exe'),
      StringStruct('ProductName', 'LyricTools++'),
      StringStruct('ProductVersion', '{ver}')])]),
    VarFileInfo([VarStruct('Translation', [1033, 1200])])
  ]
)
"""

with open(os.path.join(HERE, "version_info.txt"), "w", encoding="utf-8") as fh:
    fh.write(TEMPLATE.format(tup=tup, ver=__version__))
print(__version__)
