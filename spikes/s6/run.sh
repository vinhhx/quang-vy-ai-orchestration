#!/usr/bin/env bash
# S6 - DeepSeek through the dsh-deepseek-harness container (SPK-06, D-07). Re-runnable.
#   bash spikes/s6/run.sh          basic: headless call via docker exec, help, file edit, output format, cleanup
#   bash spikes/s6/run.sh --kill   kill-from-outside variants (a) and (b); run by the USER, not the agent
# Secrets (D-07): never lists the container environment, never runs docker inspect, never reads .env.docker,
# never restarts/stops/rebuilds the container. Signals go only to the host-side docker exec client this script
# started and to in-container PIDs that carry this run's unique marker (or descendants recorded before the kill).
# No name-based signalling tool is used. Numbers and yes/no only; no verdict.
set -u
. "$(dirname "${BASH_SOURCE[0]}")/../lib/env.sh"

C=dsh-deepseek-harness
RAW="$REPO/spikes/s6/raw"
SCR="$REPO/spikes/s6/scratch"
mkdir -p "$RAW"

guard_scr() {
  case "$SCR" in
    */spikes/s6/scratch) return 0 ;;
    *) echo "S6: refusing, scratch path does not end in /spikes/s6/scratch" >&2; return 1 ;;
  esac
}
make_scr() { guard_scr || return 1; mkdir -p "$SCR" && printf 'hello\n' > "$SCR/a.txt"; }
cleanup_scr() {
  guard_scr || return 1
  # root-owned files may exist under the bind mount, so remove them from inside the container
  docker exec "$C" rm -rf -- "$SCR" </dev/null >/dev/null 2>&1
  rmdir "$SCR" 2>/dev/null
  return 0
}
need_container() {
  case "$(docker ps --filter "name=^${C}\$" --format '{{.Status}}' 2>/dev/null)" in
    Up*) return 0 ;;
    *) echo "S6: container $C is not running; not starting or restarting it" >&2; return 1 ;;
  esac
}

# ---------------------------------------------------------------- basic
mode_basic() {
  need_container || return 3
  make_scr || return 3
  trap cleanup_scr EXIT

  # help text of the headless profile
  { timeout 60 docker exec "$C" dsh --profile headless --help </dev/null 2>&1; echo "exit_code=$?"; } > "$RAW/s6-dsh-help.txt"

  # the headless task: append a line to a.txt
  timeout 300 docker exec -w "$SCR" "$C" \
    dsh --profile headless "Append the line 'edited-by-dsh' to a.txt, then reply DONE" </dev/null \
    > "$RAW/s6-stdout.txt" 2> "$RAW/s6-stderr.txt"
  local ec=$?
  echo "exit_code=$ec" >> "$RAW/s6-stderr.txt"

  # file content and owner after the run
  {
    echo "--- a.txt content"
    cat "$SCR/a.txt" 2>&1
    echo "--- owner uid:gid=$(stat -c '%u:%g' "$SCR/a.txt" 2>/dev/null || echo missing)"
    echo "me uid:gid=$(id -u):$(id -g)"
    echo "contains_edited_by_dsh=$(grep -qx 'edited-by-dsh' "$SCR/a.txt" 2>/dev/null && echo yes || echo no)"
  } > "$RAW/s6-a-after.txt"

  # format check: lines that parse as a JSON object vs the rest
  local j=0 t=0 line
  while IFS= read -r line || [ -n "$line" ]; do
    [ -n "$line" ] || continue
    if printf '%s' "$line" | jq -e 'type=="object"' >/dev/null 2>&1; then j=$((j+1)); else t=$((t+1)); fi
  done < "$RAW/s6-stdout.txt"
  echo "json_lines=$j text_lines=$t" > "$RAW/s6-format.txt"

  cleanup_scr
  trap - EXIT
  scrub_raw "$RAW" >/dev/null
  echo "S6 basic done: exit_code=$ec $(cat "$RAW/s6-format.txt")"
}

