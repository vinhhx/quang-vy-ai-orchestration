#!/usr/bin/env bash
# Nghiệm thu Phase 6 (đội hình nhiều engine) bằng binary thật, engine giả, môi trường cô lập.
#
#   bash scripts/phase6-live-check.sh                    kiểm tra tự động, in "Phase 6 live check OK"
#   bash scripts/phase6-live-check.sh --keep             như trên, rồi giữ serve chạy để xem Cài đặt và
#                                                        một task đang dừng vì hết quota trong trình duyệt
#   bash scripts/phase6-live-check.sh --schema-dry-run   chỉ chạy thử nâng lược đồ v4 trên BẢN SAO của
#                                                        brain.db thật (không mở file thật bằng SQLite)
#
# Chế độ tự động: HOME, XDG_*, AGENT_BRAIN_DB, AGENT_BRAIN_CONFIG nằm trong một thư mục tạm; mọi engine
# là engine giả (claude, agy, dsh qua docker giả); hai key là giá trị cài sẵn. Không đụng tới
# ~/.config/agent-brain, brain.db thật, serve cổng 7437, ~/.claude-orca hay container dsh thật.
# Dừng serve bằng đúng PID đã ghi.
set -euo pipefail

SCRIPT_DIR="$(cd -P "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -P "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

KEEP=false
DRY_RUN=false
for arg in "$@"; do
  case "$arg" in
    --keep) KEEP=true ;;
    --schema-dry-run) DRY_RUN=true ;;
    *)
      echo "Tham số không hợp lệ: $arg (chỉ có --keep và --schema-dry-run)" >&2
      exit 2
      ;;
  esac
done

BIN="${AB_BIN:-$REPO_ROOT/agent-brain/target/debug/agent-brain}"
if [[ ! -x "$BIN" ]]; then
  echo "Thiếu $BIN — chạy: cargo build --manifest-path agent-brain/Cargo.toml --offline --locked" >&2
  exit 1
fi
for tool in python3 curl git; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "Thiếu công cụ $tool trong PATH" >&2
    exit 1
  fi
done

TMP="$(mktemp -d)"
SERVE_PID=""

cleanup() {
  local exit_code=$?
  if [[ -n "$SERVE_PID" ]]; then
    if kill -0 "$SERVE_PID" 2>/dev/null; then
      kill -INT "$SERVE_PID" 2>/dev/null || true
      local count=0
      while kill -0 "$SERVE_PID" 2>/dev/null && [[ $count -lt 100 ]]; do
        sleep 0.1
        count=$((count + 1))
      done
      if kill -0 "$SERVE_PID" 2>/dev/null; then
        kill -KILL "$SERVE_PID" 2>/dev/null || true
      fi
    fi
  fi
  if [[ -d "$TMP" ]]; then
    rm -rf "$TMP"
  fi
  exit "$exit_code"
}
trap cleanup EXIT INT TERM

