#!/usr/bin/env bash
# S1 evidence check (completeness only; never judges the spike outcome, except the result/usage smoke check).
set -u
DIR="$(cd "$(dirname "$0")" && pwd -P)"
J="$DIR/raw/s1.jsonl"
fail=0
chk() { # description, command...
  local d="$1"; shift
  if "$@" >/dev/null 2>&1; then echo "ok   $d"; else echo "FAIL $d"; fail=1; fi
}
[ -s "$J" ] || { echo "FAIL s1.jsonl missing or empty"; exit 1; }
chk "init line carries session_id" jq -s -e 'any(.[]; .type=="system" and .subtype=="init" and ((.session_id|type)=="string") and (.session_id|length)>0)' "$J"
chk "result line has usage and session_id" jq -s -e 'any(.[]; .type=="result" and (.usage|type)=="object" and ((.session_id|type)=="string"))' "$J"
chk "an assistant line holds a tool_use block" jq -s -e 'any(.[]; .type=="assistant" and any(.message.content[]?; .type=="tool_use"))' "$J"
chk "a user line holds a tool_result block" jq -s -e 'any(.[]; .type=="user" and any((.message.content|arrays|.[]?); .type=="tool_result"))' "$J"
chk "s1-no-verbose.txt records exit_code=" grep -q '^exit_code=' "$DIR/raw/s1-no-verbose.txt"
chk "s1-version.txt present" test -s "$DIR/raw/s1-version.txt"
chk "s1-observations.txt present" test -s "$DIR/raw/s1-observations.txt"
exit $fail