# ---------------------------------------------------------------- kill
snapshot() { # prints "pid ppid pgid args" for every in-container process
  docker exec "$C" ps -ww -eo pid=,ppid=,pgid=,args= </dev/null 2>/dev/null
}

# tree SNAPSHOT_FILE MARKER [BEFORE_FILE]
#   rows whose args carry MARKER, plus their descendants; with BEFORE_FILE also every recorded row that is
#   still alive with the same pid and the same args prefix (orphans re-parented by the kill).
tree() {
  python3 -I - "$@" <<'PY'
import re, sys
snap, marker = sys.argv[1], sys.argv[2]
before = sys.argv[3] if len(sys.argv) > 3 else None
rows = {}
for ln in open(snap, errors="replace"):
    m = re.match(r"\s*(\d+)\s+(\d+)\s+(\d+)\s+(.*)$", ln)
    if m:
        rows[int(m.group(1))] = (int(m.group(2)), int(m.group(3)), m.group(4))
sel = {p for p, (pp, pg, a) in rows.items() if marker in a}
grew = True
while grew:
    grew = False
    for p, (pp, pg, a) in rows.items():
        if pp in sel and p not in sel:
            sel.add(p); grew = True
if before:
    try:
        for ln in open(before, errors="replace"):
            m = re.match(r"(\d+) \d+ \d+ (.*)$", ln)
            if m and int(m.group(1)) in rows and rows[int(m.group(1))][2][:100] == m.group(2):
                sel.add(int(m.group(1)))
    except OSError:
        pass
for p in sorted(sel):
    pp, pg, a = rows[p]
    print(p, pp, pg, a[:100].replace("\n", " "))
PY
}

