#!/usr/bin/env bash
# E-09 - one Anthropic-format endpoint through the claude CLI (Phase 6, D-34 / D-40, ENG-11).
# Re-runnable: bash spikes/e09/run.sh
#
# Real mode (the user runs it, with their own key exported only in that shell):
#   First run (D-34, the company gateway first):
#     export VCADM_GATEWAY_KEY=...   # never written to a file, never on argv
#     E09_ROUTE_CHECK=1 E09_KEY_ENV=VCADM_GATEWAY_KEY E09_NAME=vcadm-deepseek \
#       E09_BASE_URL=https://ai-gateway.vcadm.vn/proxyllm \
#       E09_MODELS="deepseek/deepseek-chat deepseek/deepseek-reasoner" bash spikes/e09/run.sh
#   Optional (DeepSeek's official endpoint, defaults):
#     export DEEPSEEK_API_KEY=...  &&  bash spikes/e09/run.sh
#   Probe A: one tiny real `claude -p` call per model through ANTHROPIC_BASE_URL (creates e09.txt, replies DONE).
#   Probe B: once, with the first model: the same call with the fixed invalid token (no billing), to capture a real
#            auth-error stream.
#   Probe A is repeated once per model (prefix e09-a2) with CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1 and
#   CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING=1 only if it fails.
#   Env knobs:
#     E09_KEY_ENV      name of the variable that holds the key (default DEEPSEEK_API_KEY; ^[A-Z_][A-Z0-9_]*$)
#     E09_NAME         endpoint label, lowercase slug (default deepseek; ^[a-z0-9][a-z0-9-]*$)
#     E09_BASE_URL     endpoint (default https://api.deepseek.com/anthropic)
#     E09_MODELS       space-separated models, one probe A each (default $E09_MODEL, else sonnet);
#                      E09_MODEL / E09_EXTRA_MODEL (old knobs) still work and are folded into the list
#     E09_ROUTE_CHECK  1 = one unauthenticated zero-token POST {base}/v1/messages, prints only route.status=<code>
#     E09_CLAUDE_BIN   claude binary (default claude)
#
# Evidence never collides between endpoints: raw output goes to spikes/e09/raw/{name}/m{k}/, fixtures are
# tests/fixtures/stream/anthropic_e09_{name}_ok.jsonl, anthropic_e09_{name}_m{k}_ok.jsonl (k >= 2) and
# anthropic_e09_{name}_auth_error.jsonl. A probe stopped by the leak gate copies nothing.
#
# Offline self-test (no network, no key, no repo file touched):
#   E09_OFFLINE=1 bash spikes/e09/run.sh   -> prints "E09 offline OK"
#   It runs the configured endpoint plus a second fixed one (gw-test) into a simulated repo and checks both
#   sets of evidence sit side by side.
#
# The key reaches the claude child only through the ANTHROPIC_AUTH_TOKEN environment variable. Probe outputs are
# staged outside the repo, scrubbed, leak-checked, and only then copied to spikes/e09/raw and tests/fixtures/stream.
# Only the claude CLI is used (no dsh container).
#
# Exit codes: 0 ok, 1 other error, 2 missing key, 3 leak gate failed (nothing copied).
set -u
. "$(dirname "${BASH_SOURCE[0]}")/../lib/env.sh"

HERE="$REPO/spikes/e09"
E09_CLAUDE_BIN="${E09_CLAUDE_BIN:-claude}"
E09_BASE_URL="${E09_BASE_URL:-https://api.deepseek.com/anthropic}"
E09_KEY_ENV="${E09_KEY_ENV:-DEEPSEEK_API_KEY}"
E09_NAME="${E09_NAME:-deepseek}"
E09_MODELS="${E09_MODELS:-${E09_MODEL:-sonnet}}"
E09_EXTRA_MODEL="${E09_EXTRA_MODEL:-}"
E09_INVALID_TOKEN=invalid-key-for-e09-probe
OFFLINE="${E09_OFFLINE:-0}"
E09_CWD=""
E09_WRITTEN=()
E09_DONE=()
E09_MODEL_LIST=()
E09_LEAK_KEYS=()
E09_TEST_NAME=gw-test
E09_TEST_KEY_ENV=E09_TEST_GW_KEY

