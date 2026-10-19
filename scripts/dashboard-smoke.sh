#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -P "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -P "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

# Screens to check
ENABLE_KEYS=false
SCREENS=()
for arg in "$@"; do
  if [[ "$arg" == "--keys" ]]; then
    ENABLE_KEYS=true
  else
    SCREENS+=("$arg")
  fi
done

if [[ ${#SCREENS[@]} -eq 0 ]]; then
  SCREENS=("overview" "assign")
fi

BIN="${AB_BIN:-$REPO_ROOT/agent-brain/target/debug/agent-brain}"
if [[ ! -x "$BIN" ]]; then
  if [[ -x "$REPO_ROOT/$BIN" ]]; then
    BIN="$REPO_ROOT/$BIN"
  else
    echo "Thiếu $BIN — chạy: cargo build --manifest-path agent-brain/Cargo.toml --offline --locked" >&2
    exit 1
  fi
fi

CHROME_BIN=""
for c in google-chrome chromium chromium-browser; do
  if command -v "$c" >/dev/null 2>&1; then
    CHROME_BIN="$(command -v "$c")"
    break
  fi
done

if [[ -z "$CHROME_BIN" ]]; then
  echo "Không có Chrome/Chromium headless" >&2
  exit 1
fi

TMP="$(mktemp -d)"
SERVE_PID=""
CHROME_PID=""

stop_chrome() {
  if [[ -n "$CHROME_PID" ]]; then
    if kill -0 "$CHROME_PID" 2>/dev/null; then
      kill -TERM "$CHROME_PID" 2>/dev/null || true
      local count=0
      while kill -0 "$CHROME_PID" 2>/dev/null && [[ $count -lt 50 ]]; do
        sleep 0.1
        count=$((count + 1))
      done
      if kill -0 "$CHROME_PID" 2>/dev/null; then
        kill -KILL "$CHROME_PID" 2>/dev/null || true
      fi
    fi
    CHROME_PID=""
  fi
}

cleanup() {
  local exit_code=$?
  stop_chrome
  if [[ -n "$SERVE_PID" ]]; then
    if kill -0 "$SERVE_PID" 2>/dev/null; then
      kill -INT "$SERVE_PID" 2>/dev/null || true
      local count=0
      while kill -0 "$SERVE_PID" 2>/dev/null && [[ $count -lt 100 ]]; do
        sleep 0.1
        count=$((count + 1))
      done
      if kill -0 "$SERVE_PID" 2>/dev/null; then
        kill -9 "$SERVE_PID" 2>/dev/null || true
      fi
    fi
  fi
  if [[ -d "$TMP" ]]; then
    rm -rf "$TMP"
  fi
  exit "$exit_code"
}
trap cleanup EXIT INT TERM

export HOME="$TMP/home"
export XDG_CONFIG_HOME="$TMP/xdg_config"
export XDG_DATA_HOME="$TMP/xdg_data"
export AGENT_BRAIN_DB="$TMP/xdg_data/agent-brain/brain.db"
export AGENT_BRAIN_CONFIG="$TMP/brain_config.json"
export GIT_CONFIG_GLOBAL="$TMP/gitconfig"
export GIT_CONFIG_NOSYSTEM=1
export FAKE_CLAUDE_SCENARIO="$TMP/fake_claude_scenario.json"
export FAKE_CLAUDE_RECORD="$TMP/fake_claude_record.jsonl"
export CLAUDECODE=1
export CLAUDE_CODE_ENTRYPOINT=cli

mkdir -p "$TMP/home" "$TMP/xdg_config/agent-brain" "$TMP/xdg_data/agent-brain" "$TMP/runs" "$TMP/personal_cfg" "$TMP/repos"

cat << 'EOF' > "$TMP/gitconfig"
[user]
  name = Smoke User
  email = smoke@example.com
[commit]
  gpgsign = false
[init]
  defaultBranch = main
EOF

REPO_DIR="$TMP/repos/web-ban-hang"
mkdir -p "$REPO_DIR/src"
(
  cd "$REPO_DIR"
  git init -b main >/dev/null 2>&1
  echo "# Web ban hang" > README.md
  printf 'def app():\n    return 1\n' > src/app.py
  git add -A
  git commit -m "init" >/dev/null 2>&1
)

cat << EOF > "$TMP/xdg_config/agent-brain/projects_map.json"
{
  "scan_roots": [],
  "projects": [
    {
      "id": "web-ban-hang",
      "path": "$REPO_DIR",
      "name_vi": "Web bán hàng",
      "aliases": ["shop"],
      "type": "code",
      "repo": null,
      "profile": "work",
      "missing": false
    },
    {
      "id": "web-ban-hang-cu",
      "path": "$TMP/repos/khong-ton-tai",
      "name_vi": "Web bán hàng cũ",
      "aliases": ["shop-cu"],
      "type": "code",
      "repo": null,
      "profile": "work",
      "missing": true
    }
  ]
}
EOF

cat << EOF > "$TMP/brain_config.json"
{
  "claude_bin": "$REPO_ROOT/tests/fakes/fake_claude.py",
  "models": {
    "lead": "opus",
    "dev": "sonnet",
    "qa": "sonnet"
  },
  "inspect_model": "sonnet",
  "profiles": {
    "work": {
      "claude_config_dir": null,
      "unset_env": [
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "ANTHROPIC_BASE_URL"
      ]
    },
    "personal": {
      "claude_config_dir": "$TMP/personal_cfg",
      "unset_env": [
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "ANTHROPIC_BASE_URL"
      ]
    }
  },
  "common_args": ["--strict-mcp-config"],
  "max_turns": {
    "lead": null,
    "dev": null,
    "qa": null
  },
  "timeout_seconds": {
    "lead": 30,
    "dev": 30,
    "qa": 30,
    "inspect": 30
  },
  "runs_dir": "$TMP/runs",
  "kill_grace_secs": 1,
  "inspect_max_diff_bytes": 100000
}
EOF

chmod 755 "$REPO_ROOT/tests/fakes/fake_claude.py"

cat << 'EOF' > "$TMP/fake_claude_scenario.json"
{
  "ops": [
    {
      "op": "read",
      "path": ".plan.md"
    },
    {
      "op": "read",
      "path": "src/app.py"
    },
    {
      "op": "write",
      "path": ".plan.md",
      "content": "# Plan: Kế hoạch\n## Mục tiêu\nThực hiện Kế hoạch\n## Thay đổi\n| # | File | Dòng / hàm | Sửa gì | Lý do |\n|---|---|---|---|---|\n| 1 | src/app.py | toàn bộ | sửa app | yêu cầu |\n## Không được đụng tới\n\n## Cách kiểm tra\npython3 -m unittest\n## Vướng mắc (Dev ghi vào đây nếu có)\n\n",
      "keep_meta": true
    }
  ]
}
EOF

# Run lead to plan task 1
"$BIN" run lead -- web-ban-hang "sửa app" >/dev/null 2>&1

# Start serve
"$BIN" serve --port 0 > "$TMP/serve.out" 2> "$TMP/serve.err" &
SERVE_PID=$!

# Wait up to 10s for the URL line
URL=""
for i in {1..100}; do
  if [[ -f "$TMP/serve.out" ]]; then
    URL_LINE="$(grep "agent-brain dashboard: " "$TMP/serve.out" || true)"
    if [[ -n "$URL_LINE" ]]; then
      URL="$(echo "$URL_LINE" | sed 's/agent-brain dashboard: //')"
      break
    fi
  fi
  if ! kill -0 "$SERVE_PID" 2>/dev/null; then
    echo "Lỗi: serve tiến trình dừng sớm (PID $SERVE_PID)" >&2
    if [[ -f "$TMP/serve.err" ]]; then
      cat "$TMP/serve.err" >&2
    fi
    exit 1
  fi
  sleep 0.1
done

if [[ -z "$URL" ]]; then
  echo "Lỗi: Không tìm thấy URL của serve sau 10 giây" >&2
  if [[ -f "$TMP/serve.err" ]]; then
    cat "$TMP/serve.err" >&2
  fi
  exit 1
fi

for s in "${SCREENS[@]}"; do
  case "$s" in
    overview) HASH="#/" ;;
    assign) HASH="#/assign" ;;
    task) HASH="#/task/1" ;;
    stats) HASH="#/stats" ;;
    projects) HASH="#/projects" ;;
    *)
      echo "Lỗi: Màn hình không xác định: $s" >&2
      exit 1
      ;;
  esac

  TARGET_URL="${URL}${HASH}"
  DOM_OUT="$TMP/dom-$s.html"

  "$CHROME_BIN" \
    --headless=new \
    --no-sandbox \
    --disable-gpu \
    --no-first-run \
    --disable-background-networking \
    --disable-component-update \
    --disable-sync \
    --user-data-dir="$TMP/chrome-$s" \
    --virtual-time-budget=6000 \
    --dump-dom "$TARGET_URL" > "$DOM_OUT" 2>/dev/null || true

  if ! grep -q "data-screen=\"$s\"" "$DOM_OUT" || ! grep -q 'data-loaded="yes"' "$DOM_OUT"; then
    echo "Lỗi: Màn hình $s không render thành công (thiếu data-screen=\"$s\" hoặc data-loaded=\"yes\")" >&2
    echo "--- 40 dòng đầu của DOM ---" >&2
    head -n 40 "$DOM_OUT" >&2 || true
    echo "--- serve stderr ---" >&2
    if [[ -f "$TMP/serve.err" ]]; then
      cat "$TMP/serve.err" >&2
    fi
    exit 1
  fi
