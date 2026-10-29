@echo off
rem Run LyricTools++ straight from source (no build). Uses the build venv.
cd /d "%~dp0"
if not exist ".venv-build\Scripts\pythonw.exe" (
    echo Run build_portable.bat once first to set up .venv-build.
    pause
    exit /b 1
)
start "" ".venv-build\Scripts\pythonw.exe" -m lyrictools %*
