#!/bin/sh
# Tests scripts/plugins-for.sh on manifests written for the purpose, in a
# folder of their own: sh scripts/plugins-for.test.sh. Run by the Plugins
# workflow.
set -eu

script=$(cd "$(dirname "$0")" && pwd)/plugins-for.sh
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
failures=0

# manifest <plugin> <text>: the plugin's herdr-plugin.toml.
manifest() {
  mkdir -p "$work/$1"
  printf '%s\n' "$2" > "$work/$1/herdr-plugin.toml"
}

# expect <system> <plugins…>: what the script prints for that system.
expect() {
  system=$1
  shift
  want=$(printf '%s\n' "$@")
  got=$(cd "$work" && sh "$script" "$system") || {
    echo "FAIL $system: the script failed"
    failures=$((failures + 1))
    return
  }
  if [ "$got" != "$want" ]; then
    echo "FAIL $system: got [$(echo $got)], want [$(echo "$@")]"
    failures=$((failures + 1))
  fi
}

# refused <plugin> <text>: a manifest the script must fail on, saying so.
refused() {
  rm -rf "$work"/*
  manifest "$1" "$2"
  if errors=$(cd "$work" && sh "$script" linux 2>&1 >/dev/null); then
    echo "FAIL $1: accepted"
    failures=$((failures + 1))
  elif ! printf '%s\n' "$errors" | grep -q "^::error file=$1/herdr-plugin.toml::"; then
    echo "FAIL $1: unclear error: $errors"
    failures=$((failures + 1))
  fi
}

manifest both 'id = "a.both"
platforms = ["linux", "windows"]'
manifest single 'platforms = [ '"'"'windows'"'"' ] # Windows only'
manifest lines 'description = """
[not a table]
"""
platforms = [
  "linux", # musl
  # "windows",
]

[[build]]
platforms = ["windows"]'
manifest none 'id = "a.none"
name = "[x]"

[[build]]
platforms = ["linux"]'
manifest empty 'platforms = []'
manifest nested 'notes = [
[1, 2],
]
platforms = ["windows"]'
manifest quoted '"platforms" = ["linux"]'
expect linux both lines none quoted
expect windows both nested none single

refused string 'platforms = "linux"'
refused escaped 'platforms = ["lin\u0075x"]'
refused unterminated 'platforms = ["linux",
'
refused number 'platforms = ["linux", 1]'
refused comma 'platforms = ["linux" "windows"]'
refused multiline 'platforms = ["""linux"""]'

if [ "$failures" -ne 0 ]; then
  echo "$failures failure(s)"
  exit 1
fi
echo "plugins-for.sh: every case passes"
