@echo off
setlocal enabledelayedexpansion
title LyricTools++ Installer

echo ================================================
echo   LyricTools++ Installer
echo ================================================
echo.

rem --- This installer copies the LyricTool folder that sits next to it
rem --- into your Notepad++ PythonScript folder. It looks for that
rem --- folder in the usual places Notepad++ puts it.

set "SRC_DIR=%~dp0"
set "SRC_LYRICTOOL=%SRC_DIR%LyricTool"

if not exist "%SRC_LYRICTOOL%" (
    echo ERROR: Could not find the "LyricTool" folder next to this
    echo installer. Make sure Install_LyricToolsPP.bat is still inside
    echo the folder you unzipped everything into, then try again.
    echo.
    pause
    exit /b 1
)

echo Looking for your Notepad++ PythonScript folder...
echo.

set "CANDIDATES[0]=%APPDATA%\Notepad++\plugins\config\PythonScript\scripts"
set "CANDIDATES[1]=%APPDATA%\Notepad++\plugins\Config\PythonScript\scripts"
set "CANDIDATES[2]=%ProgramFiles%\Notepad++\plugins\PythonScript\scripts"
set "CANDIDATES[3]=%ProgramFiles%\Notepad++\plugins\Config\PythonScript\scripts"
set "CANDIDATES[4]=%ProgramFiles(x86)%\Notepad++\plugins\PythonScript\scripts"
set "CANDIDATES[5]=%ProgramFiles(x86)%\Notepad++\plugins\Config\PythonScript\scripts"
set "CANDIDATES[6]=%LocalAppData%\Programs\Notepad++\plugins\PythonScript\scripts"
set "CANDIDATES[7]=%LocalAppData%\Programs\Notepad++\plugins\Config\PythonScript\scripts"

set "SCRIPTS_ROOT="
for /L %%i in (0,1,7) do (
    if not defined SCRIPTS_ROOT (
        if exist "!CANDIDATES[%%i]!" set "SCRIPTS_ROOT=!CANDIDATES[%%i]!"
    )
)

if not defined SCRIPTS_ROOT (
    echo Could not find a PythonScript folder anywhere Notepad++ usually
    echo keeps it. This almost always means the PythonScript plugin
    echo itself isn't installed yet ^(LyricTools++ is built on top of it^).
    echo.
    echo To install PythonScript:
    echo   1. Open Notepad++
    echo   2. Plugins menu, then "Plugins Admin"
    echo   3. Search for "PythonScript", check its box, click Install
    echo   4. Let Notepad++ restart when it asks to
    echo   5. Run this installer again
    echo.
    pause
    exit /b 1
)

echo Found it:
echo   %SCRIPTS_ROOT%
echo.

set "TARGET=%SCRIPTS_ROOT%"
set "STARTUP_PY=%SCRIPTS_ROOT%\startup.py"

if exist "%TARGET%\LyricTool" (
    echo An existing LyricTools++ install was found and will be replaced
    echo with this version.
    rmdir /s /q "%TARGET%\LyricTool"
    echo.
)

xcopy "%SRC_LYRICTOOL%" "%TARGET%\LyricTool\" /e /i /y >nul

if errorlevel 1 (
    echo.
    echo Something went wrong copying the files. This is usually one of:
    echo   - Notepad++ is open right now -- close it completely and
    echo     try running this installer again.
    echo   - This installer needs permission to write there -- right-click
    echo     Install_LyricToolsPP.bat and choose "Run as administrator".
    echo.
    pause
    exit /b 1
)

echo Files installed successfully.
echo.

set "CHOICE=y"
set /p CHOICE=Start LyricTools++ automatically every time Notepad++ opens? [Y/n]: 

if /i "%CHOICE%"=="n" goto skip_autostart

findstr /c:"LyricTools++ autostart" "%STARTUP_PY%" >nul 2>&1
if not errorlevel 1 (
    echo Auto-start was already set up from a previous install -- leaving it as is.
    goto after_autostart
)

type "%SRC_DIR%_lyrictools_autostart_snippet.py" >> "%STARTUP_PY%"
if errorlevel 1 (
    echo Could not set up auto-start, but the plugin itself is installed fine.
    echo You can still run it manually every time -- see the next steps below.
) else (
    echo Auto-start enabled.
)
goto after_autostart

:skip_autostart
echo Skipped auto-start. You'll need to run LyricToolsPP manually each
echo time you open Notepad++ -- see the next steps below.

:after_autostart
echo.
echo ================================================
echo   Done^!
echo ================================================
echo.
echo Next steps:
echo   1. Close Notepad++ completely if it's open right now, then start
echo      it again.
if /i "%CHOICE%"=="n" (
    echo   2. Plugins -^> PythonScript -^> Scripts -^> LyricTool -^> LyricToolsPP
    echo      ^(run this once each time you open Notepad++^)
    echo   3. Fully highlight ^(double-click^) any word in your lyrics to
    echo      see rhyme suggestions.
) else (
    echo   2. That's it -- LyricTools++ turns itself on automatically now.
    echo   3. Fully highlight ^(double-click^) any word in your lyrics to
    echo      see rhyme suggestions.
)
echo.
pause
