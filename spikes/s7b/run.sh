#!/usr/bin/env bash
# S7b - agy 1.4.7 headless probe (ENG-06, D-04, D-26). Re-runnable, from the repo root:
#   bash spikes/s7b/run.sh [--with-skip]      real agy, on the machine where agy is logged in
#   S7B_OFFLINE=1 bash spikes/s7b/run.sh      self-test against tests/fakes/fake_agy.py (no login, no network)
#
# Every agy call goes through ONE of two helpers, each a single bounded line:
#   timeout N agy ... </dev/null > FILE 2> FILE.stderr.txt   (stdout to a file: agy has pipe-output bugs;
#   explicit --print-timeout on every probe; the outer timeout is the second bound).
# Only the agy CLI is invoked; no other launcher. Processes are stopped only by recorded PID and recorded
# descendants (S4 method), never by name. Scratch repos live under $SPIKE_WORK_ROOT/s7b, outside this repo.
# The skip variant (P5) runs only when the user types --with-skip. An authentication message in the output is
# a finding to record, never to work around. Output is scrubbed at the end; numbers and yes/no only.
set -u
. "$(dirname "${BASH_SOURCE[0]}")/../lib/env.sh"

S7B_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
OFFLINE="${S7B_OFFLINE:-0}"
WITH_SKIP=0
for a in "$@"; do
  case "$a" in
    --with-skip) WITH_SKIP=1 ;;
    *) echo "LỖI s7b: tham số lạ: $a (chỉ nhận --with-skip)" >&2; exit 2 ;;
  esac
done

AGY="${AGY_BIN:-agy}"
TMPD=""
T_LONG=60          # outer bound of the --print-timeout 5s probe (P6)
if [ "$OFFLINE" = "1" ]; then
  TMPD="$(mktemp -d)" || { echo "LỖI s7b: không tạo được thư mục tạm" >&2; exit 1; }
  AGY="$REPO/tests/fakes/fake_agy.py"
  OUT="$TMPD/out"
  SPIKE_WORK_ROOT="$TMPD/work"; export SPIKE_WORK_ROOT
  T_LONG=4         # offline: the fake sleeps 30 s, so the outer timeout fires and exit 124 is observed
else
  OUT="$REPO/spikes/s7b/raw"
fi
mkdir -p "$OUT" "$SPIKE_WORK_ROOT/s7b"

if ! command -v -- "${AGY}" >/dev/null 2>&1; then
  echo "LỖI s7b: không tìm thấy agy (AGY_BIN=${AGY}); không có gì để probe" >&2
  [ -n "$TMPD" ] && rm -rf -- "$TMPD"
  exit 3
fi

# ---- recorded-process bookkeeping (copied technique from spikes/s4/run.sh) ----
REC_PIDS=(); REC_ST=(); BG_PID=""

starttime() { [ -r "/proc/$1/stat" ] || return 1; sed 's/^.*) //' "/proc/$1/stat" | awk '{print $20}'; }
procstate() { [ -r "/proc/$1/stat" ] || return 1; sed 's/^.*) //' "/proc/$1/stat" | awk '{print $1}'; }

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

# remember a PID together with its start time, so a recycled PID is never signalled;
# LAST_NEW is the PID when it was newly recorded, empty otherwise (no subshell: the arrays must persist)
LAST_NEW=""
rec() {
  local p="$1" st q
  LAST_NEW=""
  st="$(starttime "$p" 2>/dev/null || true)"; [ -n "$st" ] || return 0
  for q in ${REC_PIDS[@]+"${REC_PIDS[@]}"}; do [ "$q" = "$p" ] && return 0; done
  REC_PIDS+=("$p"); REC_ST+=("$st"); LAST_NEW="$p"
}

# is recorded entry I still the same live (non-zombie) process?
alive_i() {
  local i="$1" p="${REC_PIDS[$1]}" st
  [ "$(starttime "$p" 2>/dev/null || true)" = "${REC_ST[$i]}" ] || return 1
  st="$(procstate "$p" 2>/dev/null || true)"
  [ -n "$st" ] && [ "$st" != "Z" ]
}

# count recorded descendants still alive (entry 0 is the root and is not counted)
alive_count() {
  local i n=0
  for i in "${!REC_PIDS[@]}"; do
    [ "$i" -ge 1 ] || continue
    alive_i "$i" && n=$((n + 1))
  done
  echo "$n"
}

# signal only recorded processes that are still the same process
sig_rec() {
  local s="$1" i
  for i in "${!REC_PIDS[@]}"; do
    alive_i "$i" || continue
    kill -"$s" "${REC_PIDS[$i]}" 2>/dev/null || true
  done
}

# record the whole tree under ROOT; append ps rows of newly seen PIDs to s7b-procs.txt
record_tree() {
  local root="$1" d new=""
  for d in $(descendants "$root"); do
    rec "$d"; [ -n "$LAST_NEW" ] && new="$new${new:+,}$LAST_NEW"
  done
  if [ -n "$new" ]; then
    { echo "# $(ts) new pids=$new"; ps -o pid=,ppid=,pgid=,args= -p "$new" 2>/dev/null || true; } >> "$OUT/s7b-procs.txt"
  fi
}

