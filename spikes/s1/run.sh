#!/usr/bin/env bash
# S1 - stream-json (SPK-01). Re-runnable. Writes only into spikes/s1/raw/. Prints numbers only, never a verdict.
set -u
. "$(dirname "$0")/../lib/env.sh"

RAW="$REPO/spikes/s1/raw"
WORK="$SPIKE_WORK_ROOT/s1"
mkdir -p "$RAW"

# fresh scratch dir outside the repo with three small generated files
case "$WORK" in "$SPIKE_WORK_ROOT"/s1) rm -rf -- "$WORK" ;; esac
mkdir -p "$WORK"
printf 'alpha\n' > "$WORK/a.txt"
printf 'beta\nbeta\n' > "$WORK/b.txt"
printf '# notes\n' > "$WORK/c.md"

PROMPT="liệt kê file trong thư mục này"

# version first, so every raw file can be tied to it
run_claude --version > "$RAW/s1-version.txt" 2>&1

# positive case (source S1 command + --allowedTools so the tool call needs no prompt)
START=$(epoch)
( cd "$WORK" && run_claude -p "$PROMPT" --output-format stream-json --verbose --allowedTools "Bash(ls *)" ) \
  > "$RAW/s1.jsonl" 2> "$WORK/stderr-positive.txt"
EXIT_POS=$?
END=$(epoch)

# negative case (E-01): the same command without --verbose
{
  echo "--- command: claude -p <prompt> --output-format stream-json --allowedTools 'Bash(ls *)'  (no --verbose)"
  ( cd "$WORK" && run_claude -p "$PROMPT" --output-format stream-json --allowedTools "Bash(ls *)" ) \
    > "$WORK/nv-stdout.txt" 2> "$WORK/nv-stderr.txt"
  echo "$?" > "$WORK/nv-exit.txt"
  echo "--- stdout ($(wc -l < "$WORK/nv-stdout.txt") lines, first 5 shown) ---"
  head -n 5 "$WORK/nv-stdout.txt"
  echo "--- stderr ---"
  cat "$WORK/nv-stderr.txt"
  echo "exit_code=$(cat "$WORK/nv-exit.txt")"
} > "$RAW/s1-no-verbose.txt" 2>&1

scrub_raw "$RAW"

# observations (numbers only)
J="$RAW/s1.jsonl"
{
  echo "OBSERVATIONS S1 ($(ts))"
  echo "positive_exit_code=$EXIT_POS duration_s=$((END-START)) stderr_lines=$(wc -l < "$WORK/stderr-positive.txt") stdout_lines=$(wc -l < "$J")"
  echo "claude_version=$(head -n1 "$RAW/s1-version.txt")"
  echo "--- init (session_id, model, version)"
  jq -c 'select(.type=="system" and .subtype=="init") | {session_id,model,claude_code_version}' "$J"
  echo "--- distinct type/subtype pairs (count type subtype)"
  jq -r '[.type, (.subtype // "-")] | @tsv' "$J" | sort | uniq -c
  echo "--- assistant lines (message.id, content block types)"
  jq -c 'select(.type=="assistant") | {id:.message.id, blocks:[.message.content[]?.type]}' "$J"
  echo "--- user lines (content block types)"
  jq -c 'select(.type=="user") | {blocks:[(.message.content | arrays | .[]?.type)]}' "$J"
  echo "--- counts"
  echo "tool_use_blocks=$(jq -s '[.[] | select(.type=="assistant") | .message.content[]? | select(.type=="tool_use")] | length' "$J")"
  echo "tool_result_blocks=$(jq -s '[.[] | select(.type=="user") | (.message.content | arrays | .[]?) | select(.type=="tool_result")] | length' "$J")"
  echo "assistant_lines=$(jq -s '[.[] | select(.type=="assistant")] | length' "$J")"
  echo "distinct_message_ids=$(jq -s '[.[] | select(.type=="assistant") | .message.id] | unique | length' "$J")"
  echo "--- result"
  jq -c 'select(.type=="result") | {subtype,is_error,num_turns,terminal_reason,session_id,total_cost_usd}' "$J"
  jq -c 'select(.type=="result") | {input:.usage.input_tokens,output:.usage.output_tokens,cache_read:.usage.cache_read_input_tokens,cache_creation:.usage.cache_creation_input_tokens,ephemeral_5m:.usage.cache_creation.ephemeral_5m_input_tokens,ephemeral_1h:.usage.cache_creation.ephemeral_1h_input_tokens}' "$J"
  echo "--- no-verbose case"
  grep -E '^exit_code=' "$RAW/s1-no-verbose.txt"
} > "$RAW/s1-observations.txt" 2>&1

scrub_raw "$RAW/s1-observations.txt" >/dev/null
cat "$RAW/s1-observations.txt"