one_variant() { # VARIANT(a|b)
  local V="$1" NONCE MARK PIDF LAUNCH CLIENT i n
  NONCE="$(head -c 6 /dev/urandom | od -An -tx1 | tr -d ' \n')"
  MARK="s6k-$NONCE"
  PIDF="/tmp/s6pid-$NONCE"
  local TD="$SPIKE_WORK_ROOT/s6"; mkdir -p "$TD"
  local OUT="$RAW/s6-kill-$V.txt" S1="$TD/snap1-$V.txt" S2="$TD/snap2-$V.txt" BEF="$TD/before-$V.txt"
  local PROMPT="Run the bash command sleep 120 and wait for it, then reply DONE. Marker: $MARK"
  : > "$OUT"
  echo "variant=$V marker=$MARK" >> "$OUT"

  if [ "$V" = a ]; then
    timeout 300 docker exec -w "$SCR" "$C" dsh --profile headless "$PROMPT" </dev/null > "$RAW/s6-kill-$V-stdout.txt" 2>&1 &
  else
    timeout 300 docker exec -w "$SCR" "$C" sh -c "echo \$\$ > $PIDF; exec dsh --profile headless \"\$1\"" sh "$PROMPT" </dev/null > "$RAW/s6-kill-$V-stdout.txt" 2>&1 &
  fi
  LAUNCH=$!
  sleep 1
  CLIENT="$(pgrep -P "$LAUNCH" | head -n1)"   # the host-side docker exec client is the only child of timeout
  echo "launcher_pid=$LAUNCH client_pid=${CLIENT:-none}" >> "$OUT"

  # wait up to 120 s for the marker process (and its sleep child) to exist in the container
  local seen=no sleepseen=no
  for i in $(seq 1 40); do
    snapshot > "$S1"
    if [ -n "$(tree "$S1" "$MARK")" ]; then seen=yes; fi
    if [ "$seen" = yes ] && tree "$S1" "$MARK" | awk '{ $1=$2=$3=""; print }' | grep -qE '^ *sleep 120'; then sleepseen=yes; break; fi
    sleep 3
  done
  echo "marker_seen=$seen sleep_child_seen=$sleepseen" >> "$OUT"
  tree "$S1" "$MARK" > "$BEF"
  { echo "before:"; sed 's/^/  /' "$BEF"; } >> "$OUT"

  if [ "$seen" != yes ]; then
    echo "marker-not-started" >> "$OUT"
    [ -n "${CLIENT:-}" ] && kill -TERM "$CLIENT" 2>/dev/null   # only the client this script started
    wait "$LAUNCH" 2>/dev/null
    echo "leftover: none" >> "$OUT"; echo "sweep: none" >> "$OUT"
    rm -f "$S1" "$S2" "$BEF"
    return 0
  fi

  # the signal
  if [ "$V" = a ]; then
    echo "signal: TERM to host-side docker exec client pid=$CLIENT" >> "$OUT"
    kill -TERM "$CLIENT"
  else
    local INPID; INPID="$(docker exec "$C" cat "$PIDF" </dev/null 2>/dev/null | tr -dc '0-9')"
    echo "signal: TERM to in-container pid=${INPID:-none} (recorded in $PIDF) via docker exec" >> "$OUT"
    [ -n "$INPID" ] && docker exec "$C" kill -TERM "$INPID" </dev/null
  fi
  sleep 5
  wait "$LAUNCH" 2>/dev/null; echo "launcher_exit_code=$?" >> "$OUT"
  if [ -n "${CLIENT:-}" ] && kill -0 "$CLIENT" 2>/dev/null; then echo "client_alive_after_signal=yes" >> "$OUT"; else echo "client_alive_after_signal=no" >> "$OUT"; fi

  # leftover check
  snapshot > "$S2"
  tree "$S2" "$MARK" "$BEF" > "$S2.left"
  if [ -s "$S2.left" ]; then
    echo "leftover:" >> "$OUT"; sed 's/^/  /' "$S2.left" >> "$OUT"
    # sweep by PID only: TERM, then KILL for survivors
    local pids; pids="$(awk '{print $1}' "$S2.left" | tr '\n' ' ')"
    for n in $pids; do docker exec "$C" kill -TERM "$n" </dev/null 2>/dev/null; done
    sleep 2
    snapshot > "$S2"; tree "$S2" "$MARK" "$BEF" > "$S2.left2"
    local pids2; pids2="$(awk '{print $1}' "$S2.left2" | tr '\n' ' ')"
    for n in $pids2; do docker exec "$C" kill -KILL "$n" </dev/null 2>/dev/null; done
    sleep 1
    echo "sweep: TERM pids=${pids}; KILL pids=${pids2:-none}" >> "$OUT"
    snapshot > "$S2"; tree "$S2" "$MARK" "$BEF" > "$S2.left3"
    if [ -s "$S2.left3" ]; then echo "final:" >> "$OUT"; sed 's/^/  /' "$S2.left3" >> "$OUT"; else echo "final: none" >> "$OUT"; fi
  else
    echo "leftover: none" >> "$OUT"
    echo "sweep: none" >> "$OUT"
    echo "final: none" >> "$OUT"
  fi
  [ "$V" = b ] && docker exec "$C" rm -f -- "$PIDF" </dev/null >/dev/null 2>&1
  rm -f "$S1" "$S2" "$S2.left" "$S2.left2" "$S2.left3" "$BEF"
}

mode_kill() {
  need_container || return 3
  make_scr || return 3
  trap cleanup_scr EXIT
  one_variant a
  # reset the scratch file between variants
  printf 'hello\n' > "$SCR/a.txt" 2>/dev/null || { docker exec "$C" sh -c "printf 'hello\\n' > '$SCR/a.txt'" </dev/null; }
  one_variant b
  rm -f "$RAW"/s6-kill-a-stdout.txt "$RAW"/s6-kill-b-stdout.txt
  cleanup_scr
  trap - EXIT
  scrub_raw "$RAW" >/dev/null
  echo "S6 kill done:"; cat "$RAW/s6-kill-a.txt" "$RAW/s6-kill-b.txt"
}

case "${1:-}" in
  "") mode_basic ;;
  --kill) mode_kill ;;
  *) echo "usage: bash spikes/s6/run.sh [--kill]" >&2; exit 2 ;;
esac
