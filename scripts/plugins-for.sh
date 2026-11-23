#!/bin/sh
# Prints, one per line, the folder of each plugin whose manifest lists the
# system given ("linux" or "windows") in its top-level `platforms`: the plugins
# the workflows build and release for that system. A manifest without it is
# for every system, as Herdr reads it. Run from the repository's root:
# sh scripts/plugins-for.sh linux
# A `platforms` this script cannot read fails, saying so: a plugin is never
# left out without a word. Tested by scripts/plugins-for.test.sh.
# See docs/en/adding-plugins.md.
set -eu

platform=${1:?usage: plugins-for.sh <linux|windows>}

# Reads a manifest's top level, before its first table (a [[build]] step has a
# platforms of its own), and prints:
#   absent           when it has no platforms;
#   system <name>    for each system its platforms lists;
#   bad <reason>     when platforms is not an array of strings it can read.
# It follows strings, multi-line ones included, comments and nested brackets,
# so that a line starting with `[` inside another value is not a table.
read_platforms() {
  tr -d '\r' < "$1" | awk -v q="'" '
    # The value of platforms, read up to its end: its tokens, one per entry of
    # tok[], strings with their quotes. q is a single quote.
    function add(t) { if (capture) tok[++ntok] = t }
    function fail(why) { print "bad " why; done = 1; exit }
    BEGIN { depth = 0; ml = ""; capture = 0; started = 0; ntok = 0; done = 0 }
    {
      line = $0
      if (ml == "" && depth == 0) {
        if (line ~ /^[ \t]*\[/) exit
        if (match(line, "^[ \t]*(\"platforms\"|" q "platforms" q "|platforms)[ \t]*=")) {
          capture = 1
          started = 1
          line = substr(line, RLENGTH + 1)
        }
      }
      n = length(line)
      i = 1
      while (i <= n) {
        c = substr(line, i, 1)
        if (ml != "") {
          if (substr(line, i, 3) == ml) { i += 3; ml = ""; if (capture) fail("multi-line string") }
          else i++
          continue
        }
        if (c == "#") break
        if (substr(line, i, 3) == "\"\"\"" || substr(line, i, 3) == q q q) {
          ml = substr(line, i, 3)
          i += 3
          continue
        }
        if (c == "\"" || c == q) {
          j = i + 1
          while (j <= n && substr(line, j, 1) != c) {
            if (c == "\"" && substr(line, j, 1) == "\\") {
              if (capture) fail("escape in a string")
              j++
            }
            j++
          }
          if (j > n) fail("unterminated string")
          add(substr(line, i, j - i + 1))
          i = j + 1
          continue
        }
        if (c == "[" || c == "{") depth++
        else if (c == "]" || c == "}") depth--
        if (c != " " && c != "\t") add(c)
        i++
      }
      if (capture && ml == "" && depth == 0) { capture = 0; exit }
    }
    END {
      if (done) exit
      if (!started) { print "absent"; exit }
      if (capture) fail("unterminated array")
      # [ ] or [ s (, s)* [,] ]
      if (tok[1] != "[" || tok[ntok] != "]") fail("not an array")
      k = 2
      while (k < ntok) {
        t = tok[k]
        first = substr(t, 1, 1)
        if (length(t) < 2 || (first != "\"" && first != q) || substr(t, length(t), 1) != first) fail("not a string: " t)
        print "system " substr(t, 2, length(t) - 2)
        k++
        if (k < ntok && tok[k] != ",") fail("missing comma after " t)
        k++
      }
    }
  '
}

for manifest in */herdr-plugin.toml; do
  plugin=${manifest%/herdr-plugin.toml}
  listed=$(read_platforms "$manifest")
  bad=$(printf '%s\n' "$listed" | sed -n 's/^bad //p')
  if [ -n "$bad" ]; then
    echo "::error file=$manifest::plugins-for: platforms unreadable ($bad): write platforms = [\"linux\", \"windows\"]" >&2
    exit 1
  fi
  case $listed in
    absent)
      echo "$plugin"
      ;;
    *)
      if printf '%s\n' "$listed" | grep -Fqx "system $platform"; then
        echo "$plugin"
      fi
      ;;
  esac
done