# ---------------------------------------------------------------------------------------------
# --schema-dry-run: chạy thử nâng lược đồ trên bản sao. Đường dẫn thật chỉ được `ls`, `sha256sum`
# và `cp -p` đọc; không bao giờ đưa cho SQLite hay cho binary.
# ---------------------------------------------------------------------------------------------
if [[ "$DRY_RUN" == "true" ]]; then
  REAL_DB="${AGENT_BRAIN_DB:-$HOME/.local/share/agent-brain/brain.db}"
  if [[ ! -f "$REAL_DB" ]]; then
    echo "LỖI dry run: không thấy database: $REAL_DB" >&2
    exit 1
  fi
  REAL_DIR="$(dirname "$REAL_DB")"
  DB_NAME="$(basename "$REAL_DB")"
  echo "Dừng agent-brain serve và các run trước khi chạy — bản chép của một database đang được ghi có thể không nhất quán."

  # Danh sách thư mục và sha256 của thư mục dữ liệu thật. Dòng `..` (thư mục cha) không thuộc thư mục
  # dữ liệu nên không dùng để so sánh.
  snapshot_real() {
    {
      ls -la --time-style=full-iso "$REAL_DIR" | awk '$NF != ".."'
      for f in "$REAL_DB" "$REAL_DB-wal" "$REAL_DB-shm"; do
        if [[ -f "$f" ]]; then
          sha256sum "$f"
        fi
      done
    }
  }
  snapshot_real > "$TMP/before.txt"

  mkdir -p "$TMP/copy" "$TMP/home" "$TMP/xdg_config" "$TMP/xdg_data"
  COPY="$TMP/copy/$DB_NAME"
  cp -p "$REAL_DB" "$COPY"
  for suffix in -wal -shm; do
    if [[ -f "$REAL_DB$suffix" ]]; then
      cp -p "$REAL_DB$suffix" "$COPY$suffix"
    fi
  done

  # Đọc user_version, integrity_check và số dòng của BẢN SAO.
  read_copy() {
    python3 -I - "$COPY" << 'PYEOF'
import sqlite3
import sys

sys.stdout.reconfigure(encoding="utf-8")
c = sqlite3.connect(sys.argv[1])
print("user_version=%d" % c.execute("PRAGMA user_version").fetchone()[0])
print("integrity=%s" % c.execute("PRAGMA integrity_check").fetchone()[0])
for table in ("projects", "tasks", "runs", "events"):
    try:
        n = c.execute("SELECT COUNT(*) FROM %s" % table).fetchone()[0]
    except sqlite3.Error as e:
        print("LỖI dry run: không đọc được bảng %s trong bản sao (%s) — không phải brain.db của agent-brain?" % (table, e))
        sys.exit(1)
    print("%s=%d" % (table, n))
c.close()
PYEOF
  }
  if ! BEFORE_STATE="$(read_copy)"; then
    echo "$BEFORE_STATE"
    exit 1
  fi
  OLD_VERSION="$(printf '%s\n' "$BEFORE_STATE" | sed -n 's/^user_version=//p')"
  echo "Bản sao: $BEFORE_STATE" | tr '\n' ' '
  echo

  cp "$REPO_ROOT/agent-brain/assets/config.default.json" "$TMP/brain_config.json"
  # Mở bản sao bằng binary mới: lệnh `stats` chạy các bước nâng lược đồ trên bản sao đó.
  if ! HOME="$TMP/home" XDG_CONFIG_HOME="$TMP/xdg_config" XDG_DATA_HOME="$TMP/xdg_data" \
    AGENT_BRAIN_DB="$COPY" AGENT_BRAIN_CONFIG="$TMP/brain_config.json" \
    "$BIN" stats --json > /dev/null 2> "$TMP/stats.err"; then
    echo "LỖI dry run: binary mới không mở được bản sao" >&2
    head -c 2000 "$TMP/stats.err" >&2 || true
    exit 1
  fi

  if ! AFTER_STATE="$(read_copy)"; then
    echo "$AFTER_STATE"
    exit 1
  fi
  NEW_VERSION="$(printf '%s\n' "$AFTER_STATE" | sed -n 's/^user_version=//p')"
  if [[ "$NEW_VERSION" != "4" ]]; then
    echo "LỖI dry run: sau khi mở bằng binary mới user_version = $NEW_VERSION, cần 4" >&2
    exit 1
  fi
  if ! printf '%s\n' "$AFTER_STATE" | grep -qx 'integrity=ok'; then
    echo "LỖI dry run: PRAGMA integrity_check của bản sao đã nâng không phải ok" >&2
    exit 1
  fi
  for key in projects tasks runs events; do
    b="$(printf '%s\n' "$BEFORE_STATE" | sed -n "s/^$key=//p")"
    a="$(printf '%s\n' "$AFTER_STATE" | sed -n "s/^$key=//p")"
    if [[ "$a" != "$b" ]]; then
      echo "LỖI dry run: số dòng bảng $key đổi từ $b thành $a sau khi nâng" >&2
      exit 1
    fi
  done

  snapshot_real > "$TMP/after.txt"
  if ! diff -u "$TMP/before.txt" "$TMP/after.txt" > "$TMP/dir.diff"; then
    echo "LỖI dry run: thư mục dữ liệu thật đã thay đổi" >&2
    cat "$TMP/dir.diff" >&2
    exit 1
  fi

  echo "Schema v4 dry run OK (từ v$OLD_VERSION)"
  echo "real_dir_unchanged=yes"
  exit 0
fi

# ---------------------------------------------------------------------------------------------
# Chế độ tự động
# ---------------------------------------------------------------------------------------------
die() {
  echo "LỖI live: $*" >&2
  exit 1
}

export HOME="$TMP/home"
export XDG_CONFIG_HOME="$TMP/xdg_config"
export XDG_DATA_HOME="$TMP/xdg_data"
export AGENT_BRAIN_DB="$TMP/xdg_data/agent-brain/brain.db"
export AGENT_BRAIN_CONFIG="$TMP/brain_config.json"
export GIT_CONFIG_GLOBAL="$TMP/gitconfig"
export GIT_CONFIG_NOSYSTEM=1

mkdir -p "$TMP/home" "$TMP/xdg_config/agent-brain" "$TMP/xdg_data/agent-brain" \
  "$TMP/runs" "$TMP/personal_cfg" "$TMP/out" "$TMP/bin" "$TMP/trapbin" "$TMP/repos"

cat << 'EOF' > "$TMP/gitconfig"
[user]
  name = Live Check
  email = live-check@example.com
[commit]
  gpgsign = false
[init]
  defaultBranch = main
EOF

FAKES="$REPO_ROOT/tests/fakes"
PLANTED_GW_KEY="phase6-planted-gw-key-0123456789"
PLANTED_DS_KEY="phase6-planted-deepseek-key-0123456789"

