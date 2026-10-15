#!/usr/bin/env bash
# S5 - profile separation (SPK-05, SPK-10, D-05, D-06). Re-runnable.
#   bash spikes/s5/run.sh clone   clone ~/.claude-orca -> ~/.claude-personal WITHOUT .credentials.json
#   (user step)                   CLAUDE_CONFIG_DIR=~/.claude-personal claude   then /login with the PERSONAL account
#   bash spikes/s5/run.sh test    auth status, "hi", transcript locations, cross-resume, orca fingerprint
# Never runs claude against ~/.claude-orca. Never sets CLAUDE_CONFIG_DIR to ~/.claude (E-07).
# The credentials file is only ever fingerprinted (sha256/size/mtime); it is never opened for output.
# Numbers and yes/no only; no verdict.
set -u
. "$(dirname "${BASH_SOURCE[0]}")/../lib/env.sh"

RAW="$REPO/spikes/s5/raw"
WORK="$SPIKE_WORK_ROOT/s5"
ORCA="$HOME/.claude-orca"
PERS="$HOME/.claude-personal"
mkdir -p "$RAW" "$WORK"

# fingerprint FILE -> "sha256=... size=... mtime=..." (never prints contents)
fp() {
  if [ -e "$1" ]; then
    echo "sha256=$(sha256sum "$1" | cut -d' ' -f1) size=$(stat -c %s "$1") mtime=$(stat -c %Y "$1")"
  else
    echo "absent"
  fi
}

orca_fingerprint() { # prints two lines: credentials / claude_json
  echo "credentials $(fp "$ORCA/.credentials.json")"
  echo "claude_json $(fp "$ORCA/.claude.json")"
}

mode_clone() {
  orca_fingerprint > "$WORK/orca-before.txt"
  if [ -e "$PERS" ] && [ -n "$(ls -A "$PERS" 2>/dev/null)" ]; then
    echo "S5: $PERS already exists and is not empty. Refusing to overwrite; ask the user." >&2
    return 3
  fi
  mkdir -p "$PERS"
  local EXC="projects/ sessions/ history.jsonl file-history/ session-env/ shell-snapshots/ telemetry/ backups/ .credentials.json"
  if command -v rsync >/dev/null 2>&1; then
    local -a args=(-a)
    local e; for e in $EXC; do args+=(--exclude "$e"); done
    rsync "${args[@]}" "$ORCA/" "$PERS/"
  else
    local -a ex=()
    local e; for e in $EXC; do ex+=(--exclude "./${e%/}"); done
    ( cd "$ORCA" && tar -c "${ex[@]}" . ) | ( cd "$PERS" && tar -x )
  fi
  chmod 700 "$PERS"
  {
    if [ -e "$PERS/.credentials.json" ]; then echo "credentials_copied=yes"; else echo "credentials_copied=no"; fi
    echo "file_count=$(find "$PERS" -type f | wc -l)"
    echo "mode=$(stat -c %a "$PERS")"
    echo "excludes=$EXC"
    echo "method=$(command -v rsync >/dev/null 2>&1 && echo rsync || echo tar)"
  } > "$RAW/s5-clone.txt"
  scrub_raw "$RAW/s5-clone.txt" >/dev/null
  echo "S5 clone done:"; cat "$RAW/s5-clone.txt"
}

# auth status through an allowlist filter; the unfiltered output is never written to disk (T-1-02)
auth_filtered() { # NAME; uses SPIKE_PROFILE_DIR if set
  run_claude auth status 2>/dev/null | python3 -I -c '
import json, sys
raw = sys.stdin.read()
try:
    d = json.loads(raw)
except Exception:
    print(json.dumps({"parse_error": True}))
    sys.exit(0)
keep = ["authMethod", "configDirectory", "subscriptionType", "organizationType", "seatTier", "organizationRole", "loggedIn"]
print(json.dumps({k: d[k] for k in keep if k in d}, ensure_ascii=False, indent=1))
'
}

enc_of() { ( cd "$1" && pwd -P | sed 's/[^A-Za-z0-9]/-/g' ); }

