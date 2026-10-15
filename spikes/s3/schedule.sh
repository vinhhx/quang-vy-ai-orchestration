#!/usr/bin/env bash
# S3 detached probe schedule. Start only via run.sh:
#   setsid nohup bash spikes/s3/schedule.sh > spikes/s3/raw/schedule.log 2>&1 < /dev/null &
# For K in 1, 10, 65: sleep until t0_end(K) + K*60 on the REAL clock, run one identical probe in wK, log it.
# Never runs any other claude in these workdirs. Signals nothing.
set -u
. "$(dirname "$0")/../lib/env.sh"

RAW="$REPO/spikes/s3/raw"
WROOT="$SPIKE_WORK_ROOT/s3"
TL="$RAW/s3-timeline.tsv"
PRE="$RAW/s3-precheck.txt"
PROMPT="Reply with the single word OK"

echo $$ > "$RAW/s3-schedule.pid"
echo "START $(epoch) $(ts) pid=$$ pgid=$(ps -o pgid= -p $$ | tr -d ' ')"

# snapshot the scrubber so a branch switch or a missing file cannot break the final scrub
mkdir -p "$WROOT/lib"
cp "$REPO/spikes/lib/scrub.py" "$WROOT/lib/scrub.py"

probe() { # K
  local K="$1" start end rc
  start="$(epoch)"
  ( cd "$WROOT/w$K" && run_claude -p "$PROMPT" --output-format json ) > "$RAW/s3-w$K-probe-$start.json" 2> "$WROOT/w$K.stderr"
  rc=$?
  end="$(epoch)"
  printf '%s\t%s\t%s\t%s\t%s\n' "$K" probe "$start" "$end" "$rc" >> "$TL"
  python3 -I "$WROOT/lib/scrub.py" "$RAW/s3-w$K-probe-$start.json" > /dev/null
  echo "PROBE K=$K start=$start end=$end exit=$rc cache_read=$(jq -r '.usage.cache_read_input_tokens // "n/a"' "$RAW/s3-w$K-probe-$start.json" 2>/dev/null) cache_creation=$(jq -r '.usage.cache_creation_input_tokens // "n/a"' "$RAW/s3-w$K-probe-$start.json" 2>/dev/null)"
}

for K in 1 10 65; do
  t0_end="$(awk -F'\t' -v k="$K" '$1==k && $2=="t0" {print $4}' "$TL" | tail -n1)"
  if [ -z "$t0_end" ]; then echo "ERROR no t0 row for K=$K"; continue; fi
  target=$((t0_end + K * 60))
  while :; do
    now="$(epoch)"
    [ "$now" -ge "$target" ] && break
    sleep $(( target - now > 30 ? 30 : target - now ))
  done
  echo "DUE K=$K target=$target now=$(epoch)"
  probe "$K"
done

{
  echo "util_after: $(jq -r '.cachedUsageUtilization // empty | "util five_hour=\(.utilization.five_hour // "n/a") seven_day=\(.utilization.seven_day // "n/a") fetched_age_s=\(((now*1000 - (.fetchedAtMs // 0))/1000) | floor)"' "$HOME/.claude.json" 2>/dev/null || echo 'util n/a')"
} >> "$PRE"
echo "DONE $(epoch)"
