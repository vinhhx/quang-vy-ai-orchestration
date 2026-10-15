#!/usr/bin/env bash
# S7 evidence check (completeness only; never judges the outcome).
set -u
DIR="$(cd "$(dirname "$0")" && pwd -P)"
R="$DIR/raw"
fail=0
chk() { local d="$1"; shift; if "$@" >/dev/null 2>&1; then echo "ok   $d"; else echo "FAIL $d"; fail=1; fi; }
for f in s7-version.txt s7-help.txt s7-json.json s7-stream.jsonl s7-maxturns.txt; do
  chk "$f has exit_code line" grep -qE '^exit_code=[0-9]+$' "$R/$f"
done
chk "NOTES.md has Nguon labels" bash -c "[ \"\$(grep -c 'Nguồn:' '$DIR/NOTES.md')\" -ge 3 ]"
chk "NOTES.md states docs-only or docs+local" grep -qE 'docs-only|docs \+ chạy local|docs cộng chạy local' "$DIR/NOTES.md"
# every non-comment run.sh line that invokes agy carries timeout 10 and </dev/null
bad=$(grep -v '^[[:space:]]*#' "$DIR/run.sh" | grep -E '(^|[[:space:];])agy([[:space:]]|$)' | grep -vE '^[[:space:]]*(command|echo|case|if|printf)' | grep -vE 'timeout 10 .*</dev/null' || true)
if [ -z "$bad" ]; then echo "ok   agy lines bounded"; else echo "FAIL agy lines bounded: $bad"; fail=1; fi
names=$(grep -v '^[[:space:]]*#' "$DIR/run.sh" | grep -oE '\b(agy|antigravity)\b' | sort -u | tr '\n' ' ')
if [ "$names" = "agy " ]; then echo "ok   only agy named"; else echo "FAIL names: $names"; fail=1; fi
exit $fail