mode_test() {
  [ -e "$WORK/orca-before.txt" ] || { echo "S5: run 'clone' first (no orca-before fingerprint)" >&2; return 3; }
  local ENC; ENC="$(enc_of "$WORK")"

  # (a) auth status, work = default profile (variable unset), personal = clone
  ( unset SPIKE_PROFILE_DIR; auth_filtered ) > "$RAW/s5-auth-work.json"
  ( SPIKE_PROFILE_DIR="$PERS"; export SPIKE_PROFILE_DIR; auth_filtered ) > "$RAW/s5-auth-personal.json"

  # (b) the same tiny prompt under each profile, from the same scratch dir
  ( cd "$WORK" && unset SPIKE_PROFILE_DIR && run_claude -p "hi" --output-format json ) > "$RAW/s5-hi-work.json" 2> "$WORK/e-work.txt"
  ( cd "$WORK" && SPIKE_PROFILE_DIR="$PERS" run_claude -p "hi" --output-format json ) > "$RAW/s5-hi-personal.json" 2> "$WORK/e-personal.txt"
  local SW SP
  SW="$(jq -r '.session_id // empty' "$RAW/s5-hi-work.json" 2>/dev/null | head -n1)"
  SP="$(jq -r '.session_id // empty' "$RAW/s5-hi-personal.json" 2>/dev/null | head -n1)"

  # (c) where does each transcript live? (4 yes/no rows)
  yn() { [ -n "$2" ] && [ -f "$1/projects/$ENC/$2.jsonl" ] && echo yes || echo no; }
  {
    echo "work_sid_in_work_profile=$(yn "$HOME/.claude" "$SW")"
    echo "work_sid_in_personal_profile=$(yn "$PERS" "$SW")"
    echo "personal_sid_in_personal_profile=$(yn "$PERS" "$SP")"
    echo "personal_sid_in_work_profile=$(yn "$HOME/.claude" "$SP")"
  } > "$RAW/s5-transcripts.txt"

  # (d) cross-resume both ways; stderr + exit code recorded
  : > "$RAW/s5-cross-resume.txt"
  local out ec
  out="$( cd "$WORK" && SPIKE_PROFILE_DIR="$PERS" run_claude -p "x" --resume "${SW:-none}" --output-format json 2>&1 )"; ec=$?
  { echo "work_sid_resumed_under_personal exit_code=$ec"; printf '%s\n' "$out" | head -c 600; echo; } >> "$RAW/s5-cross-resume.txt"
  out="$( cd "$WORK" && unset SPIKE_PROFILE_DIR && run_claude -p "x" --resume "${SP:-none}" --output-format json 2>&1 )"; ec=$?
  { echo "personal_sid_resumed_under_work exit_code=$ec"; printf '%s\n' "$out" | head -c 600; echo; } >> "$RAW/s5-cross-resume.txt"

  # (e) did anything touch ~/.claude-orca? credentials and .claude.json compared separately.
  # NOTE: the agent session itself runs with CLAUDE_CONFIG_DIR=~/.claude-orca, so .claude.json can change
  # because of the live session, not because of this spike.
  orca_fingerprint > "$WORK/orca-after.txt"
  {
    if [ "$(grep '^credentials ' "$WORK/orca-before.txt")" = "$(grep '^credentials ' "$WORK/orca-after.txt")" ]; then echo "credentials_unchanged=yes"; else echo "credentials_unchanged=no"; fi
    if [ "$(grep '^claude_json ' "$WORK/orca-before.txt")" = "$(grep '^claude_json ' "$WORK/orca-after.txt")" ]; then echo "claude_json_unchanged=yes"; else echo "claude_json_unchanged=no"; fi
    if [ "$(cat "$WORK/orca-before.txt")" = "$(cat "$WORK/orca-after.txt")" ]; then echo "unchanged=yes"; else echo "unchanged=no"; fi
  } > "$RAW/s5-orca-untouched.txt"

  scrub_raw "$RAW" >/dev/null
  echo "S5 test done."
}

case "${1:-}" in
  clone) mode_clone ;;
  test)  mode_test ;;
  *) echo "usage: bash spikes/s5/run.sh clone|test" >&2; exit 2 ;;
esac