die() { echo "LỖI e09: $*"; exit 1; }

# ---- knob validation (before any key is read or any call is made) ------------------------------------------------
[[ "$E09_KEY_ENV" =~ ^[A-Z_][A-Z0-9_]*$ ]] || die "E09_KEY_ENV phải khớp ^[A-Z_][A-Z0-9_]*\$"
[[ "$E09_NAME" =~ ^[a-z0-9][a-z0-9-]*$ ]] || die "E09_NAME phải khớp ^[a-z0-9][a-z0-9-]*\$"
[[ "$E09_BASE_URL" =~ ^https?://[^[:space:]]+$ ]] || die "E09_BASE_URL phải là URL http(s) không có khoảng trắng"
[ -n "$E09_EXTRA_MODEL" ] && E09_MODELS="$E09_MODELS $E09_EXTRA_MODEL"
read -r -a PRIMARY_MODELS <<< "$E09_MODELS"
[ "${#PRIMARY_MODELS[@]}" -ge 1 ] || die "E09_MODELS rỗng"
for _m in "${PRIMARY_MODELS[@]}"; do
  [[ "$_m" =~ ^[A-Za-z0-9][A-Za-z0-9._:/@-]*$ ]] || die "model không hợp lệ: $_m"
done

if [ "$OFFLINE" = 1 ]; then
  [ "$E09_NAME" != "$E09_TEST_NAME" ] || die "offline: E09_NAME=$E09_TEST_NAME được dành cho endpoint thử thứ hai"
  [ "$E09_KEY_ENV" != "$E09_TEST_KEY_ENV" ] || die "offline: E09_KEY_ENV=$E09_TEST_KEY_ENV được dành cho endpoint thử thứ hai"
  E09_CLAUDE_BIN="$REPO/tests/fakes/fake_claude.py"
  KEY="e09-offline-planted-0123456789"
  TEST_KEY="e09-offline-gw-planted-9876543210"
  export "$E09_KEY_ENV=$KEY"
  export "$E09_TEST_KEY_ENV=$TEST_KEY"
  E09_LEAK_KEYS=("$KEY" "$TEST_KEY")
  WORK="$(mktemp -d)" || die "mktemp"
  trap 'rm -rf "$WORK"' EXIT
else
  KEY="${!E09_KEY_ENV:-}"
  if [ -z "$KEY" ]; then
    echo "Thiếu ${E09_KEY_ENV}"
    exit 2
  fi
  E09_LEAK_KEYS=("$KEY")
  WORK="$SPIKE_WORK_ROOT/e09/$(date -u +%Y%m%dT%H%M%SZ)"
  mkdir -p "$WORK" || die "mkdir $WORK"
fi
OUT="$WORK/out"
mkdir -p "$OUT" "$WORK/scn" || die "mkdir $OUT"

# e09_use_endpoint NAME KEY_ENV BASE_URL MODEL... : selects the endpoint the next functions work on.
e09_use_endpoint() {
  E09_NAME="$1"
  E09_KEY_ENV="$2"
  E09_BASE_URL="$3"
  shift 3
  E09_MODEL_LIST=("$@")
}

# e09_call PREFIX TOKEN_VAR ARGS... : one bounded claude call in $E09_CWD.
# The token is read from the variable NAMED by TOKEN_VAR and exported in a subshell, so it travels only through
# the environment and never appears on any argv (not even the env helper's).
e09_call() {
  local prefix="$1" tokvar="$2"
  shift 2
  (
    cd "$E09_CWD" || exit 1
    export ANTHROPIC_BASE_URL="$E09_BASE_URL"
    export CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1
    export ANTHROPIC_AUTH_TOKEN="${!tokvar}"
    if [ "${E09_RETRY:-0}" = 1 ]; then
      export CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1 CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING=1
    fi
    "${CLEAN_ENV[@]}" -u ANTHROPIC_API_KEY -u ANTHROPIC_MODEL -u ANTHROPIC_DEFAULT_OPUS_MODEL -u ANTHROPIC_DEFAULT_SONNET_MODEL -u ANTHROPIC_DEFAULT_HAIKU_MODEL -u CLAUDE_CODE_SUBAGENT_MODEL timeout 180 "$E09_CLAUDE_BIN" "$@" </dev/null > "$prefix.jsonl" 2> "$prefix.stderr.txt"
    echo "exit_code=$?" > "$prefix.exit"
  )
}

# e09_mkrepo DIR : scratch git repo (outside the repo) with one commit of a.txt.
e09_mkrepo() {
  local d="$1"
  mkdir -p "$d" || return 1
  git init -q "$d" || return 1
  echo base > "$d/a.txt"
  git -C "$d" add a.txt || return 1
  git -C "$d" -c user.email=e09@example.invalid -c user.name=e09 commit -qm init || return 1
}

# e09_file_check DIR OUTFILE : did the probe create e09.txt with the single word ok?
e09_file_check() {
  local d="$1" out="$2" f=no c=no
  if [ -f "$d/e09.txt" ]; then
    f=yes
    [ "$(tr -d '[:space:]' < "$d/e09.txt" | tr '[:upper:]' '[:lower:]')" = ok ] && c=yes
  fi
  printf 'file=%s\ncontent_ok=%s\n' "$f" "$c" > "$out"
}

# e09_use_scenario NAME : offline only, selects the fake claude scenario for the next call.
e09_use_scenario() {
  if [ "$OFFLINE" = 1 ]; then
    export FAKE_CLAUDE_SCENARIO="$WORK/scn/$1.json"
  fi
}

# e09_verify DIR : the observation lines for one model directory, with the current endpoint's key variable.
e09_verify() {
  E09_KEY_ENV="$E09_KEY_ENV" bash "$HERE/verify.sh" "$1"
}

# e09_leak_check DIR : returns 0 when no key in play is found anywhere under DIR (status only, never prints a key).
e09_leak_check() {
  local k
  for k in "${E09_LEAK_KEYS[@]}"; do
    [ -n "$k" ] || continue
    if grep -rqF -- "$k" "$1" 2>/dev/null; then return 1; fi
  done
  return 0
}

# e09_probe_ok DIR TAG NAME ARGS... : probe-A-shaped call (asks claude to create e09.txt) in a fresh scratch repo;
# output files are DIR/e09-NAME.*
e09_probe_ok() {
  local dir="$1" tag="$2" name="$3"
  shift 3
  local d="$WORK/repo-$tag-$name"
  e09_mkrepo "$d" || die "git init $d"
  E09_CWD="$d"
  e09_use_scenario a
  e09_call "$dir/e09-$name" "$E09_KEY_ENV" "${E09_ARGS[@]}" "$@"
  e09_file_check "$d" "$dir/e09-$name.file"
}

E09_ARGS=(-p "Tạo file e09.txt chứa đúng một từ: ok. Sau đó trả lời DONE." --output-format stream-json --verbose --max-turns 4 --allowedTools "Write(./e09.txt)" --strict-mcp-config)

if [ "$OFFLINE" = 1 ]; then
  cat > "$WORK/scn/a.json" <<'JSON'
{"ops":[{"op":"write","path":"e09.txt","content":"ok"}],
 "events":[
  {"type":"system","subtype":"init","session_id":"e09-fake","model":"deepseek-fake","apiKeySource":"ANTHROPIC_AUTH_TOKEN"},
  {"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"Write","input":{"file_path":"e09.txt","content":"ok"}}]}},
  {"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_1","content":"ok"}]}},
  {"type":"result","subtype":"success","is_error":false,"num_turns":2,"result":"DONE","total_cost_usd":0.0001,"usage":{"input_tokens":12,"output_tokens":7},"modelUsage":{"deepseek-fake":{"inputTokens":12,"outputTokens":7}}}
 ]}
