#!/usr/bin/env bash
# S3 - prompt cache, mark 0 (cold baselines) + launch of the detached 1/10/65-minute probes (SPK-03, D-09, D-10).
# Re-runnable; a rerun restarts the whole measurement (the timers cannot be resumed).
# Signal = project-specific prefix tokens in cache_read_input_tokens / cache_creation_input_tokens, NOT the total.
# Three independent prefixes (w1, w10, w65) because every cache hit resets the TTL timer.
set -u
. "$(dirname "$0")/../lib/env.sh"

RAW="$REPO/spikes/s3/raw"
WROOT="$SPIKE_WORK_ROOT/s3"
PRE="$RAW/s3-precheck.txt"
TL="$RAW/s3-timeline.tsv"
PROMPT="Reply with the single word OK"
PREFIX_BYTES="${S3_PREFIX_BYTES:-49152}"
mkdir -p "$RAW"

# refuse to start a second measurement while a schedule is alive
if [ -f "$RAW/s3-schedule.pid" ] && kill -0 "$(cat "$RAW/s3-schedule.pid" 2>/dev/null)" 2>/dev/null; then
  echo "S3: a schedule is still running (pid $(cat "$RAW/s3-schedule.pid")); not starting another" >&2
  exit 3
fi

util() { # numbers only, from the cached usage block; fetched_age_s shows how stale it is
  jq -r '.cachedUsageUtilization // empty | "util five_hour=\(.utilization.five_hour // "n/a") seven_day=\(.utilization.seven_day // "n/a") fetched_age_s=\(((now*1000 - (.fetchedAtMs // 0))/1000) | floor)"' "$HOME/.claude.json" 2>/dev/null || echo "util n/a"
}
isset() { if [ -n "${!1+x}" ]; then echo yes; else echo no; fi; }

# precheck (names only)
{
  echo "S3 precheck $(ts)"
  for n in FORCE_PROMPT_CACHING_5M CLAUDE_CODE_PROMPT_CACHE_TTL ENABLE_PROMPT_CACHING_1H DISABLE_PROMPT_CACHING; do
    echo "$n set=$(isset "$n")"
  done
  echo "promptCacheTtl in ~/.claude/settings.json: $(grep -q promptCacheTtl "$HOME/.claude/settings.json" 2>/dev/null && echo yes || echo no)"
  echo "prefix_bytes_requested=$PREFIX_BYTES"
  echo "util_before: $(util)"
} > "$PRE"

# fresh, non-git workdirs outside the repo, each with a unique-nonce CLAUDE.md (the project-specific prefix)
case "$WROOT" in "$SPIKE_WORK_ROOT"/s3) rm -rf -- "$WROOT" ;; esac
mkdir -p "$WROOT"
for K in 1 10 65; do
  mkdir -p "$WROOT/w$K"
  nonce="$(cat /proc/sys/kernel/random/uuid)"
  python3 -I - "$nonce" "$PREFIX_BYTES" > "$WROOT/w$K/CLAUDE.md" <<'PY'
import random, sys
nonce, size = sys.argv[1], int(sys.argv[2])
rnd = random.Random(nonce)
syl = [a + b for a in "bcdfghjklmnprstvz" for b in "aeiou"]
out = ["nonce: " + nonce, ""]
n = len(out[0]) + 1
while n < size:
    line = " ".join("".join(rnd.choice(syl) for _ in range(rnd.randint(2, 4))) for _ in range(11))
    out.append(line)
    n += len(line) + 1
print("\n".join(out))
PY
  echo "w$K nonce=$nonce bytes=$(wc -c < "$WROOT/w$K/CLAUDE.md")" >> "$PRE"
done

probe() { # K mark
  local K="$1" mark="$2" start end rc
  start="$(epoch)"
  ( cd "$WROOT/w$K" && run_claude -p "$PROMPT" --output-format json ) > "$RAW/s3-w$K-$mark-$start.json" 2> "$WROOT/w$K.stderr"
  rc=$?
  end="$(epoch)"
  printf '%s\t%s\t%s\t%s\t%s\n' "$K" "$mark" "$start" "$end" "$rc" >> "$TL"
}

# mark 0 (cold): identical probe in each workdir
printf 'K\tmark\tstart_epoch\tend_epoch\texit_code\n' > "$TL"
rm -f "$RAW"/s3-w*-t0-*.json "$RAW"/s3-w*-probe-*.json "$RAW/s3-classification.tsv" "$RAW/schedule.log" "$RAW/s3-schedule.pid"
for K in 1 10 65; do probe "$K" t0; done
scrub_raw "$RAW"/s3-w*-t0-*.json > /dev/null

# size gate (E-04)
small=0
for f in "$RAW"/s3-w*-t0-*.json; do
  c="$(jq -r '.usage.cache_creation_input_tokens // 0' "$f" 2>/dev/null)"
  [ "${c:-0}" -ge 8000 ] 2>/dev/null || small=1
done
if [ "$small" -ne 0 ]; then
  echo "WARN prefix-too-small" >> "$PRE"
  echo "S3: a t0 cache_creation is below 8000 tokens; schedule NOT started. Retry once with S3_PREFIX_BYTES=$((PREFIX_BYTES*2))." >&2
  exit 4
fi

# launch the probes detached so no other task is blocked; the schedule writes its own PID file
setsid nohup bash "$REPO/spikes/s3/schedule.sh" > "$RAW/schedule.log" 2>&1 < /dev/null &
for _ in 1 2 3 4 5 6 7 8 9 10; do grep -q '^START' "$RAW/schedule.log" 2>/dev/null && break; sleep 1; done
echo "S3 t0 done at $(ts); schedule pid=$(cat "$RAW/s3-schedule.pid" 2>/dev/null || echo '?')"
for f in "$RAW"/s3-w*-t0-*.json; do
  echo "$(basename "$f") cache_read=$(jq -r '.usage.cache_read_input_tokens' "$f") cache_creation=$(jq -r '.usage.cache_creation_input_tokens' "$f") e5m=$(jq -r '.usage.cache_creation.ephemeral_5m_input_tokens' "$f") e1h=$(jq -r '.usage.cache_creation.ephemeral_1h_input_tokens' "$f") num_turns=$(jq -r '.num_turns' "$f")"
done
