#!/usr/bin/env bash
# Assembles the web page and the browser demo into OUT_DIR (default _site):
# the files in site/, the README screenshots, the icon, the heading font and
# the demo compiled to WebAssembly in demo/pkg/.
#
# Needs the wasm32-unknown-unknown target and the wasm-bindgen CLI at the
# same version as the wasm-bindgen crate in Cargo.lock (override the binary
# with WASM_BINDGEN=path).
set -euo pipefail

cd "$(dirname "$0")/.."
out="${1:-_site}"
wasm_bindgen="${WASM_BINDGEN:-wasm-bindgen}"

rm -rf "$out"
mkdir -p "$out/img" "$out/fonts"
cp -r site/. "$out/"
rm "$out/build.sh"
cp docs/screenshots/*.png resources/icon.png "$out/img/"
cp crates/core/fonts/Excalifont/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.woff2 "$out/fonts/excalifont.woff2"
cp crates/core/fonts/OFL.txt "$out/fonts/"

# The `web` profile (Cargo.toml) optimizes for size: about half the download.
cargo build --profile web --locked -p littledraw-web --target wasm32-unknown-unknown
"$wasm_bindgen" --target web --no-typescript --remove-name-section --remove-producers-section \
    --out-dir "$out/demo/pkg" target/wasm32-unknown-unknown/web/littledraw_web.wasm
echo "site ready in $out ($(du -h "$out/demo/pkg/littledraw_web_bg.wasm" | cut -f1) of WebAssembly)"
