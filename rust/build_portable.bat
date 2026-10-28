@echo off
rem Build the portable LyricTools++ (Rust) exe + zip on Windows.
rem Needs Rust from https://rustup.rs (default MSVC toolchain is fine).
setlocal
cd /d "%~dp0"

echo [1/3] Running tests...
cargo test --release
if errorlevel 1 goto :fail

echo [2/3] Building...
cargo build --release
if errorlevel 1 goto :fail

for /f "tokens=3 delims= " %%v in ('findstr /b "version" Cargo.toml') do set VERSION=%%~v
set NAME=LyricToolsPP-%VERSION%-portable-win64
set OUT=dist\%NAME%
if exist "%OUT%" rmdir /s /q "%OUT%"
mkdir "%OUT%"
copy /y target\release\LyricToolsPP.exe "%OUT%\" >nul
copy /y ..\LICENSE "%OUT%\LICENSE.txt" >nul
copy /y data\CMUDICT_LICENSE.txt "%OUT%\" >nul
rem The marker keeps settings.json next to the exe (nothing in AppData).
echo This copy of LyricTools++ is portable: settings are stored in settings.json in this folder.> "%OUT%\portable.txt"

echo [3/3] Zipping...
if exist "dist\%NAME%.zip" del "dist\%NAME%.zip"
powershell -NoProfile -Command "Compress-Archive -Path '%OUT%' -DestinationPath 'dist\%NAME%.zip'"
if errorlevel 1 goto :fail

echo.
echo Done: %OUT%\LyricToolsPP.exe
exit /b 0

:fail
echo BUILD FAILED
exit /b 1