done

echo "Smoke OK: ${SCREENS[*]}"

if [[ "$ENABLE_KEYS" == "true" ]]; then
  if ! node -e 'process.exit(typeof WebSocket==="function"?0:1)' 2>/dev/null; then
    echo "Cần Node ≥ 22 (có WebSocket) cho --keys" >&2
    exit 1
  fi

  "$CHROME_BIN" \
    --headless=new \
    --no-sandbox \
    --disable-gpu \
    --no-first-run \
    --disable-background-networking \
    --disable-component-update \
    --disable-sync \
    --remote-debugging-port=0 \
    --user-data-dir="$TMP/chrome-keys" \
    --window-size=1280,1000 \
    about:blank >/dev/null 2>&1 &
  CHROME_PID=$!

  PORT=""
  for i in {1..100}; do
    if [[ -f "$TMP/chrome-keys/DevToolsActivePort" ]]; then
      PORT="$(head -n 1 "$TMP/chrome-keys/DevToolsActivePort" | tr -d '\r\n')"
      if [[ -n "$PORT" ]]; then
        break
      fi
    fi
    if ! kill -0 "$CHROME_PID" 2>/dev/null; then
      echo "Lỗi: Chrome dừng sớm (PID $CHROME_PID)" >&2
      exit 1
    fi
    sleep 0.1
  done

  if [[ -z "$PORT" ]]; then
    echo "Lỗi: Không tìm thấy DevToolsActivePort sau 10 giây" >&2
    exit 1
  fi

  if ! node "$REPO_ROOT/scripts/dashboard-keys.mjs" "$PORT" "$URL" > "$TMP/keys.out" 2>&1; then
    if [[ -f "$TMP/keys.out" ]]; then
      cat "$TMP/keys.out" >&2
    fi
    if [[ -f "$TMP/serve.err" ]]; then
      cat "$TMP/serve.err" >&2
    fi
    exit 1
  fi

  stop_chrome
  echo "Keys OK: combobox, confirm-dialog, raw-dialog"
fi
