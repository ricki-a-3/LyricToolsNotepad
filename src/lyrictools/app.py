"""Application entry point."""
import os
import sys

from PySide6.QtCore import QSettings
from PySide6.QtWidgets import QApplication

from . import __version__, core

ORG = "LyricToolsPP"
APP = "LyricTools++"


def app_dir():
    """Folder holding the .exe when frozen, else the repo root."""
    if getattr(sys, "frozen", False):
        return os.path.dirname(sys.executable)
    return os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def make_settings():
    """Portable builds ship a ``portable.txt`` marker next to the exe, and keep
    their settings in ``settings.ini`` beside it (nothing in the registry or
    AppData, so the folder can live on a USB stick). Installed builds use the
    normal per-user location."""
    base = app_dir()
    if os.path.exists(os.path.join(base, "portable.txt")):
        return QSettings(os.path.join(base, "settings.ini"), QSettings.Format.IniFormat)
    return QSettings(QSettings.Format.IniFormat, QSettings.Scope.UserScope, ORG, "LyricTools")


def main(argv=None):
    argv = sys.argv if argv is None else argv
    if sys.platform == "win32":
        # Own taskbar identity, so Windows shows our icon rather than Python's.
        try:
            import ctypes
            ctypes.windll.shell32.SetCurrentProcessExplicitAppUserModelID("LyricToolsPP.App")
        except Exception:
            pass

    app = QApplication(argv)
    app.setOrganizationName(ORG)
    app.setApplicationName(APP)
    app.setApplicationVersion(__version__)

    # Syllable counts are needed for the first paint; rhyme indexes are built
    # in the background by LookupService.
    core.load_cmudict()

    from .mainwindow import MainWindow
    window = MainWindow(make_settings())
    files = [a for a in argv[1:] if not a.startswith("-")]
    if files:
        window.load(files[0])
    window.show()
    return app.exec()


if __name__ == "__main__":
    sys.exit(main())
