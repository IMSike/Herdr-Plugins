#!/bin/sh
# A plugin's [[build]] step on Linux: puts its binary in target/release/, as
# cargo would. The binary comes from the release tagged on the very commit
# Herdr checked out, checked against that release's SHA256SUMS; the checkout
# must be unchanged. Without such a release (a push to main still being
# published, a commit never released, a fork), or on another system, it builds
# with cargo when cargo is there. Run from the plugin's folder with its binary's
# name: sh ../scripts/fetch-bin.sh herdr-menu
# See docs/en/adr/0008-prebuilt-binaries.md.
set -eu

name=${1:?usage: fetch-bin.sh <binary>}

fail() {
  echo "fetch-bin: $1" >&2
  exit 1
}

# No release binary to use: builds with cargo, or says why it cannot.
build() {
  command -v cargo >/dev/null 2>&1 ||
    fail "$1; no cargo to build it with either (Rust 1.95 or later)"
  echo "fetch-bin: $1: building with cargo" >&2
  exec cargo build --release --locked --target-dir target
}

case "$(uname -s) $(uname -m)" in
  "Linux x86_64" | "Linux amd64") target=x86_64-unknown-linux-musl ;;
  "Linux aarch64" | "Linux arm64") target=aarch64-unknown-linux-musl ;;
  *) build "no release binary for $(uname -s) $(uname -m)" ;;
esac
asset="$name-$target"

command -v git >/dev/null 2>&1 || build "git not found"
commit=$(git rev-parse HEAD 2>/dev/null) || build "not a Git checkout"
[ -z "$(git status --porcelain 2>/dev/null)" ] || build "files changed since commit $commit"

origin=$(git config --get remote.origin.url 2>/dev/null || true)
repo=$(printf '%s\n' "$origin" |
  sed -nE 's#^(https://|ssh://git@|git@)github\.com[:/]([A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+)$#\2#p')
repo=${repo%.git}
[ -n "$repo" ] || build "origin is not a GitHub repository"

# The tag on the commit: a lightweight one gives the commit itself, an annotated
# one gives it on its peeled (^{}) line.
tag=$(git ls-remote --tags origin 2>/dev/null |
  awk -v c="$commit" '$1 == c && $2 ~ /^refs\/tags\/v/ { t = $2; sub(/^refs\/tags\//, "", t); sub(/\^\{\}$/, "", t); print t; exit }')
[ -n "$tag" ] || build "commit $commit has no release yet (one just pushed is published within minutes: herdr plugin update then)"
url="https://github.com/$repo/releases/download/$tag"

command -v curl >/dev/null 2>&1 || build "curl not found"
if command -v sha256sum >/dev/null 2>&1; then
  sha() { sha256sum "$1" | cut -d' ' -f1; }
elif command -v shasum >/dev/null 2>&1; then
  sha() { shasum -a 256 "$1" | cut -d' ' -f1; }
else
  build "neither sha256sum nor shasum found"
fi

dir=target/release
mkdir -p "$dir"
tmp="$dir/.fetch-$$"
trap 'rm -f "$tmp" "$tmp.sums"' EXIT
trap 'exit 1' INT TERM HUP

curl -fsSL -o "$tmp" "$url/$asset" || build "cannot download $url/$asset"
curl -fsSL -o "$tmp.sums" "$url/SHA256SUMS" || build "cannot download $url/SHA256SUMS"
want=$(awk -v a="$asset" '$2 == a || $2 == "*" a { print $1; exit }' "$tmp.sums")
case "$want" in
  *[!0-9a-f]* | "") fail "$asset is not in the SHA256SUMS of $tag" ;;
esac
[ ${#want} -eq 64 ] || fail "$asset is not in the SHA256SUMS of $tag"
[ "$(sha "$tmp")" = "$want" ] || fail "$asset does not match the SHA256SUMS of $tag: not installed"

chmod +x "$tmp"
mv -f "$tmp" "$dir/$name"
echo "fetch-bin: $name $tag ($asset)"
