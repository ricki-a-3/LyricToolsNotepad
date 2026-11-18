#!/bin/bash
# Build the Windows release (portable zip + installer) from Linux.
# Needs: rustup target add x86_64-pc-windows-gnu
#        apt install gcc-mingw-w64-x86-64 nsis zip
set -euo pipefail
cd "$(dirname "$0")"
cargo test --workspace --release
cargo build --release --target x86_64-pc-windows-gnu
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
STAGE=dist/stage
rm -rf dist && mkdir -p "$STAGE"
cp target/x86_64-pc-windows-gnu/release/LyricToolsNotepad.exe "$STAGE/"
cp LICENSE "$STAGE/LICENSE.txt"
cp crates/lyrics-core/data/CMUDICT_LICENSE.txt assets/fonts/FONT_LICENSE.txt "$STAGE/"

# Installer first, so the portable zip can carry it too.
SETUP="LyricToolsNotepad-${VERSION}-setup.exe"
makensis -V2 -DVERSION="$VERSION" -DSTAGE="../$STAGE" -DOUTFILE="../dist/$SETUP" installer/LyricToolsNotepad.nsi

# Portable: the marker keeps settings.json beside the exe. The installer rides
# along, so one download gives you both.
NAME="LyricToolsNotepad-${VERSION}-portable-win64"
cp -r "$STAGE" "dist/$NAME"
printf 'This copy of LyricToolsNotepad is portable: settings are stored in settings.json in this folder.\r\nTo install it instead, run %s.\r\n' "$SETUP" > "dist/$NAME/portable.txt"
cp "dist/$SETUP" "dist/$NAME/"
(cd dist && zip -qr "$NAME.zip" "$NAME")
ls -1 dist/*.zip dist/*.exe