JSON
  cat > "$WORK/scn/b.json" <<'JSON'
{"events":[
  {"type":"system","subtype":"init","session_id":"e09-fake-b","model":"deepseek-fake","apiKeySource":"ANTHROPIC_AUTH_TOKEN"},
  {"type":"result","subtype":"success","is_error":true,"api_error_status":401,"num_turns":1,"result":"Invalid API key · Please run /login","total_cost_usd":0,"usage":{"input_tokens":0,"output_tokens":0}}
 ],
 "exit_code":1}
JSON
fi

# e09_route_check : E09_ROUTE_CHECK=1 only; one unauthenticated, zero-token POST. Prints only route.status=<code>.
e09_route_check() {
  [ "${E09_ROUTE_CHECK:-0}" = 1 ] || return 0
  if [ "$OFFLINE" = 1 ]; then
    echo "route.status=skipped-offline"
    return 0
  fi
  local code
  code="$(curl -sS -o /dev/null -w '%{http_code}' --max-time 15 -X POST -H 'content-type: application/json' --data '{}' "${E09_BASE_URL%/}/v1/messages" 2>/dev/null)" || true
  [[ "$code" =~ ^[0-9]{3}$ ]] || code=000
  echo "route.status=$code"
}

# e09_probe_endpoint : probe A per model (+ one a2 retry), probe B once (with the first model), all staged under
# $OUT/$E09_NAME/m{k}/. Uses the globals set by e09_use_endpoint.
e09_probe_endpoint() {
  local k=0 model dir used
  for model in "${E09_MODEL_LIST[@]}"; do
    k=$((k + 1))
    dir="$OUT/$E09_NAME/m$k"
    mkdir -p "$dir" || die "mkdir $dir"
    echo "E09: endpoint=$E09_NAME probe A (gọi thật qua $E09_BASE_URL, model $model)"
    e09_probe_ok "$dir" "$E09_NAME-m$k" a --model "$model"
    used=a
    if e09_verify "$dir" | grep -qE '^a\.(result_seen=no|result_is_error=yes)$'; then
      echo "E09: probe A lỗi, thử lại một lần với CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1 và CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING=1"
      E09_RETRY=1 e09_probe_ok "$dir" "$E09_NAME-m$k" a2 --model "$model"
      used=a2
    fi
    echo "$used" > "$dir/e09-used-a.txt"
    if [ "$k" = 1 ]; then
      echo "E09: endpoint=$E09_NAME probe B (token sai cố định, không tính phí)"
      e09_mkrepo "$WORK/repo-$E09_NAME-b" || die "git init repo-b"
      E09_CWD="$WORK/repo-$E09_NAME-b"
      e09_use_scenario b
      e09_call "$dir/e09-b" E09_INVALID_TOKEN "${E09_ARGS[@]}" --model "$model"
    fi
  done
  # scrub, then write the per-model observation file
  scrub_raw "$OUT/$E09_NAME" >/dev/null
  for k in $(seq 1 "${#E09_MODEL_LIST[@]}"); do
    dir="$OUT/$E09_NAME/m$k"
    e09_verify "$dir" > "$dir/e09-verify.txt"
    scrub_raw "$dir/e09-verify.txt" >/dev/null
  done
  E09_DONE+=("$E09_NAME:${#E09_MODEL_LIST[@]}")
}