# `dsh` giả cho docker giả; một `docker` bẫy đứng trước docker thật trong PATH của serve: nếu có gì gọi
# nó, nó ghi lại và thoát 97 (cuối script yêu cầu nhật ký bẫy phải rỗng).
ln -s "$FAKES/fake_dsh.py" "$TMP/bin/dsh"
cat << EOF > "$TMP/trapbin/docker"
#!/bin/sh
echo "docker \$*" >> "$TMP/docker-trap.log"
exit 97
EOF
chmod +x "$TMP/trapbin/docker"
: > "$TMP/docker-trap.log"

cat << EOF > "$TMP/brain_config.json"
{
  "claude_bin": "$FAKES/fake_claude.py",
  "models": {
    "lead": "opus",
    "dev": "sonnet",
    "qa": "sonnet",
    "plan-review": "opus",
    "code-review": "opus"
  },
  "inspect_model": "sonnet",
  "profiles": {
    "work": {
      "claude_config_dir": null,
      "unset_env": ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL"]
    },
    "personal": {
      "claude_config_dir": "$TMP/personal_cfg",
      "unset_env": ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL"]
    }
  },
  "common_args": ["--strict-mcp-config"],
  "max_turns": {"lead": null, "dev": null, "qa": null, "plan-review": null, "code-review": null},
  "timeout_seconds": {"lead": 30, "dev": 30, "qa": 30, "inspect": 30, "plan-review": 30, "code-review": 30},
  "runs_dir": "$TMP/runs",
  "kill_grace_secs": 1,
  "inspect_max_diff_bytes": 100000,
  "engines": [
    {
      "id": "gw",
      "kind": "anthropic",
      "label": "Gateway giả",
      "command": "$FAKES/fake_claude.py",
      "base_url": "https://gw.invalid/proxyllm",
      "key_env": "PHASE6_GW_KEY",
      "verified": "live-check giả",
      "models": {"dev": "deepseek/deepseek-chat"}
    },
    {
      "id": "deepseek",
      "kind": "anthropic",
      "label": "DeepSeek giả (chưa kiểm)",
      "command": "$FAKES/fake_claude.py",
      "base_url": "https://deepseek.invalid/anthropic",
      "key_env": "DEEPSEEK_API_KEY",
      "models": {"dev": "sonnet"}
    },
    {
      "id": "claude-max",
      "kind": "anthropic",
      "label": "Claude Max giả — chỉ dự án personal",
      "command": "$FAKES/fake_claude.py",
      "pin_profile": "personal",
      "models": {"lead": "opus", "dev": "opus"}
    },
    {
      "id": "agy",
      "kind": "agy",
      "label": "Antigravity giả",
      "command": "$FAKES/fake_agy.py"
    },
    {
      "id": "dsh",
      "kind": "dsh",
      "label": "dsh giả qua docker giả",
      "docker": "$FAKES/fake_docker.py",
      "container": "fake-dsh",
      "dsh_profile": "headless",
      "allowed_profiles": ["personal"]
    }
  ]
}
EOF

cat << 'EOF' > "$TMP/xdg_config/agent-brain/teams.json"
{
  "tiet-kiem": {
    "description": "Dev qua gateway đã khai báo (ghi đè đội hình có sẵn, D-43).",
    "roles": {"dev": "gw"}
  },
  "doi-dsh": {
    "description": "Dev trên dsh (hộp đen).",
    "roles": {"dev": "dsh"}
  },
  "doi-max": {
    "description": "Lead và Dev trên Claude Max, chỉ dự án personal.",
    "roles": {"lead": "claude-max", "dev": "claude-max"}
  }
}
EOF

# Hai repo nháp (git init + một commit): live-p6 (work) và live-p6-ca-nhan (personal).
make_repo() {
  local dir="$TMP/repos/$1"
  mkdir -p "$dir/src"
  printf 'def app():\n    return 1\n' > "$dir/src/app.py"
  git -C "$dir" init -q -b main
  git -C "$dir" add src/app.py
  git -C "$dir" commit -q -m init
}
make_repo live-p6
make_repo live-p6-ca-nhan

cat << EOF > "$TMP/xdg_config/agent-brain/projects_map.json"
{
  "scan_roots": [],
  "projects": [
    {
      "id": "live-p6",
      "path": "$TMP/repos/live-p6",
      "name_vi": "Live P6 (work)",
      "aliases": [],
      "type": "code",
      "repo": null,
      "profile": "work",
      "missing": false,
      "workflow": "nhanh",
      "team": "tiet-kiem"
    },
    {
      "id": "live-p6-ca-nhan",
      "path": "$TMP/repos/live-p6-ca-nhan",
      "name_vi": "Live P6 (personal)",
      "aliases": [],
      "type": "code",
      "repo": null,
      "profile": "personal",
      "missing": false,
      "workflow": "nhanh"
    }
  ]
}
EOF

