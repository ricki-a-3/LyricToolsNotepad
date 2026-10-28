@echo off
rem Build the portable LyricTools++ folder + zip.
rem   build_portable.bat            build (runs tests first)
rem   build_portable.bat notest     build without running tests
setlocal
cd /d "%~dp0"

set VENV=.venv-build
if not exist "%VENV%\Scripts\python.exe" (
    echo [1/5] Creating build environment...
    py -3 -m venv "%VENV%" 2>nul || python -m venv "%VENV%"
    if errorlevel 1 goto :fail
)
set PY=%VENV%\Scripts\python.exe

"%PY%" -c "import sys; sys.exit(sys.version_info < (3, 12))"
if errorlevel 1 (
    echo Python 3.12 or newer is required. Install it from python.org, then delete %VENV% and retry.
    goto :fail
)

echo [2/5] Installing dependencies...
"%PY%" -m pip install --quiet --upgrade pip
"%PY%" -m pip install --quiet -e ".[dev]"
if errorlevel 1 goto :fail

if /i not "%~1"=="notest" (
    echo [3/5] Running tests...
    "%PY%" -m pytest -q
    if errorlevel 1 goto :fail
) else (
    echo [3/5] Skipping tests.
)

echo [4/5] Building exe...
for /f %%v in ('"%PY%" packaging\make_version_info.py') do set VERSION=%%v
"%PY%" -m PyInstaller --noconfirm --clean --distpath dist --workpath build packaging\lyrictools.spec
if errorlevel 1 goto :fail

set OUT=dist\LyricToolsPP
rem The marker makes the app keep settings.ini next to the exe (no AppData/registry).
echo This copy of LyricTools++ is portable: settings are stored in settings.ini in this folder.> "%OUT%\portable.txt"
copy /y LICENSE "%OUT%\LICENSE.txt" >nul
copy /y src\lyrictools\data\CMUDICT_LICENSE.txt "%OUT%\CMUDICT_LICENSE.txt" >nul

echo [5/5] Zipping...
set ZIP=dist\LyricToolsPP-%VERSION%-portable-win64.zip
if exist "%ZIP%" del "%ZIP%"
powershell -NoProfile -Command "Compress-Archive -Path '%OUT%' -DestinationPath '%ZIP%'"
if errorlevel 1 goto :fail

echo.
echo Done.
echo   Run:   %OUT%\LyricToolsPP.exe
echo   Share: %ZIP%
exit /b 0

:fail
echo.
echo BUILD FAILED
exit /b 1