# e09_unwrite : remove only what the last e09_copy_evidence wrote.
e09_unwrite() {
  local p
  for p in ${E09_WRITTEN[@]+"${E09_WRITTEN[@]}"}; do
    case "$p" in
      */raw/*) [ -d "$p" ] && rm -rf -- "$p" ;;
      *) rm -f -- "$p" ;;
    esac
  done
  E09_WRITTEN=()
}

# e09_copy_evidence DEST_RAW_ROOT DEST_FIX_DIR : copy $OUT/$E09_NAME to DEST_RAW_ROOT/$E09_NAME and write the
# fixtures of this endpoint only. Returns 3 (writing nothing) when the leak gate finds a key anywhere under $OUT.
# Never touches another endpoint name's files.
e09_copy_evidence() {
  local rawroot="$1" fixdir="$2" name="$E09_NAME" src dest k used fix f
  src="$OUT/$name"
  [ -d "$src/m1" ] || return 1
  e09_leak_check "$OUT" || return 3
  dest="$rawroot/$name"
  case "$dest" in
    */raw/"$name") ;;
    *) echo "e09: từ chối thư mục đích $dest (không kết thúc bằng /raw/$name)" >&2; return 1 ;;
  esac
  E09_WRITTEN=()
  mkdir -p "$rawroot" "$fixdir" || return 1
  rm -rf -- "$dest" || return 1
  mkdir -p "$dest" || return 1
  cp -R "$src/." "$dest/" || return 1
  E09_WRITTEN+=("$dest")
  # drop only this name's old fixtures (a shorter model list must not leave stale m{k} files behind)
  rm -f -- "$fixdir/anthropic_e09_${name}_ok.jsonl" "$fixdir/anthropic_e09_${name}_auth_error.jsonl"
  for f in "$fixdir/anthropic_e09_${name}_m"[0-9]*"_ok.jsonl"; do
    [ -e "$f" ] && rm -f -- "$f"
  done
  k=1
  while [ -d "$src/m$k" ]; do
    used="$(cat "$src/m$k/e09-used-a.txt" 2>/dev/null)"
    [ -n "$used" ] || used=a
    if [ "$k" = 1 ]; then fix="$fixdir/anthropic_e09_${name}_ok.jsonl"; else fix="$fixdir/anthropic_e09_${name}_m${k}_ok.jsonl"; fi
    if ! cp "$src/m$k/e09-$used.jsonl" "$fix"; then e09_unwrite; return 1; fi
    E09_WRITTEN+=("$fix")
    k=$((k + 1))
  done
  fix="$fixdir/anthropic_e09_${name}_auth_error.jsonl"
  if ! cp "$src/m1/e09-b.jsonl" "$fix"; then e09_unwrite; return 1; fi
  E09_WRITTEN+=("$fix")
  # last look at what was just written
  if ! e09_leak_check "$dest" || ! e09_leak_check "$fixdir"; then
    e09_unwrite
    return 3
  fi
  return 0
}