# Kịch bản claude giả theo vai trò. Dev: [xong (task 1), hết quota (task 2), xong (task 2 chạy lại),
# hết quota rồi xong (task chỉ tạo khi --keep, để bấm chạy lại trong trình duyệt)]. Dsh: ghi src/app.py của task 3 rồi in hai dòng.
python3 -I - "$TMP" << 'PYEOF'
import json
import sys

tmp = sys.argv[1]


def init_event():
    return {"type": "system", "subtype": "init", "session_id": "live-p6", "model": "fake"}


def result_event(error, quota):
    ev = {
        "type": "result",
        "subtype": "success",
        "is_error": error,
        "num_turns": 1,
        "session_id": "live-p6",
        "total_cost_usd": 0,
        "usage": {
            "input_tokens": 0 if quota else 10,
            "output_tokens": 0 if quota else 5,
            "cache_read_input_tokens": 0,
            "cache_creation_input_tokens": 0,
        },
        "permission_denials": [],
    }
    if quota:
        ev["api_error_status"] = 429
        ev["terminal_reason"] = "blocking_limit"
        ev["result"] = "You've hit your session limit"
    return ev


def rate_limit_event():
    return {
        "type": "rate_limit_event",
        "rate_limit_info": {
            "status": "rejected",
            "resetsAt": 1792310400,
            "rateLimitType": "five_hour",
            "isUsingOverage": False,
            "overageStatus": "rejected",
        },
        "uuid": "u1",
        "session_id": "live-p6",
    }


def write_op(path, content):
    return {"op": "write", "path": path, "content": content}


plan_lines = [
    "# Plan: Kế hoạch",
    "## Mục tiêu",
    "Thực hiện Kế hoạch",
    "## Thay đổi",
    "| # | File | Dòng / hàm | Sửa gì | Lý do |",
    "|---|---|---|---|---|",
    "| 1 | src/app.py | app() | trả về 2 | demo |",
    "## Không được đụng tới",
    "",
    "## Cách kiểm tra",
    "python3 -m unittest",
    "## Vướng mắc (Dev ghi vào đây nếu có)",
    "",
    "",
]
lead = {
    "ops": [
        {"op": "read", "path": ".plan.md"},
        {"op": "read", "path": "src/app.py"},
        {"op": "write", "path": ".plan.md", "content": "\n".join(plan_lines), "keep_meta": True},
    ]
}


def dev_ok(text):
    return {
        "ops": [write_op("src/app.py", text)],
        "events": [init_event(), result_event(False, False)],
        "exit_code": 0,
    }


def dev_quota(text):
    return {
        "ops": [write_op("src/app.py", text)],
        "events": [init_event(), rate_limit_event(), result_event(True, True)],
        "exit_code": 1,
    }


qa_report = (
    "# QA: sửa app\n## Thay đổi có khớp plan không (từng mục)\n- src/app.py: đã xem.\n"
    "## Sửa ngoài plan (nếu có)\nKhông.\n## Rủi ro\nKhông.\n## Kết luận\nĐạt\n"
)
scenario = {
    "by_role": {
        "lead": [lead],
        "dev": [
            dev_ok("def app():\n    return 2\n"),
            dev_quota("phần 1"),
            dev_ok("def app():\n    return 3\n"),
            dev_quota("phần 1"),
            dev_ok("def app():\n    return 5\n"),
        ],
        "qa": [{"ops": [write_op(".qa-report.md", qa_report)]}],
    }
}
with open(tmp + "/fake_claude_scenario.json", "w", encoding="utf-8") as f:
    json.dump(scenario, f, ensure_ascii=False)

dsh = {
    "ops": [write_op("src/app.py", "def app():\n    return 4\n")],
    "lines": ["dsh: đọc plan", "dsh: xong"],
}
with open(tmp + "/fake_dsh_scenario.json", "w", encoding="utf-8") as f:
    json.dump(dsh, f, ensure_ascii=False)
PYEOF

# Môi trường của serve: engine giả, hai key cài sẵn (chỉ nằm trong môi trường, không ghi ra file).
export FAKE_CLAUDE_SCENARIO="$TMP/fake_claude_scenario.json"
export FAKE_CLAUDE_RECORD="$TMP/fake_claude_record.jsonl"
export FAKE_CLAUDE_EXPECT_TOKEN="$PLANTED_GW_KEY"
export FAKE_DSH_SCENARIO="$TMP/fake_dsh_scenario.json"
export FAKE_DSH_RECORD="$TMP/fake_dsh_record.jsonl"
export FAKE_DOCKER_RECORD="$TMP/fake_docker_record.jsonl"
export FAKE_DOCKER_STATUS="Up 1 minute"
export PHASE6_GW_KEY="$PLANTED_GW_KEY"
export DEEPSEEK_API_KEY="$PLANTED_DS_KEY"
export PATH="$TMP/bin:$TMP/trapbin:$PATH"

