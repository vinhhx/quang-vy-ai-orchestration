#!/usr/bin/env bash
# S8 - --max-turns (SPK-08). Re-runnable: bash spikes/s8/run.sh
# Scratch git repo OUTSIDE the project repo (the diff text becomes a committed fixture, so it stays non-sensitive).
#   Variant A: the source command verbatim:  claude -p "review git diff" --max-turns 1 --output-format json
#   Variant B: the FAST-02 form: diff text inside the prompt argument, no tools, no session persistence.
# Numbers and yes/no only; no verdict. Every claude call goes through run_claude (clean env, </dev/null, timeout).
set -u
. "$(dirname "${BASH_SOURCE[0]}")/../lib/env.sh"

RAW="$REPO/spikes/s8/raw"
S8="$SPIKE_WORK_ROOT/s8"
R="$S8/repo"
mkdir -p "$RAW"
rm -rf "$S8"; mkdir -p "$R"
cd "$R" || exit 3

G=(git -c user.name=spike -c user.email=spike@example.invalid)
"${G[@]}" init -q . && "${G[@]}" checkout -q -b main 2>/dev/null
cat > calc.py <<'PY'
def add(a, b):
    return a + b
PY
"${G[@]}" add calc.py && "${G[@]}" commit -q -m "add calc"
cat > calc.py <<'PY'
def add(a, b):
    return a + b


def divide(a, b):
    return a / b
PY
git diff > "$RAW/s8-diff.patch"

enc() { pwd -P | sed 's/[^A-Za-z0-9]/-/g'; }
PROJ="$HOME/.claude/projects/$(enc)"
count_jsonl() { ls "$PROJ"/*.jsonl 2>/dev/null | wc -l; }

: > "$RAW/s8-exit.txt"

# Variant A
run_claude -p "review git diff" --max-turns 1 --output-format json > "$RAW/s8-a.json" 2> "$RAW/s8-a.stderr.txt"
echo "A exit_code=$?" >> "$RAW/s8-exit.txt"

# Variant B
DIFF="$(cat "$RAW/s8-diff.patch")"
BEFORE="$(count_jsonl)"
run_claude -p "Review this diff in two sentences:
$DIFF" --max-turns 1 --tools "" --no-session-persistence --output-format json > "$RAW/s8-b.json" 2> "$RAW/s8-b.stderr.txt"
echo "B exit_code=$?" >> "$RAW/s8-exit.txt"
AFTER="$(count_jsonl)"
echo "before=$BEFORE after=$AFTER proj_dir_exists=$([ -d "$PROJ" ] && echo yes || echo no)" > "$RAW/s8-transcripts.txt"

scrub_raw "$RAW" >/dev/null
echo "== exit"; cat "$RAW/s8-exit.txt"; echo "== transcripts"; cat "$RAW/s8-transcripts.txt"
for v in a b; do
  echo "== $v"
  jq -c '{subtype,is_error,num_turns,terminal_reason,result_nonempty:((.result // "")|length>0),stop_reason}' "$RAW/s8-$v.json" 2>&1
done