cleanup() {
  sig_rec TERM >/dev/null 2>&1 || true
  sleep 1
  sig_rec KILL >/dev/null 2>&1 || true
  case "$TMPD" in /tmp/*|"${TMPDIR:-/tmp}"/*) rm -rf -- "$TMPD" ;; esac
}
trap cleanup EXIT

# ---- the two agy call sites ----
# agy_to NAME TIMEOUT ARGS... : one bounded foreground call; exit code goes to NAME.exit
agy_to() {
  timeout "$2" "$AGY" "${@:3}" </dev/null > "$OUT/$1.jsonl" 2> "$OUT/$1.stderr.txt"
  echo "exit_code=$?" > "$OUT/$1.exit"
}
# agy_bg NAME TIMEOUT ARGS... : the same call in the background; sets BG_PID (the timeout wrapper)
agy_bg() {
  timeout "$2" "$AGY" "${@:3}" </dev/null > "$OUT/$1.jsonl" 2> "$OUT/$1.stderr.txt" &
  BG_PID=$!
}
bg_wait() { # NAME : reap the background call and keep its exit code
  wait "$BG_PID" 2>/dev/null
  echo "exit_code=$?" > "$OUT/$1.exit"
}

# ---- scratch repo + offline scenario helpers ----
mkrepo() { # NAME -> prints the scratch repo dir (one commit)
  local d="$SPIKE_WORK_ROOT/s7b/$(date +%Y%m%dT%H%M%S)-$1"
  mkdir -p "$d" && printf 'probe\n' > "$d/README.md" || return 1
  git -C "$d" init -q || return 1
  git -C "$d" add README.md || return 1
  git -C "$d" -c user.name=s7b -c user.email=s7b@example.invalid commit -q -m init || return 1
  echo "$d"
}

scenario() { # NAME JSON : offline only, selects what the fake does for this probe
  [ "$OFFLINE" = "1" ] || return 0
  printf '%s' "$2" > "$TMPD/$1.scenario.json"
  FAKE_AGY_SCENARIO="$TMPD/$1.scenario.json"; export FAKE_AGY_SCENARIO
}

SC_WRITE='{"ops":[{"op":"write","path":"probe.txt","content":"ok\n"}],"response":"DONE\n"}'
SC_SLOW='{"sleep_s":30,"spawn_child":true}'
SC_ERROR='{"status":"ERROR","response":"","exit_code":1,"stderr":"model not found\n"}'
PROMPT_FILE='Tạo file probe.txt chứa đúng chữ ok rồi trả lời DONE.'
PROMPT_SLOW='Đếm chậm từ 1 đến 300, mỗi số một dòng, nghỉ giữa các số.'

probe_file() { # PROBE TIMEOUT SCENARIO_JSON [extra agy args...]
  local p="$1" to="$2" sc="$3" name dir
  name="s7b-$p"; shift 3
  dir="$(mkrepo "$p")" || { echo "LỖI s7b: không tạo được repo tạm cho $p" >&2; exit 1; }
  scenario "$name" "$sc"
  echo "[s7b] probe $p ..."
  ( cd "$dir" && agy_to "$name" "$to" -p "$PROMPT_FILE" --output-format stream-json --print-timeout 120s "$@" )
  if [ -f "$dir/probe.txt" ]; then echo "file=yes" > "$OUT/$name.file"; else echo "file=no" > "$OUT/$name.file"; fi
}

rm -f "$OUT"/s7b-*

# ---- P1: version and help (stderr merged) ----
echo "[s7b] P1 --version / --help"
unset FAKE_AGY_SCENARIO
HELPDIR="$(mkrepo help)" || { echo "LỖI s7b: không tạo được repo tạm cho help" >&2; exit 1; }
( cd "$HELPDIR" && agy_to s7b-helpv 20 --version && agy_to s7b-helph 20 --help )
{
  echo "# agy --version"; cat "$OUT/s7b-helpv.jsonl" "$OUT/s7b-helpv.stderr.txt" "$OUT/s7b-helpv.exit"
  echo "# agy --help";    cat "$OUT/s7b-helph.jsonl" "$OUT/s7b-helph.stderr.txt" "$OUT/s7b-helph.exit"
} > "$OUT/s7b-help.txt"
rm -f "$OUT"/s7b-helpv.* "$OUT"/s7b-helph.*

# ---- P2-P5: a file-write task under each permission setting ----
probe_file default      150 "$SC_WRITE"
probe_file accept-edits 150 "$SC_WRITE" --mode accept-edits
probe_file sandbox      150 "$SC_WRITE" --sandbox
if [ "$WITH_SKIP" = "1" ]; then
  echo "[s7b] P5 skip (người dùng đã gõ --with-skip)"
  probe_file skip 150 "$SC_WRITE" --dangerously-skip-permissions
else
  echo "[s7b] P5 skip: bỏ qua (chỉ chạy khi gõ --with-skip)"
fi

# ---- P6: --print-timeout expiry ----
echo "[s7b] probe timeout (--print-timeout 5s)"
TDIR="$(mkrepo timeout)" || { echo "LỖI s7b: không tạo được repo tạm cho timeout" >&2; exit 1; }
scenario s7b-timeout "$SC_SLOW"
( cd "$TDIR" && agy_to s7b-timeout "$T_LONG" -p "$PROMPT_SLOW" --output-format stream-json --print-timeout 5s )
echo "file=no" > "$OUT/s7b-timeout.file"

# ---- P7: bad model ----
probe_file badmodel 150 "$SC_ERROR" --model khong-co-model-nay

# ---- P8: process tree after a normal exit and after an S4 kill ----
echo "[s7b] P8 cây tiến trình"
: > "$OUT/s7b-procs.txt"
PDIR="$(mkrepo procs)" || { echo "LỖI s7b: không tạo được repo tạm cho procs" >&2; exit 1; }
scenario s7b-procs "$SC_WRITE"
REC_PIDS=(); REC_ST=()
cd "$PDIR" || exit 1
agy_bg s7b-procs 150 -p "$PROMPT_FILE" --output-format stream-json --print-timeout 120s --mode accept-edits
cd "$S7B_DIR" || exit 1
rec "$BG_PID"
t=0
while kill -0 "$BG_PID" 2>/dev/null && [ "$t" -lt 240 ]; do
  record_tree "$BG_PID"; sleep 0.5; t=$((t + 1))
done
if kill -0 "$BG_PID" 2>/dev/null; then
  echo "# vượt 120 s; dừng PID đã ghi $BG_PID" >> "$OUT/s7b-procs.txt"
  kill -TERM "$BG_PID" 2>/dev/null || true
fi
bg_wait s7b-procs
if [ -f "$PDIR/probe.txt" ]; then echo "file=yes" > "$OUT/s7b-procs.file"; else echo "file=no" > "$OUT/s7b-procs.file"; fi
sleep 2
echo "leftover_after_exit=$(alive_count)" >> "$OUT/s7b-procs.txt"
sig_rec TERM; sleep 1; sig_rec KILL        # clean up before the kill run; recorded PIDs only

# kill run: S4 method - record descendants, SIGTERM the recorded PID only, then sweep the recorded descendants
KDIR="$(mkrepo procs-kill)" || { echo "LỖI s7b: không tạo được repo tạm cho procs-kill" >&2; exit 1; }
scenario s7b-procs-kill "$SC_SLOW"
REC_PIDS=(); REC_ST=()
cd "$KDIR" || exit 1
agy_bg s7b-procs-kill 90 -p "$PROMPT_SLOW" --output-format stream-json --print-timeout 120s
cd "$S7B_DIR" || exit 1
rec "$BG_PID"
for _ in 1 2 3 4 5 6; do record_tree "$BG_PID"; sleep 0.5; done
echo "# SIGTERM -> PID đã ghi $BG_PID" >> "$OUT/s7b-procs.txt"
kill -TERM "$BG_PID" 2>/dev/null || true
sleep 5
echo "survivors_after_kill=$(alive_count)" >> "$OUT/s7b-procs.txt"
sig_rec TERM; sleep 2; sig_rec KILL; sleep 1
echo "survivors_after_sweep=$(alive_count)" >> "$OUT/s7b-procs.txt"
bg_wait s7b-procs-kill
echo "file=no" > "$OUT/s7b-procs-kill.file"

# ---- scrub, verify, report ----
scrub_raw "$OUT" >/dev/null
bash "$S7B_DIR/verify.sh" "$OUT" > "$OUT/s7b-verify.txt"
cat "$OUT/s7b-verify.txt"

if [ "$OFFLINE" = "1" ]; then
  fail=""
  for want in \
    "default.permission_mode=request-review" \
    "default.file=no" \
    "accept_edits.file=yes" \
    "accept_edits.status=SUCCESS" \
    "badmodel.exit_code=1" \
    "procs.survivors_after_sweep=0"; do
    grep -qxF -- "$want" "$OUT/s7b-verify.txt" || fail="$fail $want"
  done
  if [ -n "$fail" ]; then
    echo "LỖI s7b:$fail" >&2
    exit 1
  fi
  echo "S7B offline OK"
  exit 0
fi

# real mode: fixtures for plan 06-09 (copied after the scrub)
FIX="$REPO/tests/fixtures/stream"
mkdir -p "$FIX"
if [ -s "$OUT/s7b-accept-edits.jsonl" ]; then
  cp "$OUT/s7b-accept-edits.jsonl" "$FIX/agy_tool_run.jsonl"
elif [ -s "$OUT/s7b-default.jsonl" ]; then
  cp "$OUT/s7b-default.jsonl" "$FIX/agy_tool_run.jsonl"
fi
[ -s "$OUT/s7b-badmodel.jsonl" ] && cp "$OUT/s7b-badmodel.jsonl" "$FIX/agy_error.jsonl"
echo "S7b xong. Dán các dòng s7b-verify.txt ở trên cho agent."