# e09_print_endpoint NAME COUNT : header + observation lines per model (already scrubbed).
e09_print_endpoint() {
  local name="$1" count="$2" k
  for k in $(seq 1 "$count"); do
    echo "endpoint=$name model=${PRINT_MODELS[$((k - 1))]}"
    cat "$OUT/$name/m$k/e09-verify.txt"
  done
}

# ---- probes ----------------------------------------------------------------------------------------------------
e09_use_endpoint "$E09_NAME" "$E09_KEY_ENV" "$E09_BASE_URL" "${PRIMARY_MODELS[@]}"
PRINT_MODELS=("${PRIMARY_MODELS[@]}")
PRIMARY_NAME="$E09_NAME"
PRIMARY_KEY_ENV="$E09_KEY_ENV"
PRIMARY_BASE_URL="$E09_BASE_URL"

e09_route_check
e09_probe_endpoint

if [ "$OFFLINE" = 1 ]; then
  # a second, fixed endpoint in the same invocation: proves the two never overwrite each other
  e09_use_endpoint "$E09_TEST_NAME" "$E09_TEST_KEY_ENV" "$PRIMARY_BASE_URL" m-one m-two
  e09_probe_endpoint
fi

# ---- leak gate over everything staged ---------------------------------------------------------------------------
if ! e09_leak_check "$OUT"; then
  echo "LỖI e09: phát hiện key trong kết quả — không chép fixture"
  rm -rf "$OUT"
  exit 3
fi

