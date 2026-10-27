#!/usr/bin/env bash
# E-09 - DeepSeek through the claude CLI (Phase 6, D-01..D-03). Re-runnable: bash spikes/e09/run.sh
#
# Real mode (the user runs it, with their own key exported only in that shell):
#   export DEEPSEEK_API_KEY=...   # never written to a file, never on argv
#   bash spikes/e09/run.sh
#   Probe A: one tiny real `claude -p` call through ANTHROPIC_BASE_URL (DeepSeek's Anthropic-format endpoint).
#   Probe B: the same call with the fixed invalid token (no billing) to capture a real auth-error stream.
#   Probe C (optional): E09_EXTRA_MODEL=<explicit DeepSeek model id> runs probe A again with that model.
#   Probe A is repeated once (prefix e09-a2) with CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1 and
#   CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING=1 only if it fails.
#   Env knobs: E09_CLAUDE_BIN (default claude), E09_BASE_URL, E09_MODEL (default sonnet), E09_EXTRA_MODEL.
#
# Offline self-test (no network, no key, no repo file touched):
#   E09_OFFLINE=1 bash spikes/e09/run.sh   -> prints "E09 offline OK"
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
E09_MODEL="${E09_MODEL:-sonnet}"
E09_EXTRA_MODEL="${E09_EXTRA_MODEL:-}"
E09_INVALID_TOKEN=invalid-key-for-e09-probe
OFFLINE="${E09_OFFLINE:-0}"
E09_CWD=""

die() { echo "LỖI e09: $*"; exit 1; }

if [ "$OFFLINE" = 1 ]; then
  E09_CLAUDE_BIN="$REPO/tests/fakes/fake_claude.py"
  KEY="e09-offline-planted-0123456789"
  export DEEPSEEK_API_KEY="$KEY"
  WORK="$(mktemp -d)" || die "mktemp"
  trap 'rm -rf "$WORK"' EXIT
else
  KEY="${DEEPSEEK_API_KEY:-}"
  if [ -z "$KEY" ]; then
    echo "Thiếu DEEPSEEK_API_KEY"
    exit 2
  fi
  WORK="$SPIKE_WORK_ROOT/e09/$(date -u +%Y%m%dT%H%M%SZ)"
  mkdir -p "$WORK" || die "mkdir $WORK"
fi
OUT="$WORK/out"
mkdir -p "$OUT" "$WORK/scn" || die "mkdir $OUT"

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

# e09_leak_check DIR : returns 0 when the key is NOT found anywhere under DIR (status only, never prints the key).
e09_leak_check() {
  if grep -rqF -- "$KEY" "$1" 2>/dev/null; then return 1; fi
  return 0
}

# e09_probe_ok NAME ARGS... : probe-A-shaped call (asks claude to create e09.txt) in a fresh scratch repo.
e09_probe_ok() {
  local name="$1"
  shift
  local d="$WORK/repo-$name"
  e09_mkrepo "$d" || die "git init $d"
  E09_CWD="$d"
  e09_use_scenario a
  e09_call "$OUT/e09-$name" DEEPSEEK_API_KEY "${E09_ARGS[@]}" "$@"
  e09_file_check "$d" "$OUT/e09-$name.file"
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

# ---- probes ----------------------------------------------------------------------------------------------------
echo "E09: probe A (gọi thật qua $E09_BASE_URL, model $E09_MODEL)"
e09_probe_ok a --model "$E09_MODEL"
USED_A=a
if bash "$HERE/verify.sh" "$OUT" | grep -qE '^a\.(result_seen=no|result_is_error=yes)$'; then
  echo "E09: probe A lỗi, thử lại một lần với CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1 và CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING=1"
  E09_RETRY=1 e09_probe_ok a2 --model "$E09_MODEL"
  USED_A=a2
fi

echo "E09: probe B (token sai cố định, không tính phí)"
e09_mkrepo "$WORK/repo-b" || die "git init repo-b"
E09_CWD="$WORK/repo-b"
e09_use_scenario b
e09_call "$OUT/e09-b" E09_INVALID_TOKEN "${E09_ARGS[@]}" --model "$E09_MODEL"

if [ -n "$E09_EXTRA_MODEL" ]; then
  echo "E09: probe C (model tường minh: $E09_EXTRA_MODEL)"
  e09_probe_ok c --model "$E09_EXTRA_MODEL"
fi

# ---- scrub, observe, leak gate ----------------------------------------------------------------------------------
scrub_raw "$OUT" >/dev/null
bash "$HERE/verify.sh" "$OUT" > "$OUT/e09-verify.txt"
scrub_raw "$OUT/e09-verify.txt" >/dev/null

if ! e09_leak_check "$OUT"; then
  echo "LỖI e09: phát hiện key trong kết quả — không chép fixture"
  rm -rf "$OUT"
  exit 3
fi

if [ "$OFFLINE" = 1 ]; then
  V="$OUT/e09-verify.txt"
  for want in a.stream_json_parses=yes a.tool_use_write=yes a.result_seen=yes a.result_is_error=no \
      a.usage_input_gt0=yes a.usage_output_gt0=yes a.file_ok=yes a.key_absent=yes \
      b.result_is_error=yes b.api_error_status=401; do
    grep -qxF -- "$want" "$V" || die "$want"
  done
  # the leak gate itself must detect a planted key (negative test)
  mkdir -p "$WORK/neg" || die "mkdir neg"
  printf 'x %s y\n' "$KEY" > "$WORK/neg/planted.txt"
  if e09_leak_check "$WORK/neg"; then die "leak gate missed the planted key"; fi
  echo "E09 offline OK"
  exit 0
fi

# ---- real mode: copy the scrubbed evidence into the repo ---------------------------------------------------------
RAW="$HERE/raw"
FIX="$REPO/tests/fixtures/stream"
mkdir -p "$RAW" "$FIX" || die "mkdir raw/fixtures"
rm -f "$RAW"/e09-*
cp "$OUT"/e09-* "$RAW"/ || die "copy raw"

if ! bash "$REPO/spikes/lib/scan-secrets.sh"; then
  echo "LỖI e09: phát hiện key trong kết quả — không chép fixture"
  rm -f "$RAW"/e09-*
  exit 3
fi

cp "$RAW/e09-$USED_A.jsonl" "$FIX/deepseek_e09_ok.jsonl" || die "copy ok fixture"
cp "$RAW/e09-b.jsonl" "$FIX/deepseek_e09_auth_error.jsonl" || die "copy auth_error fixture"
if grep -qF -- "$KEY" "$FIX/deepseek_e09_ok.jsonl" "$FIX/deepseek_e09_auth_error.jsonl"; then
  echo "LỖI e09: phát hiện key trong kết quả — không chép fixture"
  rm -f "$RAW"/e09-* "$FIX/deepseek_e09_ok.jsonl" "$FIX/deepseek_e09_auth_error.jsonl"
  exit 3
fi

cat "$RAW/e09-verify.txt"
echo "E09: xong — dán các dòng trên vào cuộc trò chuyện."
exit 0