# serve trong nền; PID ghi lại để dừng đúng tiến trình này.
"$BIN" serve --port 0 > "$TMP/serve.out" 2> "$TMP/serve.err" < /dev/null &
SERVE_PID=$!

URL=""
for _ in {1..100}; do
  if [[ -f "$TMP/serve.out" ]]; then
    URL_LINE="$(grep "agent-brain dashboard: " "$TMP/serve.out" || true)"
    if [[ -n "$URL_LINE" ]]; then
      URL="${URL_LINE#agent-brain dashboard: }"
      break
    fi
  fi
  if ! kill -0 "$SERVE_PID" 2>/dev/null; then
    cat "$TMP/serve.err" >&2 || true
    die "serve dừng sớm (PID $SERVE_PID)"
  fi
  sleep 0.1
done
if [[ -z "$URL" ]]; then
  cat "$TMP/serve.err" >&2 || true
  die "không thấy URL của serve sau 10 giây"
fi

PORT="$(printf '%s' "$URL" | sed -E 's#^http://127\.0\.0\.1:([0-9]+)/.*#\1#')"
TOKEN="$(printf '%s' "$URL" | sed -E 's#.*[?]token=([^&# ]+).*#\1#')"
if [[ ! "$PORT" =~ ^[0-9]+$ || -z "$TOKEN" || "$TOKEN" == "$URL" ]]; then
  die "không đọc được cổng hoặc token từ dòng khởi động của serve"
fi
BASE="http://127.0.0.1:$PORT"

# Cookie của phiên đi vào curl qua stdin (-K -), không bao giờ nằm trên dòng lệnh.
# api_get TÊN ĐƯỜNG_DẪN → $TMP/out/TÊN.json và TÊN.code
api_get() {
  local name="$1" path="$2"
  printf 'header = "Cookie: agent_brain_token=%s"\n' "$TOKEN" |
    curl -sS -K - --max-time 30 \
      -o "$TMP/out/$name.json" \
      -w '%{http_code}' \
      "$BASE$path" > "$TMP/out/$name.code"
}

# api_post TÊN ĐƯỜNG_DẪN JSON: POST cần Origin trùng và Content-Type JSON.
api_post() {
  local name="$1" path="$2" body="$3"
  printf 'header = "Cookie: agent_brain_token=%s"\n' "$TOKEN" |
    curl -sS -K - --max-time 30 -X POST \
      -H "Origin: $BASE" -H 'Content-Type: application/json' \
      --data-binary "$body" \
      -o "$TMP/out/$name.json" \
      -w '%{http_code}' \
      "$BASE$path" > "$TMP/out/$name.code"
}

expect_code() {
  local name="$1" want="$2" got
  got="$(cat "$TMP/out/$name.code")"
  if [[ "$got" != "$want" ]]; then
    head -c 600 "$TMP/out/$name.json" >&2 || true
    echo >&2
    die "$name: HTTP $got, cần $want"
  fi
}

# jget TỆP KHOÁ.CHẤM.NGĂN.CÁCH → in một giá trị của JSON (chỉ số cho mảng).
jget() {
  python3 -I -c '
import json
import sys

d = json.load(open(sys.argv[1], encoding="utf-8"))
for k in sys.argv[2].split("."):
    d = d[int(k)] if isinstance(d, list) else d[k]
print(d)
' "$1" "$2"
}

# dbq SQL → in cột đầu của dòng đầu, chỉ đọc brain.db của môi trường tạm này.
dbq() {
  python3 -I -c '
import sqlite3
import sys

c = sqlite3.connect("file:" + sys.argv[1] + "?mode=ro", uri=True)
row = c.execute(sys.argv[2]).fetchone()
print("" if row is None or row[0] is None else row[0])
' "$AGENT_BRAIN_DB" "$1"
}

# wait_job TÊN JOB_ID: chờ job kết thúc (tối đa 120 giây), ghi $TMP/out/TÊN.json.
wait_job() {
  local name="$1" id="$2" n=0
  while ((n < 600)); do
    api_get "$name" "/api/jobs/$id"
    if [[ "$(cat "$TMP/out/$name.code")" == "200" && "$(jget "$TMP/out/$name.json" state)" == "exited" ]]; then
      return 0
    fi
    sleep 0.2
    n=$((n + 1))
  done
  die "job $id không kết thúc sau 120 giây"
}

# new_task TÊN DỰ_ÁN YÊU_CẦU ĐỘI → tạo task, chờ Lead xong, in id task.
new_task() {
  local name="$1" project="$2" request="$3" team="$4" body
  body="$(python3 -I -c '
import json
import sys

print(json.dumps({"project": sys.argv[1], "request": sys.argv[2], "workflow": "nhanh", "team": sys.argv[3]}, ensure_ascii=False))
' "$project" "$request" "$team")"
  api_post "${name}_create" /api/tasks "$body"
  expect_code "${name}_create" 202
  wait_job "${name}_lead" "$(jget "$TMP/out/${name}_create.json" job_id)"
  if [[ "$(jget "$TMP/out/${name}_lead.json" exit_code)" != "0" ]]; then
    die "$name: Lead không xong (exit $(jget "$TMP/out/${name}_lead.json" exit_code))"
  fi
  dbq "SELECT max(id) FROM tasks"
}