if [ "$OFFLINE" = 1 ]; then
  SIM="$WORK/repo-sim"
  mkdir -p "$SIM" || die "mkdir repo-sim"
  have=""
  for entry in "${E09_DONE[@]}"; do
    name="${entry%%:*}"
    count="${entry##*:}"
    for k in $(seq 1 "$count"); do
      V="$OUT/$name/m$k/e09-verify.txt"
      for want in a.stream_json_parses=yes a.tool_use_write=yes a.result_seen=yes a.result_is_error=no \
          a.usage_input_gt0=yes a.usage_output_gt0=yes a.file_ok=yes a.key_absent=yes; do
        grep -qxF -- "$want" "$V" || die "$name m$k: $want"
      done
      if [ "$k" = 1 ]; then
        for want in b.result_is_error=yes b.api_error_status=401; do
          grep -qxF -- "$want" "$V" || die "$name m$k: $want"
        done
      fi
    done
    # copy this endpoint into the simulated repo through the real copy function
    if [ "$name" = "$PRIMARY_NAME" ]; then
      e09_use_endpoint "$PRIMARY_NAME" "$PRIMARY_KEY_ENV" "$PRIMARY_BASE_URL" "${PRIMARY_MODELS[@]}"
    else
      e09_use_endpoint "$E09_TEST_NAME" "$E09_TEST_KEY_ENV" "$PRIMARY_BASE_URL" m-one m-two
    fi
    e09_copy_evidence "$SIM/spikes/e09/raw" "$SIM/tests/fixtures/stream" || die "copy $name vào repo-sim"
  done
  # both endpoints' raw directories and every expected fixture exist side by side
  nfix=0
  for entry in "${E09_DONE[@]}"; do
    name="${entry%%:*}"
    count="${entry##*:}"
    FIXD="$SIM/tests/fixtures/stream"
    for k in $(seq 1 "$count"); do
      [ -f "$SIM/spikes/e09/raw/$name/m$k/e09-a.jsonl" ] || die "thiếu raw/$name/m$k/e09-a.jsonl"
      if [ "$k" = 1 ]; then want_f="anthropic_e09_${name}_ok.jsonl"; else want_f="anthropic_e09_${name}_m${k}_ok.jsonl"; fi
      [ -s "$FIXD/$want_f" ] || die "thiếu fixture $want_f"
      nfix=$((nfix + 1))
    done
    [ -s "$SIM/spikes/e09/raw/$name/m1/e09-b.jsonl" ] || die "thiếu raw/$name/m1/e09-b.jsonl"
    [ -s "$FIXD/anthropic_e09_${name}_auth_error.jsonl" ] || die "thiếu fixture anthropic_e09_${name}_auth_error.jsonl"
    nfix=$((nfix + 1))
  done
  [ "$(find "$SIM/tests/fixtures/stream" -type f | wc -l)" -eq "$nfix" ] || die "số fixture trong repo-sim không khớp ($nfix)"
  e09_leak_check "$SIM" || die "key lọt vào repo-sim"
  # a copy attempted with a planted key inside $OUT writes nothing
  SIM2="$WORK/repo-sim-leak"
  mkdir -p "$SIM2" || die "mkdir repo-sim-leak"
  e09_use_endpoint "$PRIMARY_NAME" "$PRIMARY_KEY_ENV" "$PRIMARY_BASE_URL" "${PRIMARY_MODELS[@]}"
  printf 'x %s y\n' "${E09_LEAK_KEYS[0]}" > "$OUT/$PRIMARY_NAME/m1/planted.txt"
  e09_copy_evidence "$SIM2/spikes/e09/raw" "$SIM2/tests/fixtures/stream"
  [ "$?" -eq 3 ] || die "copy với key trong OUT phải trả 3"
  [ -z "$(find "$SIM2" -type f 2>/dev/null)" ] || die "copy bị chặn vẫn ghi file"
  rm -f "$OUT/$PRIMARY_NAME/m1/planted.txt"
  # the leak gate itself must detect a planted key (negative test)
  mkdir -p "$WORK/neg" || die "mkdir neg"
  printf 'x %s y\n' "${E09_LEAK_KEYS[0]}" > "$WORK/neg/planted.txt"
  if e09_leak_check "$WORK/neg"; then die "leak gate missed the planted key"; fi
  echo "E09 offline OK"
  exit 0
fi

# ---- real mode: copy the scrubbed evidence into the repo ---------------------------------------------------------
e09_copy_evidence "$HERE/raw" "$REPO/tests/fixtures/stream"
rc=$?
if [ "$rc" -eq 3 ]; then
  echo "LỖI e09: phát hiện key trong kết quả — không chép fixture"
  rm -rf "$OUT"
  exit 3
elif [ "$rc" -ne 0 ]; then
  die "copy evidence (mã $rc)"
fi

if ! bash "$REPO/spikes/lib/scan-secrets.sh"; then
  echo "LỖI e09: phát hiện key trong kết quả — không chép fixture"
  e09_unwrite
  exit 3
fi

e09_print_endpoint "$PRIMARY_NAME" "${#PRIMARY_MODELS[@]}"
echo "E09: xong — dán các dòng trên vào cuộc trò chuyện."
exit 0
