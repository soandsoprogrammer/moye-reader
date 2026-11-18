#!/bin/sh
set -eu

if [ "$(uname -s)" != Linux ] || [ "$(uname -m)" != x86_64 ]; then
    printf '%s\n' '请在 x86_64 Linux 上构建 Steam Deck 版。' >&2
    exit 1
fi

cd "$(dirname "$0")/.."
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
app="$stage/MoYe"
mkdir -p "$app/fonts" dist
download_font() {
    upstream="https://raw.githubusercontent.com/notofonts/noto-cjk/$1"
    curl --fail --silent --show-error --location --retry 3 \
        "$upstream/$2/OTF/SimplifiedChinese/$3" --output "$app/fonts/$3"
    curl --fail --silent --show-error --location --retry 3 \
        "$upstream/$4" --output "$app/fonts/$2-LICENSE.txt"
}
download_font Sans2.004 Sans NotoSansCJKsc-Regular.otf LICENSE
download_font Serif2.003 Serif NotoSerifCJKsc-Regular.otf Serif/LICENSE

cargo build --release --locked
install -m 755 "${CARGO_TARGET_DIR:-target}/release/moye-reader" "$app/moye-reader"
install -m 755 scripts/launch-moye.sh "$app/launch-moye.sh"
install -m 644 README.md "$app/README.md"
tar -C "$stage" -czf dist/MoYe-SteamDeck-x86_64.tar.gz MoYe
