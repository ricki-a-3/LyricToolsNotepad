# PyInstaller spec for LyricTools++ (one-folder build).
# Build from the repo root:  pyinstaller --noconfirm packaging/lyrictools.spec
import os

ROOT = os.path.abspath(os.path.join(SPECPATH, ".."))
PKG = os.path.join(ROOT, "src", "lyrictools")

a = Analysis(
    [os.path.join(SPECPATH, "launcher.py")],
    pathex=[os.path.join(ROOT, "src")],
    datas=[
        (os.path.join(PKG, "data"), "lyrictools/data"),
        (os.path.join(PKG, "resources"), "lyrictools/resources"),
    ],
    # Only QtCore/QtGui/QtWidgets are used; keep the bundle lean.
    excludes=["tkinter", "PySide6.QtNetwork", "PySide6.QtQml", "PySide6.QtQuick",
              "PySide6.QtPdf", "PySide6.QtSvg", "PySide6.QtOpenGL", "PySide6.QtDBus"],
    noarchive=False,
)
pyz = PYZ(a.pure)
exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name="LyricToolsPP",
    icon=os.path.join(PKG, "resources", "start.ico"),
    console=False,
    upx=False,  # UPX-packed exes trip antivirus far more often
    version=os.path.join(SPECPATH, "version_info.txt") if os.name == "nt" else None,
)
coll = COLLECT(exe, a.binaries, a.datas, strip=False, upx=False, name="LyricToolsPP")
