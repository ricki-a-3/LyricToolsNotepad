@echo off
rem Build the Windows release on Windows: portable zip + installer.
rem Needs Rust (https://rustup.rs). The installer also needs NSIS 3
rem (https://nsis.sourceforge.io) -- without it you just get the zip.
setlocal
cd /d "%~dp0"

echo [1/4] Running tests...
cargo test --release
if errorlevel 1 goto :fail

echo [2/4] Building...
cargo build --release
if errorlevel 1 goto :fail

for /f "tokens=3 delims= " %%v in ('findstr /b "version" Cargo.toml') do set VERSION=%%~v
set STAGE=dist\stage
if exist dist rmdir /s /q dist
mkdir "%STAGE%"
copy /y target\release\LyricToolsPP.exe "%STAGE%\" >nul
copy /y LICENSE "%STAGE%\LICENSE.txt" >nul
copy /y data\CMUDICT_LICENSE.txt "%STAGE%\" >nul
copy /y assets\fonts\FONT_LICENSE.txt "%STAGE%\" >nul

echo [3/4] Portable zip...
set NAME=LyricToolsPP-%VERSION%-portable-win64
xcopy /e /i /q "%STAGE%" "dist\%NAME%" >nul
echo This copy of LyricTools++ is portable: settings are stored in settings.json in this folder.> "dist\%NAME%\portable.txt"
powershell -NoProfile -Command "Compress-Archive -Path 'dist\%NAME%' -DestinationPath 'dist\%NAME%.zip'"
if errorlevel 1 goto :fail

echo [4/4] Installer...
set MAKENSIS=makensis
where makensis >nul 2>nul || set MAKENSIS="%ProgramFiles(x86)%\NSIS\makensis.exe"
%MAKENSIS% -V2 -DVERSION=%VERSION% -DSTAGE=..\%STAGE% -DOUTFILE=..\dist\LyricToolsPP-%VERSION%-setup.exe installer\LyricToolsPP.nsi
if errorlevel 1 echo (Installer skipped: install NSIS 3 to build it.)

echo.
echo Done. See the dist folder.
exit /b 0

:fail
echo BUILD FAILED
exit /b 1
