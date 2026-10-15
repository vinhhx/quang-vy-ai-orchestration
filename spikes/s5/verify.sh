#!/usr/bin/env bash
# S5 evidence check (completeness only; never judges the outcome). `--clone` checks only the clone step.
set -u
DIR="$(cd "$(dirname "$0")" && pwd -P)"
R="$DIR/raw"
fail=0
chk() { local d="$1"; shift; if "$@" >/dev/null 2>&1; then echo "ok   $d"; else echo "FAIL $d"; fail=1; fi; }
chk "credentials_copied=no" grep -qx 'credentials_copied=no' "$R/s5-clone.txt"
chk "personal dir mode is 700" bash -c "[ \"\$(stat -c %a \"\$HOME/.claude-personal\")\" = 700 ]"
[ "${1:-}" = "--clone" ] && exit $fail
chk "work auth has configDirectory" jq -e 'has("configDirectory")' "$R/s5-auth-work.json"
chk "personal auth has configDirectory" jq -e 'has("configDirectory")' "$R/s5-auth-personal.json"
chk "personal configDirectory ends with .claude-personal" bash -c "jq -r .configDirectory '$R/s5-auth-personal.json' | grep -q '\\.claude-personal\$'"
chk "work hi has session_id" jq -e '(.session_id|type)=="string"' "$R/s5-hi-work.json"
chk "personal hi has session_id" jq -e '(.session_id|type)=="string"' "$R/s5-hi-personal.json"
chk "4 transcript rows" bash -c "[ \$(grep -cE '^[a-z_]+=(yes|no)\$' '$R/s5-transcripts.txt') -eq 4 ]"
chk "2 cross-resume exit codes" bash -c "[ \$(grep -cE 'exit_code=[0-9]+' '$R/s5-cross-resume.txt') -eq 2 ]"
chk "orca credentials unchanged" grep -qx 'credentials_unchanged=yes' "$R/s5-orca-untouched.txt"
chk "orca untouched file has unchanged=" grep -qE '^unchanged=(yes|no)$' "$R/s5-orca-untouched.txt"
exit $fail
