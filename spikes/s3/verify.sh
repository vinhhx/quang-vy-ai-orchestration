#!/usr/bin/env bash
# S3 evidence check. Never judges the outcome: warm/cold classification is an observation.
#   verify.sh --started   after run.sh: three t0 files, timeline, schedule alive
#   verify.sh             after the schedule finished: six files, DONE, writes raw/s3-classification.tsv
set -u
DIR="$(cd "$(dirname "$0")" && pwd -P)"
R="$DIR/raw"
fail=0
chk() { local d="$1"; shift; if "$@" >/dev/null 2>&1; then echo "ok   $d"; else echo "FAIL $d"; fail=1; fi; }
num_fields() { jq -e '(.usage.cache_read_input_tokens|type)=="number" and (.usage.cache_creation_input_tokens|type)=="number"' "$1"; }

chk "precheck has no prefix-too-small warning" bash -c "! grep -q 'WARN prefix-too-small' '$R/s3-precheck.txt'"
if [ "${1:-}" = "--started" ]; then
  for K in 1 10 65; do
    f="$(ls "$R"/s3-w$K-t0-*.json 2>/dev/null | head -n1)"
    chk "w$K t0 file with numeric cache fields" bash -c "[ -n '$f' ] && jq -e '(.usage.cache_read_input_tokens|type)==\"number\" and (.usage.cache_creation_input_tokens|type)==\"number\"' '$f'"
  done
  chk "timeline has 3 t0 rows" bash -c "[ \$(awk -F'\t' '\$2==\"t0\"' '$R/s3-timeline.tsv' | wc -l) -eq 3 ]"
  chk "schedule.log has START" grep -q '^START' "$R/schedule.log"
  chk "schedule pid is running (existence check, no signal)" bash -c "kill -0 \$(cat '$R/s3-schedule.pid')"
  exit $fail
fi

chk "schedule.log has DONE" grep -q '^DONE' "$R/schedule.log"
OUT="$R/s3-classification.tsv"
printf '# K\tgap_min\tt0_cache_read\tt0_cache_creation\tcache_read\tcache_creation\tephemeral_5m\tephemeral_1h\tnum_turns\tclassification\n' > "$OUT.tmp"
for K in 1 10 65; do
  t0="$(ls "$R"/s3-w$K-t0-*.json 2>/dev/null | head -n1)"
  pr="$(ls "$R"/s3-w$K-probe-*.json 2>/dev/null | head -n1)"
  chk "w$K t0 and probe files exist" bash -c "[ -n '$t0' ] && [ -n '$pr' ]"
  [ -n "$t0" ] && [ -n "$pr" ] || continue
  chk "w$K t0 numeric cache fields" num_fields "$t0"
  chk "w$K probe numeric cache fields" num_fields "$pr"
  t0_end="$(awk -F'\t' -v k="$K" '$1==k && $2=="t0" {print $4}' "$R/s3-timeline.tsv" | tail -n1)"
  pr_start="$(awk -F'\t' -v k="$K" '$1==k && $2=="probe" {print $3}' "$R/s3-timeline.tsv" | tail -n1)"
  gap="$(awk -v a="$t0_end" -v b="$pr_start" 'BEGIN{printf "%.1f", (b-a)/60}')"
  g() { jq -r "$1 // 0" "$2"; }
  t0r="$(g .usage.cache_read_input_tokens "$t0")"; t0c="$(g .usage.cache_creation_input_tokens "$t0")"
  pr_r="$(g .usage.cache_read_input_tokens "$pr")"; pr_c="$(g .usage.cache_creation_input_tokens "$pr")"
  e5="$(g .usage.cache_creation.ephemeral_5m_input_tokens "$pr")"; e1="$(g .usage.cache_creation.ephemeral_1h_input_tokens "$pr")"
  nt="$(g .num_turns "$pr")"
  cls="$(awk -v r="$pr_r" -v c="$pr_c" -v t0r="$t0r" -v t0c="$t0c" 'BEGIN{ if (r >= t0r + 0.9*t0c) print "warm"; else if (c >= 0.9*t0c) print "cold"; else print "không rõ" }')"
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$K" "$gap" "$t0r" "$t0c" "$pr_r" "$pr_c" "$e5" "$e1" "$nt" "$cls" >> "$OUT.tmp"
done
if [ "$fail" -eq 0 ]; then mv "$OUT.tmp" "$OUT"; cat "$OUT"; else rm -f "$OUT.tmp"; fi
exit $fail