# step TÊN ID_TASK BƯỚC: chạy /dev hoặc /qa, chờ job, ghi mã thoát vào $TMP/out/TÊN.exit.
step() {
  local name="$1" id="$2" what="$3"
  api_post "${name}_start" "/api/tasks/$id/$what" '{}'
  expect_code "${name}_start" 202
  wait_job "${name}_job" "$(jget "$TMP/out/${name}_start.json" job_id)"
  jget "$TMP/out/${name}_job.json" exit_code > "$TMP/out/${name}.exit"
}

# (a) task 1: tiet-kiem trên gateway, Lead → Dev → QA.
T1="$(new_task t1 live-p6 "thêm dòng chào" tiet-kiem)"
step t1_dev "$T1" dev
step t1_qa "$T1" qa
api_get t1_task "/api/tasks/$T1"
expect_code t1_task 200
api_get stats /api/stats
expect_code stats 200

# (b) task 2: Dev trên gateway gặp hết quota, chạy lại bằng đội hình mac-dinh.
T2="$(new_task t2 live-p6 "sửa dòng hai" tiet-kiem)"
step t2_dev "$T2" dev
api_get t2_quota "/api/tasks/$T2"
expect_code t2_quota 200
REV="$(jget "$TMP/out/t2_quota.json" rev)"
api_post t2_rerun "/api/tasks/$T2/quota-rerun" "{\"team\": \"mac-dinh\", \"rev\": \"$REV\"}"
expect_code t2_rerun 202
wait_job t2_rerun_job "$(jget "$TMP/out/t2_rerun.json" job_id)"
jget "$TMP/out/t2_rerun_job.json" exit_code > "$TMP/out/t2_rerun.exit"
api_get t2_after "/api/tasks/$T2"
expect_code t2_after 200

# (c) task 3: Dev trên dsh (docker giả) ở dự án personal.
T3="$(new_task t3 live-p6-ca-nhan "việc nhỏ trên dsh" doi-dsh)"
step t3_dev "$T3" dev
api_get t3_task "/api/tasks/$T3"
expect_code t3_task 200

# (d) đội hình claude-max: bị từ chối ở dự án work, chạy ở dự án personal.
api_post max_work /api/tasks '{"project": "live-p6", "request": "việc claude max", "workflow": "nhanh", "team": "doi-max"}'
expect_code max_work 422
T4="$(new_task max_personal live-p6-ca-nhan "việc claude max" doi-max)"

# (e) danh sách engine và độ sẵn sàng.
api_get engines /api/engines
expect_code engines 200

cat << EOF > "$TMP/out/ids.json"
{"t1": $T1, "t2": $T2, "t3": $T3, "t4": $T4}
EOF

# Quét key: toàn bộ thư mục tạm (run dir, stream, stderr, brain.db, kết quả job, repo) cộng với bản dump
# SQL của brain.db và `git log -p` của hai repo nháp. Chỉ in có/không, không bao giờ in giá trị.
mkdir -p "$TMP/sweep"
python3 -I - "$AGENT_BRAIN_DB" "$TMP/sweep/brain.sql" << 'PYEOF'
import sqlite3
import sys

c = sqlite3.connect("file:" + sys.argv[1] + "?mode=ro", uri=True)
with open(sys.argv[2], "w", encoding="utf-8") as f:
    for line in c.iterdump():
        f.write(line + "\n")
PYEOF
for repo in live-p6 live-p6-ca-nhan; do
  git -C "$TMP/repos/$repo" log -p --all > "$TMP/sweep/$repo.gitlog.txt"
done
KEY_FAIL=0
for var in PHASE6_GW_KEY DEEPSEEK_API_KEY; do
  value="${!var}"
  hits="$(grep -rlF -e "$value" "$TMP" 2> /dev/null || true)"
  if [[ -z "$hits" ]]; then
    echo "key_absent[$var]=yes"
  else
    echo "key_absent[$var]=no"
    printf '%s\n' "$hits" | sed "s#^$TMP/##" | sed 's/^/  tệp chứa key: /'
    KEY_FAIL=1
  fi
done
if [[ "$KEY_FAIL" != "0" ]]; then
  die "key còn nằm trong tệp (xem danh sách trên)"
fi

if [[ -s "$TMP/docker-trap.log" ]]; then
  die "có lệnh gọi tới docker thật (nhật ký bẫy không rỗng)"
fi

python3 -I - "$TMP" "$AGENT_BRAIN_DB" << 'PYEOF'
import json
import os
import sqlite3
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8")
tmp, db_path = sys.argv[1], sys.argv[2]
out_dir = os.path.join(tmp, "out")


