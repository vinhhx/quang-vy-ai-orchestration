#!/usr/bin/env bash
# S5 - profile separation (SPK-05, SPK-10, D-05, D-06). Re-runnable.
#   bash spikes/s5/run.sh clone   clone ~/.claude-orca -> ~/.claude-personal WITHOUT .credentials.json
#   (user step)                   CLAUDE_CONFIG_DIR=~/.claude-personal claude   then /login with the PERSONAL account
#   bash spikes/s5/run.sh test    auth status, "hi", transcript locations, cross-resume, orca fingerprint
#   bash spikes/s5/run.sh partial same, minus everything that needs the personal login (map-only decision, 2026-10-15)
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

# Account facts for the SPK-10 table, read-only from each profile's .claude.json (oauthAccount),
# through an allowlist: organizationType / seatTier / organizationRole only. No email, UUID or org name.
account_facts() { # PATH-to-.claude.json
  python3 -I - "$1" <<'PY'
import json, sys
try:
    d = json.load(open(sys.argv[1]))
    oa = d.get("oauthAccount") or {}
    out = {k: oa.get(k) for k in ("organizationType", "seatTier", "organizationRole")}
except Exception as e:
    out = {"error": type(e).__name__}
print(json.dumps(out, ensure_ascii=False))
PY
}

# Partial mode (user decision 2026-10-15: "Map only, skip live test"): everything that needs no personal login.
#  - the personal profile is ~/.claude-orca, which this spike must never run claude against (E-06);
#  - ~/.claude-personal stays as an unused, credential-free clone, so it is only used here to show it is logged out;
#  - live personal run, transcript separation and cross-resume between profiles are NOT tested (recorded as not_run).
mode_partial() {
  [ -e "$WORK/orca-before.txt" ] || { echo "S5: run 'clone' first (no orca-before fingerprint)" >&2; return 3; }
  local ENC ECW; ENC="$(enc_of "$WORK")"

  # (a) auth status: work = default profile (variable unset); uncredentialed clone = expected logged out
  ( unset SPIKE_PROFILE_DIR; auth_filtered ) > "$RAW/s5-auth-work.json"
  ( SPIKE_PROFILE_DIR="$PERS"; export SPIKE_PROFILE_DIR; auth_filtered ) > "$RAW/s5-auth-personal.json"

  # (a2) account facts for SPK-10 (work ~/.claude.json, personal ~/.claude-orca/.claude.json), read only
  {
    echo "work $(account_facts "$HOME/.claude.json")"
    echo "personal_orca $(account_facts "$ORCA/.claude.json")"
  } > "$RAW/s5-account-facts.txt"

  # (b) ONE tiny prompt under the work profile only (work weekly utilization was 0.94)
  ( cd "$WORK" && unset SPIKE_PROFILE_DIR && run_claude -p "hi" --output-format json ) > "$RAW/s5-hi-work.json" 2> "$WORK/e-work.txt"
  ECW=$?
  local SW; SW="$(jq -r '.session_id // empty' "$RAW/s5-hi-work.json" 2>/dev/null | head -n1)"
  # (b2) the same prompt against the credential-free clone: expected to fail with a login prompt (finding, not a bug)
  local outc ecc
  outc="$( cd "$WORK" && SPIKE_PROFILE_DIR="$PERS" run_claude -p "hi" --output-format json 2>&1 )"; ecc=$?
  {
    echo "exit_code=$ecc"
    if printf '%s' "$outc" | jq -e . >/dev/null 2>&1; then
      printf '%s' "$outc" | jq -r '"subtype=\(.subtype // "?") is_error=\(.is_error // "?") num_turns=\(.num_turns // "?") total_cost_usd=\(.total_cost_usd // "?")", "result=\((.result // "") | .[0:160])"'
    else
      printf 'raw=%s\n' "$(printf '%s' "$outc" | head -c 300)"
    fi
  } > "$RAW/s5-hi-clone.txt"
  echo "work_hi_exit_code=$ECW" >> "$RAW/s5-hi-clone.txt"

  # (c) where does the work transcript live? rows involving the personal login are not_run
  yn() { [ -n "$2" ] && [ -f "$1/projects/$ENC/$2.jsonl" ] && echo yes || echo no; }
  {
    echo "work_sid_in_work_profile=$(yn "$HOME/.claude" "$SW")"
    echo "work_sid_in_clone_profile=$(yn "$PERS" "$SW")"
    echo "personal_sid_in_personal_profile=not_run"
    echo "personal_sid_in_work_profile=not_run"
  } > "$RAW/s5-transcripts.txt"

  # (d) cross-resume between profiles needs a personal session id: not run
  {
    echo "work_sid_resumed_under_personal not_run (no personal login; user chose map-only)"
    echo "personal_sid_resumed_under_work not_run (no personal login; user chose map-only)"
  } > "$RAW/s5-cross-resume.txt"

  # (e) orca fingerprint: credentials and .claude.json reported separately. The agent session itself runs on
  # ~/.claude-orca, so .claude.json may change from the live session; that is not caused by this spike.
  orca_fingerprint > "$WORK/orca-after.txt"
  {
    if [ "$(grep '^credentials ' "$WORK/orca-before.txt")" = "$(grep '^credentials ' "$WORK/orca-after.txt")" ]; then echo "credentials_unchanged=yes"; else echo "credentials_unchanged=no"; fi
    if [ "$(grep '^claude_json ' "$WORK/orca-before.txt")" = "$(grep '^claude_json ' "$WORK/orca-after.txt")" ]; then echo "claude_json_unchanged=yes"; else echo "claude_json_unchanged=no"; fi
    if [ "$(cat "$WORK/orca-before.txt")" = "$(cat "$WORK/orca-after.txt")" ]; then echo "unchanged=yes"; else echo "unchanged=no"; fi
    echo "claude_run_against_orca=no"
  } > "$RAW/s5-orca-untouched.txt"

  scrub_raw "$RAW" >/dev/null
  echo "S5 partial done."
}

case "${1:-}" in
  clone) mode_clone ;;
  test)  mode_test ;;
  partial) mode_partial ;;
  *) echo "usage: bash spikes/s5/run.sh clone|test|partial" >&2; exit 2 ;;
esac
