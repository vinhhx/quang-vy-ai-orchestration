#!/usr/bin/env bash
# S7 - Antigravity headless via the agy CLI (SPK-07, D-11). Re-runnable: bash spikes/s7/run.sh
# Every agy call is bounded by `timeout 10`, has stdin from /dev/null and stdout redirected to a file
# (agy has a history of pipe-output bugs). Exit 124 means the 10 s bound was hit: an observation, not a failure.
# An "authentication required" reply is recorded as a finding and never worked around.
# Only the agy CLI is invoked here; no other launcher.
# Numbers and yes/no only; no verdict. Output is scrubbed at the end.
set -u
. "$(dirname "${BASH_SOURCE[0]}")/../lib/env.sh"

RAW="$REPO/spikes/s7/raw"
WD="$SPIKE_WORK_ROOT/s7"
mkdir -p "$RAW" "$WD"
cd "$WD" || exit 3

if ! command -v agy >/dev/null 2>&1; then
  echo "S7: agy not found; docs-only result (D-11: nothing is installed)" >&2
  echo "agy_present=no" > "$RAW/s7-version.txt"; echo "exit_code=127" >> "$RAW/s7-version.txt"
  exit 0
fi

# agy_to OUTFILE ARGS... : one bounded call, stdout to OUTFILE, stderr to OUTFILE-with-.stderr.txt, exit code appended.
agy_to() {
  local out="$1"; shift
  timeout 10 agy "$@" </dev/null > "$out" 2> "${out%.*}.stderr.txt"
  local ec=$?
  echo "exit_code=$ec" >> "$out"
  echo "exit_code=$ec" >> "${out%.*}.stderr.txt"
}

# agy_all OUTFILE ARGS... : same, but stderr merged into OUTFILE (agy prints help and usage errors on stderr).
agy_all() {
  local out="$1"; shift
  timeout 10 agy "$@" </dev/null > "$out" 2>&1
  echo "exit_code=$?" >> "$out"
}

agy_all "$RAW/s7-version.txt" --version
agy_all "$RAW/s7-help.txt" --help
agy_to "$RAW/s7-json.json" -p "Reply with the single word OK" --output-format json
agy_to "$RAW/s7-stream.jsonl" -p "Reply with the single word OK" --output-format stream-json
# falsification test for E-10: does agy accept a turn-limit flag?
agy_all "$RAW/s7-maxturns.txt" -p x --max-turns 1

scrub_raw "$RAW" >/dev/null
for f in version help json stream maxturns; do
  for g in "$RAW"/s7-$f.*; do [ -f "$g" ] && echo "$(basename "$g"): $(grep -E '^exit_code=' "$g" | tail -n1)"; done
done