def fail(msg):
    print("LỖI live: " + msg)
    sys.exit(1)


def load(name):
    with open(os.path.join(out_dir, name + ".json"), encoding="utf-8") as f:
        return json.load(f)


def exit_of(name):
    with open(os.path.join(out_dir, name + ".exit"), encoding="utf-8") as f:
        return int(f.read().strip())


ids = load("ids")
db = sqlite3.connect("file:" + db_path + "?mode=ro", uri=True)


def runs_of(task_id):
    return db.execute(
        "SELECT id, role, engine, status, outcome, usage_source FROM runs WHERE task_id = ? ORDER BY id",
        (task_id,),
    ).fetchall()


# (a) task 1.
if load("t1_create")["team"] != "tiet-kiem":
    fail("task 1: team trả về %r, cần 'tiet-kiem'" % load("t1_create").get("team"))
for step_name in ("t1_dev", "t1_qa"):
    if exit_of(step_name) != 0:
        fail("%s: mã thoát %d, cần 0" % (step_name, exit_of(step_name)))
r1 = runs_of(ids["t1"])
got = [(r[1], r[2]) for r in r1]
if got != [("lead", "claude"), ("dev", "gw"), ("qa", "claude")]:
    fail("task 1: vai trò và engine của các run = %r, cần lead/claude, dev/gw, qa/claude" % (got,))
if any(r[3] != "succeeded" for r in r1):
    fail("task 1: có run không succeeded: %r" % (r1,))
print("LIVE OK: task 1 tiet-kiem: Lead claude, Dev gw, QA claude, cả ba succeeded")
stats = load("stats")
engine_rows = {row["engine"]: row for row in stats["engines"]}
if "claude" not in engine_rows or "gw" not in engine_rows:
    fail("/api/stats: các dòng engine = %r, cần có claude và gw" % sorted(engine_rows))
if engine_rows["gw"].get("usage") != "tokens":
    fail("/api/stats: dòng gw có usage %r, cần 'tokens'" % engine_rows["gw"].get("usage"))
print("LIVE OK: /api/stats có dòng engine claude và gw")

records = []
with open(os.path.join(tmp, "fake_claude_record.jsonl"), encoding="utf-8") as f:
    for line in f:
        if line.strip():
            records.append(json.loads(line))
if len(records) < 3:
    fail("nhật ký claude giả có %d bản ghi, cần ít nhất 3" % len(records))
lead_rec, dev_rec, qa_rec = records[0], records[1], records[2]
if dev_rec["anthropic_base_url"] != "https://gw.invalid/proxyllm" or dev_rec["auth_token_matches_expected"] is not True:
    fail("Dev trên gw: endpoint hoặc token không đúng (token khớp: %r)" % dev_rec["auth_token_matches_expected"])
for label, rec in (("Lead", lead_rec), ("QA", qa_rec)):
    if rec["anthropic_base_url"] is not None or rec["auth_token_matches_expected"] is not None:
        fail("%s trên claude có ANTHROPIC_BASE_URL hoặc token của gateway" % label)
    for var in ("PHASE6_GW_KEY", "DEEPSEEK_API_KEY"):
        if var in rec["env_names"]:
            fail("%s trên claude còn thấy biến %s" % (label, var))
if "DEEPSEEK_API_KEY" in dev_rec["env_names"]:
    fail("Dev trên gw còn thấy biến DEEPSEEK_API_KEY của engine khác")
print("LIVE OK: Dev nhận endpoint và key của gw; Lead và QA không thấy biến key nào")

# (b) task 2: hết quota rồi chạy lại.
if exit_of("t2_dev") != 5:
    fail("task 2: bước Dev trên gw thoát %d, cần 5 (hết quota)" % exit_of("t2_dev"))
quota = load("t2_quota")["quota"]
if not quota or quota.get("engine") != "gw":
    fail("task 2: quota = %r, cần quota.engine == 'gw'" % (quota,))
print("LIVE OK: task 2: Dev trên gw dừng vì hết quota, quota.engine = gw")
if exit_of("t2_rerun") != 0:
    fail("task 2: chạy lại thoát %d, cần 0" % exit_of("t2_rerun"))
r2 = runs_of(ids["t2"])
if [(r[1], r[2]) for r in r2] != [("lead", "claude"), ("dev", "gw"), ("dev", "claude")]:
    fail("task 2: run = %r, cần lead/claude, dev/gw, dev/claude" % ([(r[1], r[2]) for r in r2],))
if r2[1][4] != "quota" or r2[2][3] != "succeeded":
    fail("task 2: run dev đầu cần outcome quota, run chạy lại cần succeeded: %r" % (r2,))
if r2[2][0] != r2[1][0] + 1:
    fail("task 2: có run chen giữa lần dừng vì quota và lần chạy lại (id %d rồi %d)" % (r2[1][0], r2[2][0]))
