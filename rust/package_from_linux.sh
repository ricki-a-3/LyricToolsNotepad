#!/bin/bash
# Cross-compile the Windows portable zip from Linux.
# Needs: rustup target add x86_64-pc-windows-gnu; apt install gcc-mingw-w64-x86-64 zip
set -euo pipefail
cd "$(dirname "$0")"
cargo test --release
cargo build --release --target x86_64-pc-windows-gnu
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
NAME="LyricToolsPP-${VERSION}-portable-win64"
OUT="dist/${NAME}"
rm -rf "$OUT" "dist/${NAME}.zip"
mkdir -p "$OUT"
cp target/x86_64-pc-windows-gnu/release/LyricToolsPP.exe "$OUT/"
cp ../LICENSE "$OUT/LICENSE.txt"
cp data/CMUDICT_LICENSE.txt "$OUT/"
printf 'This copy of LyricTools++ is portable: settings are stored in settings.json in this folder.\r\n' > "$OUT/portable.txt"
(cd dist && zip -qr "${NAME}.zip" "${NAME}")
echo "dist/${NAME}.zip"
