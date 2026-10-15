#!/usr/bin/env bash
# S2 evidence check (completeness only; never judges the outcome).
set -u
DIR="$(cd "$(dirname "$0")" && pwd -P)"
R="$DIR/raw"
fail=0
chk() { local d="$1"; shift; if "$@" >/dev/null 2>&1; then echo "ok   $d"; else echo "FAIL $d"; fail=1; fi; }
for f in s2-1-create s2-2-resume s2-3-fork; do
  chk "$f.json has a session_id field (or a recorded error)" bash -c "jq -e '(.session_id|type)==\"string\" or (.is_error==true) or has(\"error\")' '$R/$f.json'"
done
chk "hash file has a before line" grep -q '^before ' "$R/s2-transcript-hash.txt"
chk "hash file has an after line" grep -q '^after ' "$R/s2-transcript-hash.txt"
for k in create_id resume_same_id fork_new_id transcript_unchanged fork_answer_has_43 cache_read_resume cache_read_fork; do
  chk "observation key $k" grep -qE "^$k=" "$R/s2-observations.txt"
done
for k in resume_same_id fork_new_id transcript_unchanged fork_answer_has_43; do
  chk "observation $k is yes|no" grep -qE "^$k=(yes|no)$" "$R/s2-observations.txt"
done
exit $fail
