#!/usr/bin/env bash
# S4 evidence check (completeness only; never judges the outcome, except that a spike-started process survived).
set -u
DIR="$(cd "$(dirname "$0")" && pwd -P)"
R="$DIR/raw"
fail=0
chk() { local d="$1"; shift; if "$@" >/dev/null 2>&1; then echo "ok   $d"; else echo "FAIL $d"; fail=1; fi; }
for n in 1 2 3; do
  for f in descendants before after leftover sweep final; do
    chk "r$n $f file exists" test -s "$R/s4-r$n-$f.txt"
  done
  chk "r$n final reads none (no spike-started process survived)" bash -c "[ \"\$(tr -d ' \n' < '$R/s4-r$n-final.txt')\" = none ]"
  chk "r$n stream file exists" test -f "$R/s4-r$n.jsonl"
done
chk "s4-exit.txt has 3 lines" bash -c "[ \$(wc -l < '$R/s4-exit.txt') -eq 3 ]"
chk "s4-env.txt has nested=" grep -qE '^nested=(yes|no)$' "$R/s4-env.txt"
chk "run.sh has no name-based signalling tool" bash -c "! grep -v '^[[:space:]]*#' '$DIR/run.sh' | grep -qE '\\b(pkill|killall)\\b'"
exit $fail
