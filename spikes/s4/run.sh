#!/usr/bin/env bash
# S4 - kill the process tree (SPK-04, D-08, D-12). Linux only.
#
# Run it yourself from a normal terminal:   bash spikes/s4/run.sh      (about 2-4 minutes)
#
# Three runs, each on its own unique marker (sleep 317 / 318 / 319) so nothing else on the machine can match:
#   r1  SIGTERM to the recorded claude PID          (D-12 step 1)
#   r2  SIGKILL to the recorded claude PID          (D-12 step 1, other signal)
#   r3  claude launched via setsid; SIGTERM to its process group   (D-12 step 2)
# Signals go ONLY to: the recorded claude PID, the recorded r3 process group (checked to be the launcher's own
# group, never this script's), and the recorded descendants (checked by /proc start time before every signal).
# No signalling tool that works by process name is used anywhere in this script.
# After each signal: wait 5 s, snapshot, list leftovers by anchored marker, then SWEEP (candidate Phase 3 method):
# every recorded descendant still alive gets TERM, then KILL 2 s later. Observations only; no verdict is written.
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/../lib/env.sh"

[ "$(uname -s)" = "Linux" ] || { echo "S4: Linux only (D-08)" >&2; exit 2; }

RAW="$REPO/spikes/s4/raw"
WORK="$SPIKE_WORK_ROOT/s4"
RUN=""
REC_PIDS=()
REC_ST=()

starttime() { [ -r "/proc/$1/stat" ] || return 1; sed 's/^.*) //' "/proc/$1/stat" | awk '{print $20}'; }
ppid_of()   { [ -r "/proc/$1/stat" ] || return 0; sed 's/^.*) //' "/proc/$1/stat" | awk '{print $2}'; }

# does ANCESTOR sit on PID's PPid chain?
reaches() {
  local p="$1" a="$2" i=0
  while [ -n "$p" ] && [ "$p" -gt 1 ] 2>/dev/null && [ "$i" -lt 40 ]; do
    [ "$p" = "$a" ] && return 0
    p="$(ppid_of "$p")"; i=$((i + 1))
  done
  return 1
}

# all descendants of ROOT (recursive), one PID per line, from a single ps listing
descendants() {
  ps -eo pid=,ppid= | awk -v root="$1" '
    { kids[$2] = kids[$2] " " $1 }
    END {
      head = 1; tail = 0
      n = split(kids[root], a, " "); for (i = 1; i <= n; i++) queue[++tail] = a[i]
      while (head <= tail) { p = queue[head++]; print p; m = split(kids[p], b, " "); for (j = 1; j <= m; j++) queue[++tail] = b[j] }
    }'
}

# remember a PID together with its start time, so a recycled PID is never signalled
rec() {
  local p="$1" st q
  st="$(starttime "$p" 2>/dev/null || true)"; [ -n "$st" ] || return 0
  for q in ${REC_PIDS[@]+"${REC_PIDS[@]}"}; do [ "$q" = "$p" ] && return 0; done
  REC_PIDS+=("$p"); REC_ST+=("$st")
}

# signal only recorded processes that are still the same process; print what was signalled
sig_rec() {
  local s="$1" i p
  for i in ${!REC_PIDS[@]+"${!REC_PIDS[@]}"}; do
    p="${REC_PIDS[$i]}"
    [ "$(starttime "$p" 2>/dev/null || true)" = "${REC_ST[$i]}" ] || continue
    kill -"$s" "$p" 2>/dev/null || continue
    echo "$s $p"
  done
}

rows() { # ps rows for a comma-separated PID list
  [ -n "$1" ] || return 0
  ps -o pid=,ppid=,pgid=,sid=,etimes=,args= -p "$1" 2>/dev/null || true
}
rec_csv() { local IFS=,; echo "${REC_PIDS[*]-}"; }

snap() { # label
  { echo "# $(ts) run=r$RUN $1"; echo "pid ppid pgid sid etimes args"; rows "$(rec_csv)"; } > "$RAW/s4-r$RUN-$1.txt"
}

marker_pids() { pgrep -f "^sleep $1\$" 2>/dev/null || true; }

marker_report() { # marker -> "pid=N ppid=M" lines, or none
  local m="$1" out="" p
  for p in $(marker_pids "$m"); do out="$out""pid=$p ppid=$(ppid_of "$p")"$'\n'; done
  if [ -n "$out" ]; then printf '%s' "$out"; else echo none; fi
}

cleanup() { # only recorded PIDs; a normal finish finds nothing alive
  sig_rec TERM >/dev/null 2>&1 || true
  sleep 1
  sig_rec KILL >/dev/null 2>&1 || true
}

