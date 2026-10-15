#!/usr/bin/env bash
# S2 - resume and fork (SPK-02). Re-runnable. Writes only into spikes/s2/raw/. Numbers only, no verdict.
set -u
. "$(dirname "$0")/../lib/env.sh"

RAW="$REPO/spikes/s2/raw"
WORK="$SPIKE_WORK_ROOT/s2"
mkdir -p "$RAW"
case "$WORK" in "$SPIKE_WORK_ROOT"/s2) rm -rf -- "$WORK" ;; esac
mkdir -p "$WORK"

# transcript path: ~/.claude/projects/<pwd -P with non-alphanumerics replaced by '-'>/<id>.jsonl
ENC="$(cd "$WORK" && pwd -P | sed 's/[^A-Za-z0-9]/-/g')"
tpath() { printf '%s/.claude/projects/%s/%s.jsonl' "$HOME" "$ENC" "$1"; }
snap() { # label id -> "label sha256=... lines=N" (no path is written)
  local f; f="$(tpath "$2")"
  if [ -f "$f" ]; then echo "$1 sha256=$(sha256sum "$f" | cut -d' ' -f1) lines=$(wc -l < "$f")"; else echo "$1 sha256=absent lines=0"; fi
}

# step 1: create
( cd "$WORK" && run_claude -p "nhớ số 42" --output-format json ) > "$RAW/s2-1-create.json" 2> "$WORK/e1.txt"
ID="$(jq -r '.session_id // empty' "$RAW/s2-1-create.json" 2>/dev/null | head -n1)"
[ -n "$ID" ] || { echo "S2: no session_id from step 1 (see $RAW/s2-1-create.json)"; ID="none"; }

# step 2: resume (the same id is expected to be reused)
( cd "$WORK" && run_claude -p "số tôi bảo nhớ là gì?" --resume "$ID" --output-format json ) > "$RAW/s2-2-resume.json" 2> "$WORK/e2.txt"

# hash the original transcript before the fork
snap before "$ID" > "$RAW/s2-transcript-hash.txt"

# step 3: fork
( cd "$WORK" && run_claude -p "số đó cộng 1?" --resume "$ID" --fork-session --output-format json ) > "$RAW/s2-3-fork.json" 2> "$WORK/e3.txt"

# hash again after the fork
snap after "$ID" >> "$RAW/s2-transcript-hash.txt"

scrub_raw "$RAW"

# observations
j() { jq -r "$1" "$2" 2>/dev/null | head -n1; }
ID2="$(j '.session_id // empty' "$RAW/s2-2-resume.json")"
ID3="$(j '.session_id // empty' "$RAW/s2-3-fork.json")"
B="$(grep '^before ' "$RAW/s2-transcript-hash.txt")"; A="$(grep '^after ' "$RAW/s2-transcript-hash.txt")"
{
  echo "OBSERVATIONS S2 ($(ts))"
  echo "claude_version=$(run_claude --version 2>&1 | head -n1)"
  echo "create_id=$ID"
  echo "resume_id=$ID2"
  echo "fork_id=$ID3"
  [ -n "$ID2" ] && [ "$ID2" = "$ID" ] && echo "resume_same_id=yes" || echo "resume_same_id=no"
  [ -n "$ID3" ] && [ "$ID3" != "$ID" ] && echo "fork_new_id=yes" || echo "fork_new_id=no"
  [ -n "$B" ] && [ "${B#before }" = "${A#after }" ] && echo "transcript_unchanged=yes" || echo "transcript_unchanged=no"
  echo "transcript_before=${B#before }"
  echo "transcript_after=${A#after }"
  grep -q '43' <<<"$(j '.result // ""' "$RAW/s2-3-fork.json")" && echo "fork_answer_has_43=yes" || echo "fork_answer_has_43=no"
  grep -q '42' <<<"$(j '.result // ""' "$RAW/s2-2-resume.json")" && echo "resume_answer_has_42=yes" || echo "resume_answer_has_42=no"
  echo "cache_read_resume=$(j '.usage.cache_read_input_tokens // "n/a"' "$RAW/s2-2-resume.json")"
  echo "cache_creation_resume=$(j '.usage.cache_creation_input_tokens // "n/a"' "$RAW/s2-2-resume.json")"
  echo "cache_read_fork=$(j '.usage.cache_read_input_tokens // "n/a"' "$RAW/s2-3-fork.json")"
  echo "cache_creation_fork=$(j '.usage.cache_creation_input_tokens // "n/a"' "$RAW/s2-3-fork.json")"
  echo "cache_read_create=$(j '.usage.cache_read_input_tokens // "n/a"' "$RAW/s2-1-create.json")"
  echo "cache_creation_create=$(j '.usage.cache_creation_input_tokens // "n/a"' "$RAW/s2-1-create.json")"
  echo "num_turns_create=$(j '.num_turns // "n/a"' "$RAW/s2-1-create.json") resume=$(j '.num_turns // "n/a"' "$RAW/s2-2-resume.json") fork=$(j '.num_turns // "n/a"' "$RAW/s2-3-fork.json")"
  echo "fork_transcript_exists=$([ -n "$ID3" ] && [ -f "$(tpath "$ID3")" ] && echo yes || echo no)"
} > "$RAW/s2-observations.txt"
scrub_raw "$RAW/s2-observations.txt" > /dev/null
cat "$RAW/s2-observations.txt"