after = load("t2_after")
history = after["team_history"]
if len(history) != 1 or (history[0].get("from"), history[0].get("to"), history[0].get("by")) != ("tiet-kiem", "mac-dinh", "dashboard"):
    fail("task 2: team_history = %r, cần một dòng tiet-kiem → mac-dinh bởi dashboard" % (history,))
if after["team"]["name"] != "mac-dinh" or after["quota"] is not None:
    fail("task 2: sau khi chạy lại team = %r, quota = %r" % (after["team"]["name"], after["quota"]))
print("LIVE OK: task 2: chạy lại bằng mac-dinh, Dev trên claude xong, một dòng lịch sử, không có run chen giữa")

# (c) task 3: dsh.
if exit_of("t3_dev") != 0:
    fail("task 3: Dev trên dsh thoát %d, cần 0" % exit_of("t3_dev"))
r3 = runs_of(ids["t3"])
dev3 = [r for r in r3 if r[1] == "dev"]
if len(dev3) != 1 or dev3[0][2] != "dsh":
    fail("task 3: run dev = %r, cần một run engine dsh" % (dev3,))
if dev3[0][5] != "none":
    fail("task 3: usage_source = %r, cần 'none'" % dev3[0][5])
if dev3[0][3] != "succeeded" or dev3[0][4] == "dsh:empty":
    fail("task 3: run dsh có status %r, outcome %r" % (dev3[0][3], dev3[0][4]))
repo3 = os.path.join(tmp, "repos", "live-p6-ca-nhan")
branch3 = db.execute("SELECT branch FROM tasks WHERE id = ?", (ids["t3"],)).fetchone()[0]
log3 = subprocess.run(
    ["git", "-C", repo3, "log", "--format=%s", branch3, "--", "src/app.py"],
    capture_output=True,
    text=True,
    check=True,
).stdout.split("\n")
if len([s for s in log3 if s.strip()]) < 2:
    fail("task 3: không có commit Dev sửa src/app.py trên nhánh %s" % branch3)
shown = subprocess.run(
    ["git", "-C", repo3, "show", branch3 + ":src/app.py"], capture_output=True, text=True, check=True
).stdout
if "return 4" not in shown:
    fail("task 3: src/app.py trên nhánh %s không mang thay đổi của dsh" % branch3)
print("LIVE OK: task 3: Dev trên dsh (docker giả), usage_source none, thay đổi đã commit, outcome %s" % (dev3[0][4] or "không có"))

# (d) claude-max.
err = load("max_work").get("error", "")
if "chỉ dùng cho dự án profile personal" not in err:
    fail("claude-max ở dự án work: lỗi = %r, cần chứa 'chỉ dùng cho dự án profile personal'" % err)
print("LIVE OK: doi-max ở dự án work bị từ chối (422) với lý do profile personal")
r4 = runs_of(ids["t4"])
lead4 = [r for r in r4 if r[1] == "lead"]
if len(lead4) != 1 or lead4[0][2] != "claude-max":
    fail("task 4: run lead = %r, cần engine claude-max" % (lead4,))
personal_dir = os.path.join(tmp, "personal_cfg")
if records[-1]["claude_config_dir"] != personal_dir:
    fail("task 4: Lead claude-max chạy với claude_config_dir khác thư mục profile personal")
if lead_rec["claude_config_dir"] is not None:
    fail("task 1: Lead ở dự án work có claude_config_dir %r, cần rỗng" % lead_rec["claude_config_dir"])
print("LIVE OK: doi-max ở dự án personal chạy (202), Lead dùng thư mục profile personal của môi trường tạm")

# (e) readiness.
eng = load("engines")["engines"]
names = [e["name"] for e in eng]
want_names = ["claude", "gw", "deepseek", "claude-max", "agy", "dsh", "người"]
if names != want_names:
    fail("/api/engines: thứ tự = %r, cần %r" % (names, want_names))
readiness = {e["name"]: e["readiness"] for e in eng}
want_ready = {"gw": "san_sang", "deepseek": "chua_kiem", "claude-max": "san_sang", "dsh": "san_sang"}
for name, want in want_ready.items():
    if readiness[name] != want:
        fail("/api/engines: %s = %r, cần %r" % (name, readiness[name], want))
print("LIVE OK: /api/engines: thứ tự %s; gw, claude-max, dsh san_sang, deepseek chua_kiem" % ", ".join(names))
PYEOF

echo "Phase 6 live check OK"

if [[ "$KEEP" == "true" ]]; then
  # Một task nữa dừng ở trạng thái hết quota để xem banner trong trình duyệt (queue Dev: entry thứ tư).
  T5="$(new_task keep live-p6 "việc để xem hết quota" tiet-kiem)"
  step keep_dev "$T5" dev
  echo "Task $T5 đang dừng vì hết quota: ${URL}#/task/$T5"
  echo "${URL}#/settings"
  echo "Mở URL trên trong trình duyệt; Ctrl+C để dừng serve."
  wait "$SERVE_PID" || true
fi
