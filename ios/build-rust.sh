#!/bin/bash
# Build the Rust lyric engine for iOS and generate its Swift bindings.
# Runs on macOS (locally or in CI). Output goes to ios/Generated/:
#   LyricsFFI.xcframework   static library for iPhone + Apple-silicon simulator
#   bindings/lyrics_ffi.swift  the Swift API (compiled into the app target)
set -euo pipefail
cd "$(dirname "$0")/.."

export IPHONEOS_DEPLOYMENT_TARGET=17.0
rustup target add aarch64-apple-ios aarch64-apple-ios-sim

cargo build -p lyrics-ffi --release --target aarch64-apple-ios
cargo build -p lyrics-ffi --release --target aarch64-apple-ios-sim
# A host build, just so uniffi-bindgen can read the API metadata.
cargo build -p lyrics-ffi --release

OUT=ios/Generated
rm -rf "$OUT"
mkdir -p "$OUT/bindings" "$OUT/headers"
cargo run -q -p lyrics-ffi --release --bin uniffi-bindgen -- generate \
  --library target/release/liblyrics_ffi.dylib --language swift --out-dir "$OUT/bindings"

cp "$OUT/bindings/lyrics_ffiFFI.h" "$OUT/headers/"
cp "$OUT/bindings/lyrics_ffiFFI.modulemap" "$OUT/headers/module.modulemap"

xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/liblyrics_ffi.a -headers "$OUT/headers" \
  -library target/aarch64-apple-ios-sim/release/liblyrics_ffi.a -headers "$OUT/headers" \
  -output "$OUT/LyricsFFI.xcframework"

echo "Built $OUT/LyricsFFI.xcframework"