run_one() { # N MARKER MODE(pid-term|pid-kill|group-term)
  RUN="$1"; local M="$2" MODE="$3" lp cp="" mp="" i p pgid="" ec="na" d out="$RAW/s4-r$1.jsonl"
  REC_PIDS=(); REC_ST=()
  local -a launcher=()
  [ "$MODE" = "group-term" ] && launcher=(setsid)
  echo "[r$RUN] launching (mode $MODE, marker sleep $M)"
  ( cd "$WORK" && exec ${launcher[@]+"${launcher[@]}"} "${CLEAN_ENV[@]}" timeout 600 claude -p "Run this exact bash command and wait for it: sleep $M" --allowedTools "Bash(sleep *)" --output-format stream-json --verbose </dev/null > "$out" 2> "$WORK/r$RUN.stderr" ) &
  lp=$!
  rec "$lp"
  # claude = the single child of the launcher (timeout forks it); never looked up by name
  for i in $(seq 1 20); do
    cp="$(pgrep -P "$lp" 2>/dev/null | head -n1 || true)"; [ -n "$cp" ] && break; sleep 0.5
  done
  if [ -z "$cp" ]; then
    echo "[r$RUN] claude child not found" >&2
    for f in descendants before after sweep; do printf 'launcher-has-no-child\n' > "$RAW/s4-r$RUN-$f.txt"; done
    echo "marker-not-started" > "$RAW/s4-r$RUN-leftover.txt"
    sig_rec TERM >/dev/null 2>&1 || true; wait "$lp" 2>/dev/null || true
    echo "r$RUN exit_code=na" >> "$RAW/s4-exit.txt"
    marker_report "$M" > "$RAW/s4-r$RUN-final.txt"
    return 0
  fi
  rec "$cp"

  # wait up to 90 s for the marker process whose PPid chain reaches the claude PID
  for i in $(seq 1 90); do
    for p in $(marker_pids "$M"); do
      if reaches "$p" "$cp"; then mp="$p"; break; fi
    done
    [ -n "$mp" ] && break
    sleep 1
  done

  # record the full descendant set of claude BEFORE signalling (orphans re-parent afterwards)
  local dl=""
  for d in $(descendants "$cp"); do rec "$d"; dl="$dl${dl:+,}$d"; done
  { echo "# $(ts) run=r$RUN descendants of claude (marker_pid=${mp:-none})"; echo "pid ppid pgid sid etimes args"; rows "$dl"; } > "$RAW/s4-r$RUN-descendants.txt"
  snap before

  if [ -z "$mp" ]; then
    echo "[r$RUN] marker never appeared; terminating only this run's own claude PID"
    kill -TERM "$cp" 2>/dev/null || true
  else
    case "$MODE" in
      pid-term)   echo "[r$RUN] SIGTERM -> claude pid $cp"; kill -TERM "$cp" ;;
      pid-kill)   echo "[r$RUN] SIGKILL -> claude pid $cp"; kill -KILL "$cp" ;;
      group-term)
        pgid="$(ps -o pgid= -p "$cp" | tr -d ' ')"
        echo "launcher=$lp claude=$cp pgid=$pgid" >> "$RAW/s4-r$RUN-descendants.txt"
        # the group must be the launcher's own, and never this script's
        if [ "$pgid" = "$lp" ] && [ "$pgid" != "$(ps -o pgid= -p $$ | tr -d ' ')" ]; then
          echo "[r$RUN] SIGTERM -> process group $pgid"; kill -TERM -- "-$pgid"
        else
          echo "[r$RUN] refusing group kill: pgid $pgid is not the launcher's own group" >&2
          echo "group-kill-refused pgid=$pgid launcher=$lp" >> "$RAW/s4-r$RUN-descendants.txt"
          kill -TERM "$cp" 2>/dev/null || true
        fi ;;
    esac
  fi

  sleep 5
  snap after
  if [ -n "$mp" ]; then marker_report "$M" > "$RAW/s4-r$RUN-leftover.txt"; else echo "marker-not-started" > "$RAW/s4-r$RUN-leftover.txt"; fi

  # launcher exit code
  for i in $(seq 1 20); do kill -0 "$lp" 2>/dev/null || break; sleep 0.5; done
  if kill -0 "$lp" 2>/dev/null; then ec="still-running"; else if wait "$lp" 2>/dev/null; then ec=0; else ec=$?; fi; fi
  echo "r$RUN exit_code=$ec" >> "$RAW/s4-exit.txt"

  # sweep: every recorded descendant still alive gets TERM, then KILL after 2 s
  { sig_rec TERM; sleep 2; sig_rec KILL; } > "$RAW/s4-r$RUN-sweep.txt"
  [ -s "$RAW/s4-r$RUN-sweep.txt" ] || echo none > "$RAW/s4-r$RUN-sweep.txt"
  sleep 1
  marker_report "$M" > "$RAW/s4-r$RUN-final.txt"
  echo "[r$RUN] leftover: $(tr '\n' ' ' < "$RAW/s4-r$RUN-leftover.txt") | swept: $(tr '\n' ' ' < "$RAW/s4-r$RUN-sweep.txt") | final: $(tr '\n' ' ' < "$RAW/s4-r$RUN-final.txt") | launcher exit: $ec"
}

main() {
  trap cleanup EXIT
  mkdir -p "$RAW"
  rm -f "$RAW"/s4-r[123]* "$RAW/s4-exit.txt" "$RAW/s4-env.txt"
  case "$WORK" in "$SPIKE_WORK_ROOT"/s4) rm -rf -- "$WORK" ;; esac
  mkdir -p "$WORK"; printf 'a\n' > "$WORK/a.txt"
  {
    echo "nested=$([ -n "${CLAUDECODE:-}" ] && echo yes || echo no)"
    echo "date=$(ts)"
    echo "claude_version=$(run_claude --version 2>&1 | head -n1)"
    echo "kernel=$(uname -sr)"
  } > "$RAW/s4-env.txt"
  : > "$RAW/s4-exit.txt"

  run_one 1 317 pid-term
  run_one 2 318 pid-kill
  run_one 3 319 group-term

  scrub_raw "$RAW" >/dev/null
  echo "S4 finished. Next: tell the agent \"s4 done\" (or paste any error output above)."
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then main "$@"; fi
