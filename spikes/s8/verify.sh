#!/usr/bin/env bash
# S8 evidence check (completeness only; never judges the outcome).
set -u
DIR="$(cd "$(dirname "$0")" && pwd -P)"
R="$DIR/raw"
fail=0
chk() { local d="$1"; shift; if "$@" >/dev/null 2>&1; then echo "ok   $d"; else echo "FAIL $d"; fail=1; fi; }
for v in a b; do
  chk "s8-$v.json has subtype and num_turns" jq -e '(.subtype|type)=="string" and (.num_turns|type)=="number"' "$R/s8-$v.json"
done
chk "exit file has A line" grep -qE '^A exit_code=[0-9]+$' "$R/s8-exit.txt"
chk "exit file has B line" grep -qE '^B exit_code=[0-9]+$' "$R/s8-exit.txt"
chk "transcripts file has before= and after=" grep -qE '^before=[0-9]+ after=[0-9]+' "$R/s8-transcripts.txt"
chk "diff patch non-empty" test -s "$R/s8-diff.patch"
exit $fail
