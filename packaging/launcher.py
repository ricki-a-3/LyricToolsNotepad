"""PyInstaller entry point (a package's __main__ can't be frozen directly)."""
import sys

from lyrictools.app import main

if __name__ == "__main__":
    sys.exit(main())
