#!/bin/bash
# Build the Windows release (portable zip + installer) from Linux.
# Needs: rustup target add x86_64-pc-windows-gnu
#        apt install gcc-mingw-w64-x86-64 nsis zip
set -euo pipefail
cd "$(dirname "$0")"
cargo test --release
cargo build --release --target x86_64-pc-windows-gnu
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
STAGE=dist/stage
rm -rf dist && mkdir -p "$STAGE"
cp target/x86_64-pc-windows-gnu/release/LyricToolsPP.exe "$STAGE/"
cp LICENSE "$STAGE/LICENSE.txt"
cp data/CMUDICT_LICENSE.txt assets/fonts/FONT_LICENSE.txt "$STAGE/"

# Portable: the marker keeps settings.json beside the exe.
NAME="LyricToolsPP-${VERSION}-portable-win64"
cp -r "$STAGE" "dist/$NAME"
printf 'This copy of LyricTools++ is portable: settings are stored in settings.json in this folder.\r\n' > "dist/$NAME/portable.txt"
(cd dist && zip -qr "$NAME.zip" "$NAME")

makensis -V2 -DVERSION="$VERSION" -DSTAGE="../$STAGE" -DOUTFILE="../dist/LyricToolsPP-${VERSION}-setup.exe" installer/LyricToolsPP.nsi
ls -1 dist/*.zip dist/*.exe
