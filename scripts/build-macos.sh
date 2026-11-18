#!/bin/sh
set -eu

if [ "$(uname -s)" != Darwin ]; then
    printf '%s\n' '此脚本需要 macOS。' >&2
    exit 1
fi

cd "$(dirname "$0")/.."
cargo build --release --locked

mkdir -p dist
stage=$(mktemp -d "$PWD/dist/.moye-build.XXXXXX")
trap 'rm -rf "$stage"' EXIT
trap 'exit 1' HUP INT TERM
bundle="$stage/墨页.app"
app="$PWD/dist/墨页.app"

mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
install -m 755 "${CARGO_TARGET_DIR:-target}/release/moye-reader" "$bundle/Contents/MacOS/moye-reader"
install -m 644 packaging/Info.plist "$bundle/Contents/Info.plist"
install -m 644 packaging/AppIcon.icns "$bundle/Contents/Resources/AppIcon.icns"
xattr -cr "$bundle"
codesign --force --sign - "$bundle"
codesign --verify --strict "$bundle"

if [ -e "$app" ]; then
    mv "$app" "$stage/previous.app"
fi
if ! mv "$bundle" "$app"; then
    if [ -e "$stage/previous.app" ]; then
        mv "$stage/previous.app" "$app"
    fi
    exit 1
fi
printf '%s\n' "$app"
