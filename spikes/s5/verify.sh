#!/usr/bin/env bash
# S5 evidence check (completeness only; never judges the outcome).
#   --clone    only the clone step
#   --partial  evidence from `run.sh partial` (map-only decision 2026-10-15: no personal login, personal = ~/.claude-orca)
set -u
DIR="$(cd "$(dirname "$0")" && pwd -P)"
R="$DIR/raw"
fail=0
chk() { local d="$1"; shift; if "$@" >/dev/null 2>&1; then echo "ok   $d"; else echo "FAIL $d"; fail=1; fi; }
chk "credentials_copied=no" grep -qx 'credentials_copied=no' "$R/s5-clone.txt"
chk "personal dir mode is 700" bash -c "[ \"\$(stat -c %a \"\$HOME/.claude-personal\")\" = 700 ]"
[ "${1:-}" = "--clone" ] && exit $fail
if [ "${1:-}" = "--partial" ]; then
  chk "work auth has configDirectory" jq -e 'has("configDirectory")' "$R/s5-auth-work.json"
  chk "clone auth has configDirectory ending .claude-personal" bash -c "jq -r .configDirectory '$R/s5-auth-personal.json' | grep -q '\\.claude-personal\$'"
  chk "clone auth records loggedIn" jq -e 'has("loggedIn")' "$R/s5-auth-personal.json"
  chk "account facts for work and personal" bash -c "[ \$(grep -cE '^(work|personal_orca) \{' '$R/s5-account-facts.txt') -eq 2 ]"
  chk "work hi has session_id" jq -e '(.session_id|type)=="string"' "$R/s5-hi-work.json"
  chk "clone hi has exit_code" grep -qE '^exit_code=[0-9]+$' "$R/s5-hi-clone.txt"
  chk "4 transcript rows (yes/no/not_run)" bash -c "[ \$(grep -cE '^[a-z_]+=(yes|no|not_run)\$' '$R/s5-transcripts.txt') -eq 4 ]"
  chk "2 cross-resume rows (not_run)" bash -c "[ \$(grep -c 'not_run' '$R/s5-cross-resume.txt') -eq 2 ]"
  chk "orca credentials unchanged" grep -qx 'credentials_unchanged=yes' "$R/s5-orca-untouched.txt"
  chk "orca claude_json reported separately" grep -qE '^claude_json_unchanged=(yes|no)$' "$R/s5-orca-untouched.txt"
  chk "no claude run against orca" grep -qx 'claude_run_against_orca=no' "$R/s5-orca-untouched.txt"
  exit $fail
fi
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
