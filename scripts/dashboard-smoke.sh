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
# 06-24: two planted gateway keys; the serve process reads them through the key variables named
# in config.json, and no rendered page may ever contain their values.
export SMOKE_GW_KEY="smoke-planted-gw-key-7f3a91c20b"
export SMOKE_GW2_KEY="smoke-planted-gw2-key-5d8e44a1c7"

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
  # GSD fixture for the work board (#/board): epic 02 is the default view.
  mkdir -p .planning/phases/01-nen-tang .planning/phases/02-xuat-bao-cao
  cat << 'ROADMAP_EOF' > .planning/ROADMAP.md
# Roadmap: Web bán hàng

## Phases

- [x] **Phase 1: Nền tảng** - định dạng và truy vấn
- [ ] **Phase 2: Xuất báo cáo** - xuất CSV
- [ ] **Phase 3: Hướng dẫn** - tài liệu cho cửa hàng

## Phase Details

### Phase 1: Nền tảng
**Depends on**: Nothing (first phase)
Plans:
- [x] 01-01-PLAN.md — Chốt định dạng CSV
- [x] 01-02-PLAN.md — Truy vấn đơn hàng theo khoảng ngày

### Phase 2: Xuất báo cáo
**Depends on**: Phase 1
Plans:
- [ ] 02-01-PLAN.md — Bộ ghi CSV theo luồng
- [ ] 02-02-PLAN.md — API xuất đơn hàng
- [ ] 02-03-PLAN.md — Nút Xuất CSV ở trang đơn hàng

### Phase 3: Hướng dẫn
**Depends on**: Nothing (làm song song)
Plans: TBD
ROADMAP_EOF
  cat << 'STATE_EOF' > .planning/STATE.md
---
current_phase: "02"
current_phase_name: Xuất báo cáo
---

# Project State
STATE_EOF
  smoke_plan() {
    # smoke_plan FILE PHASE_DIR PLAN WAVE DEPENDS_ON
    cat << PLAN_EOF > "$1"
---
phase: $2
plan: $3
type: execute
wave: $4
depends_on: $5
files_modified:
  - src/app.py
autonomous: true
requirements: [SMK-01]
must_haves:
  truths:
    - "Smoke fixture plan $2-$3"
---

<objective>
Smoke fixture plan $2-$3 for the work board.
</objective>
PLAN_EOF
  }
  smoke_plan .planning/phases/01-nen-tang/01-01-PLAN.md 01-nen-tang 01 1 '[]'
  printf '# Summary\n' > .planning/phases/01-nen-tang/01-01-SUMMARY.md
  smoke_plan .planning/phases/01-nen-tang/01-02-PLAN.md 01-nen-tang 02 2 '[01-01]'
  printf '# Summary\n' > .planning/phases/01-nen-tang/01-02-SUMMARY.md
  smoke_plan .planning/phases/02-xuat-bao-cao/02-01-PLAN.md 02-xuat-bao-cao 01 1 '["01-02"]'
  smoke_plan .planning/phases/02-xuat-bao-cao/02-02-PLAN.md 02-xuat-bao-cao 02 2 '[02-01]'
  smoke_plan .planning/phases/02-xuat-bao-cao/02-03-PLAN.md 02-xuat-bao-cao 03 2 '[02-01]'
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

# Cài đặt fixtures (06-08): one custom team and one staff entry under the private config dir.
cat << 'EOF' > "$TMP/xdg_config/agent-brain/teams.json"
{
  "doi-thu": {
    "description": "Đội hình thử cho smoke.",
    "roles": {
      "lead": "claude",
      "dev": "deepseek",
      "qa": "claude",
      "plan-review": "claude",
      "code-review": "claude"
    }
  },
  "doi-nguoi": {
    "description": "Đội hình có người làm bước Code (smoke 06-18).",
    "roles": {
      "dev": "người:ha"
    }
  },
  "thu-cong": {
    "description": "Đội hình có Dev chạy endpoint chưa có bằng chứng E-09 (smoke 06-24).",
    "roles": {
      "dev": "gw-chua"
    }
  }
}
EOF
cat << 'EOF' > "$TMP/xdg_config/agent-brain/staff.json"
{
  "ha": {
    "display_name": "Hà"
  }
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
  "inspect_max_diff_bytes": 100000,
  "engines": [
    {
      "id": "deepseek",
      "kind": "anthropic",
      "command": "$REPO_ROOT/tests/fakes/fake_claude.py",
      "base_url": "https://deepseek.invalid/anthropic",
      "key_env": "SMOKE_DEEPSEEK_KEY",
      "models": {"dev": "sonnet"}
    },
    {
      "id": "gw-thu",
      "kind": "anthropic",
      "label": "Cổng thử nghiệm của smoke",
      "command": "$REPO_ROOT/tests/fakes/fake_claude.py",
      "base_url": "https://gw-thu.invalid/anthropic",
      "key_env": "SMOKE_GW_KEY",
      "models": {"dev": "sonnet"},
      "verified": "2026-10-28 smoke E-09"
    },
    {
      "id": "gw-chua",
      "kind": "anthropic",
      "command": "$REPO_ROOT/tests/fakes/fake_claude.py",
      "base_url": "https://gw-chua.invalid/anthropic",
      "key_env": "SMOKE_GW2_KEY",
      "models": {"dev": "sonnet"}
    },
    {
      "id": "claude-max",
      "kind": "anthropic",
      "command": "$REPO_ROOT/tests/fakes/fake_claude.py",
      "pin_profile": "personal",
      "models": {"dev": "opus"}
    },
    {
      "id": "agy",
      "kind": "agy",
      "command": "$REPO_ROOT/tests/fakes/fake_agy.py"
    },
    {
      "id": "dsh",
      "kind": "dsh",
      "docker": "$TMP/khong-co-docker",
      "container": "dsh-smoke",
      "allowed_profiles": ["personal"]
    }
  ]
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

# Task 1 (default mac-dinh workflow): one Review plan run that approved the
# plan, and autopilot paused at the Bạn gate.
python3 - "$AGENT_BRAIN_DB" <<'PYEOF'
import sys, sqlite3, json
conn = sqlite3.connect(sys.argv[1], timeout=5)
cur = conn.cursor()
cur.execute("SELECT project_id, workflow_json FROM tasks WHERE id = 1")
row = cur.fetchone()
if not row:
    print("Lỗi seeding: Không tìm thấy task 1", file=sys.stderr)
    sys.exit(1)
project_id, workflow_json = row
if not workflow_json:
    print("Lỗi seeding: task 1 không có workflow_json", file=sys.stderr)
    sys.exit(1)
cur.execute("SELECT MAX(COALESCE(ended_at, started_at)) FROM runs WHERE task_id = 1")
lead_end = cur.fetchone()[0]
started_at = lead_end + 1000
ended_at = started_at + 90000
cur.execute("""
    INSERT INTO runs (task_id, project_id, role, status, profile, outcome, usage_source, cost_usd,
                      num_turns, started_at, ended_at, stage, round, metrics_json)
    VALUES (?, ?, 'plan-review', 'succeeded', 'work', 'review:approve', 'result', 0.4,
            6, ?, ?, 'plan-review', 1, '{"verdict":"approve","blocking":0,"advisory":2}')
""", (1, project_id, started_at, ended_at))
review_run_id = cur.lastrowid
# The Review plan run gets log lines (copied from the Lead run) so the log
# card and the raw-JSON keys check have rows for the newest run.
cur.execute("SELECT id FROM runs WHERE task_id = 1 AND role = 'lead' ORDER BY id LIMIT 1")
lead_run_id = cur.fetchone()[0]
cur.execute("""
    INSERT INTO events (run_id, seq, ts, kind, raw_json)
    SELECT ?, seq, ? + seq, kind, raw_json FROM events WHERE run_id = ? ORDER BY seq
""", (review_run_id, started_at, lead_run_id))
if cur.rowcount < 1:
    print("Lỗi seeding: Lead run của task 1 không có dòng log nào", file=sys.stderr)
    sys.exit(1)
cur.execute("UPDATE tasks SET auto_state = 'gate' WHERE id = 1")
# 06-17: the task runs on `doi-thu` after one change made on the dashboard.
team = {"name": "doi-thu", "history": [{"at": started_at, "by": "dashboard", "from": "mac-dinh", "to": "doi-thu"}]}
cur.execute("UPDATE tasks SET team_json = ? WHERE id = 1", (json.dumps(team),))
conn.commit()
conn.close()
PYEOF

# Task 2 (dung-lai): autopilot stopped in the build loop — Review code still
# asks for changes after 3 / 3 rounds (hết vòng). `run lead` checks out
# task/dung-lai from the current HEAD (task/sua-app); the fake Lead writes a
# different plan so the plan commit is not empty.
cat << 'EOF' > "$TMP/fake_claude_scenario_stop.json"
{
  "ops": [
    {
      "op": "read",
      "path": "src/app.py"
    },
    {
      "op": "write",
      "path": ".plan.md",
      "content": "# Plan: Dừng lại\n## Mục tiêu\nThực hiện Dừng lại\n## Thay đổi\n| # | File | Dòng / hàm | Sửa gì | Lý do |\n|---|---|---|---|---|\n| 1 | src/app.py | app | trả về 2 | yêu cầu |\n## Không được đụng tới\n\n## Cách kiểm tra\npython3 -m unittest\n## Vướng mắc (Dev ghi vào đây nếu có)\n\n",
      "keep_meta": true
    }
  ]
}
EOF

FAKE_CLAUDE_SCENARIO="$TMP/fake_claude_scenario_stop.json" \
  "$BIN" run lead --name dung-lai -- web-ban-hang "dừng lại" >/dev/null 2>&1

PLAN2_COMMIT="$(git -C "$REPO_DIR" log --format=%H -1 --grep='^dung-lai: plan$' task/dung-lai)"
if [[ -z "$PLAN2_COMMIT" ]]; then
  echo "Lỗi seeding: không tìm thấy commit 'dung-lai: plan' trên task/dung-lai" >&2
  exit 1
fi

python3 - "$AGENT_BRAIN_DB" "$PLAN2_COMMIT" <<'PYEOF'
import sys, sqlite3, json
db_path, plan_commit = sys.argv[1], sys.argv[2]
conn = sqlite3.connect(db_path, timeout=5)
cur = conn.cursor()
cur.execute("SELECT id, project_id, workflow_json FROM tasks WHERE slug = 'dung-lai'")
row = cur.fetchone()
if not row:
    print("Lỗi seeding: Không tìm thấy task dung-lai", file=sys.stderr)
    sys.exit(1)
task_id, project_id, workflow_json = row
if task_id != 2 or not workflow_json:
    print(f"Lỗi seeding: task dung-lai có id {task_id} (kỳ vọng 2) hoặc thiếu workflow_json", file=sys.stderr)
    sys.exit(1)
cur.execute("SELECT MAX(COALESCE(ended_at, started_at)) FROM runs WHERE task_id = ?", (task_id,))
t = cur.fetchone()[0] + 1000
MIN = 60000

# role, stage, round, outcome, metrics_json, usage_source, cost_usd, input / output tokens, num_turns, minutes
rows = [
    ('plan-review', 'plan-review', 1, 'review:approve', '{"verdict":"approve","blocking":0,"advisory":1}', 'result', 0.5, 0, 0, 5, 3),
    ('dev', 'implement', 1, None, None, 'result', 5.0, 0, 0, 12, 6),
    ('code-review', 'code-review', 1, 'review:revise', '{"verdict":"revise","blocking":1,"advisory":3}', 'result', 1.5, 0, 0, 8, 3),
    ('dev', 'fix', 2, None, '{"fix_for":"code_review"}', 'result', 4.0, 0, 0, 9, 5),
    ('code-review', 'code-review', 2, 'review:revise', '{"verdict":"revise","blocking":1,"advisory":0}', 'result', 1.5, 0, 0, 7, 3),
    # partial usage: cost is estimated from tokens (sonnet: 0.5M in + 0.2M out = $3.00)
    ('dev', 'fix', 3, None, '{"fix_for":"code_review"}', 'partial', 0.0, 500000, 200000, None, 4),
    ('code-review', 'code-review', 3, 'review:revise', '{"verdict":"revise","blocking":2,"advisory":1}', 'result', 1.5, 0, 0, 7, 3),
]
approved_at = None
for role, stage, rnd, outcome, metrics, usage, cost, tin, tout, turns, minutes in rows:
    started_at = t
    ended_at = t + minutes * MIN
    cur.execute("""
        INSERT INTO runs (task_id, project_id, role, status, profile, model, outcome, usage_source, cost_usd,
                          input_tokens, output_tokens, num_turns, started_at, ended_at, stage, round, metrics_json)
        VALUES (?, ?, ?, 'succeeded', 'work', 'sonnet', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    """, (task_id, project_id, role, outcome, usage, cost, tin, tout, turns, started_at, ended_at, stage, rnd, metrics))
    t = ended_at + 1000
    if stage == 'plan-review':
        # Bạn approved the plan right after Review plan.
        approved_at = t
        t += 30000

stop = {
    "stop": {
        "reason": "het_vong",
        "loop_name": "build",
        "stage": "code-review",
        "round": 3,
        "cap": 3,
        "at_ms": t,
    }
}
cur.execute(
    "UPDATE tasks SET approved_at = ?, approved_commit = ?, auto_state = 'stopped', auto_json = ? WHERE id = ?",
    (approved_at, plan_commit, json.dumps(stop), task_id),
)
conn.commit()
conn.close()
PYEOF

# Task 3 (quota-test, 06-17): team `tiet-kiem`, a Lead run on claude and a Dev run on deepseek
# that stopped on quota (outcome `quota`). `run lead` checks out task/quota-test from the
# current HEAD; the fake Lead writes a different plan so the plan commit is not empty.
cat << 'EOF' > "$TMP/fake_claude_scenario_quota.json"
{
  "ops": [
    {
      "op": "read",
      "path": "src/app.py"
    },
    {
      "op": "write",
      "path": ".plan.md",
      "content": "# Plan: Hết quota\n## Mục tiêu\nThực hiện Hết quota\n## Thay đổi\n| # | File | Dòng / hàm | Sửa gì | Lý do |\n|---|---|---|---|---|\n| 1 | src/app.py | app | trả về 3 | yêu cầu |\n## Không được đụng tới\n\n## Cách kiểm tra\npython3 -m unittest\n## Vướng mắc (Dev ghi vào đây nếu có)\n\n",
      "keep_meta": true
    }
  ]
}
EOF

FAKE_CLAUDE_SCENARIO="$TMP/fake_claude_scenario_quota.json" \
  "$BIN" run lead --name quota-test -- web-ban-hang "hết quota" >/dev/null 2>&1

python3 - "$AGENT_BRAIN_DB" <<'PYEOF'
import sys, sqlite3, json
conn = sqlite3.connect(sys.argv[1], timeout=5)
cur = conn.cursor()
cur.execute("SELECT id, project_id FROM tasks WHERE slug = 'quota-test'")
row = cur.fetchone()
if not row or row[0] != 3:
    print(f"Lỗi seeding: task quota-test có id {row[0] if row else None} (kỳ vọng 3)", file=sys.stderr)
    sys.exit(1)
task_id, project_id = row
cur.execute("SELECT MAX(COALESCE(ended_at, started_at)) FROM runs WHERE task_id = ?", (task_id,))
started_at = cur.fetchone()[0] + 1000
ended_at = started_at + 4 * 60000
cur.execute("""
    INSERT INTO runs (task_id, project_id, role, engine, status, profile, model, outcome, usage_source, cost_usd,
                      num_turns, started_at, ended_at, stage, round)
    VALUES (?, ?, 'dev', 'deepseek', 'failed', 'work', 'sonnet', 'quota', 'result', 0.3, 5, ?, ?, 'implement', 1)
""", (task_id, project_id, started_at, ended_at))
team = {"name": "tiet-kiem", "history": []}
cur.execute("UPDATE tasks SET team_json = ? WHERE id = ?", (json.dumps(team), task_id))
conn.commit()
conn.close()
PYEOF

# Task 4 (nguoi-lam, 06-18): team `doi-nguoi`, a person (Hà) waits for the Code step for 3 days
# and 2 hours; the branch was pushed (push.ok). `run lead` checks out task/nguoi-lam from the
# current HEAD; the fake Lead writes a different plan so the plan commit is not empty.
cat << 'EOF' > "$TMP/fake_claude_scenario_human.json"
{
  "ops": [
    {
      "op": "read",
      "path": "src/app.py"
    },
    {
      "op": "write",
      "path": ".plan.md",
      "content": "# Plan: Người làm\n## Mục tiêu\nThực hiện Người làm\n## Thay đổi\n| # | File | Dòng / hàm | Sửa gì | Lý do |\n|---|---|---|---|---|\n| 1 | src/app.py | app | trả về 4 | yêu cầu |\n## Không được đụng tới\n\n## Cách kiểm tra\npython3 -m unittest\n## Vướng mắc (Dev ghi vào đây nếu có)\n\n",
      "keep_meta": true
    }
  ]
}
EOF

FAKE_CLAUDE_SCENARIO="$TMP/fake_claude_scenario_human.json" \
  "$BIN" run lead --name nguoi-lam -- web-ban-hang "người làm" >/dev/null 2>&1

HUMAN_BASE="$(git -C "$REPO_DIR" rev-parse task/nguoi-lam)"

python3 - "$AGENT_BRAIN_DB" "$HUMAN_BASE" <<'PYEOF'
import sys, sqlite3, json, time
conn = sqlite3.connect(sys.argv[1], timeout=5)
cur = conn.cursor()
cur.execute("SELECT id FROM tasks WHERE slug = 'nguoi-lam'")
row = cur.fetchone()
if not row or row[0] != 4:
    print(f"Lỗi seeding: task nguoi-lam có id {row[0] if row else None} (kỳ vọng 4)", file=sys.stderr)
    sys.exit(1)
now_ms = int(time.time() * 1000)
since_ms = now_ms - (3 * 24 + 2) * 3600 * 1000 - 5 * 60000
wait = {
    "mode": "waiting", "role": "dev", "stage": "implement", "round": 1, "staff": "ha",
    "since_ms": since_ms, "branch": "task/nguoi-lam", "base": sys.argv[2],
    "report_paths": [],
    "push": {"ok": True, "command": "git push -u origin task/nguoi-lam", "error": None},
    "autopilot": False,
}
team = {"name": "doi-nguoi", "history": [], "wait": wait}
cur.execute("UPDATE tasks SET team_json = ? WHERE id = 4", (json.dumps(team),))
conn.commit()
conn.close()
PYEOF

# Task 5 (do-thong-ke, 06-18): Dev runs on claude, deepseek (no price in config.json) and a
# person, each followed by a QA verdict, for the Thống kê engine and QA-rate cards.
cat << 'EOF' > "$TMP/fake_claude_scenario_statsfx.json"
{
  "ops": [
    {
      "op": "read",
      "path": "src/app.py"
    },
    {
      "op": "write",
      "path": ".plan.md",
      "content": "# Plan: Đo thống kê\n## Mục tiêu\nThực hiện Đo thống kê\n## Thay đổi\n| # | File | Dòng / hàm | Sửa gì | Lý do |\n|---|---|---|---|---|\n| 1 | src/app.py | app | trả về 5 | yêu cầu |\n## Không được đụng tới\n\n## Cách kiểm tra\npython3 -m unittest\n## Vướng mắc (Dev ghi vào đây nếu có)\n\n",
      "keep_meta": true
    }
  ]
}
EOF

FAKE_CLAUDE_SCENARIO="$TMP/fake_claude_scenario_statsfx.json" \
  "$BIN" run lead --name do-thong-ke -- web-ban-hang "đo thống kê" >/dev/null 2>&1

python3 - "$AGENT_BRAIN_DB" <<'PYEOF'
import sys, sqlite3
conn = sqlite3.connect(sys.argv[1], timeout=5)
cur = conn.cursor()
cur.execute("SELECT id, project_id FROM tasks WHERE slug = 'do-thong-ke'")
row = cur.fetchone()
if not row or row[0] != 5:
    print(f"Lỗi seeding: task do-thong-ke có id {row[0] if row else None} (kỳ vọng 5)", file=sys.stderr)
    sys.exit(1)
task_id, project_id = row
cur.execute("SELECT MAX(COALESCE(ended_at, started_at)) FROM runs WHERE task_id = ?", (task_id,))
t = cur.fetchone()[0] + 1000
MIN = 60000
# role, engine, stage, round, outcome, input tokens, output tokens, cost, minutes
rows = [
    ('dev', 'claude', 'implement', 1, None, 12000, 3000, 0.40, 5),
    ('qa', 'claude', 'qa', 1, 'qa:pass', 6000, 800, 0.10, 2),
    ('dev', 'deepseek', 'fix', 2, None, 40000, 9000, 0.20, 6),
    ('qa', 'claude', 'qa', 2, 'qa:fix', 6000, 900, 0.10, 2),
    ('dev', 'deepseek', 'fix', 3, None, 30000, 7000, 0.15, 5),
    ('qa', 'claude', 'qa', 3, 'qa:pass', 6000, 700, 0.10, 2),
    ('dev', 'người', 'fix', 4, None, 0, 0, 0.0, 120),
    ('qa', 'claude', 'qa', 4, 'qa:pass', 6000, 600, 0.10, 2),
]
for role, engine, stage, rnd, outcome, tin, tout, cost, minutes in rows:
    started_at = t
    ended_at = t + minutes * MIN
    cur.execute("""
        INSERT INTO runs (task_id, project_id, role, engine, status, profile, model, outcome, usage_source, cost_usd,
                          input_tokens, output_tokens, num_turns, started_at, ended_at, stage, round)
        VALUES (?, ?, ?, ?, 'succeeded', 'work', 'sonnet', ?, 'result', ?, ?, ?, 4, ?, ?, ?, ?)
    """, (task_id, project_id, role, engine, outcome, cost, tin, tout, started_at, ended_at, stage, rnd))
    t = ended_at + 1000
conn.commit()
conn.close()
PYEOF

# Task 1 stays the checked-out task (its actions, e.g. Chạy lại Lead for the
# keys confirm-dialog check, need its branch checked out).
git -C "$REPO_DIR" checkout task/sua-app >/dev/null 2>&1

# 06-15: the smoke project gets a default team only now, after the CLI seeding above, so
# the seeded tasks keep running on the default team; the Giao việc picker reads it.
python3 - "$TMP/xdg_config/agent-brain/projects_map.json" <<'PYEOF'
import json, sys
path = sys.argv[1]
with open(path, 'r', encoding='utf-8') as f:
    data = json.load(f)
for p in data['projects']:
    if p['id'] == 'web-ban-hang':
        p['team'] = 'doi-thu'
with open(path, 'w', encoding='utf-8') as f:
    json.dump(data, f, ensure_ascii=False, indent=2)
PYEOF

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

# One headless Chrome DOM dump: dump_dom URL OUT_FILE PROFILE_DIR. Chrome runs in
# the foreground and exits after the dump.
dump_dom() {
  "$CHROME_BIN" \
    --headless=new \
    --no-sandbox \
    --disable-gpu \
    --no-first-run \
    --disable-background-networking \
    --disable-component-update \
    --disable-sync \
    --user-data-dir="$3" \
    --virtual-time-budget=6000 \
    --dump-dom "$1" > "$2" 2>/dev/null || true
}

for s in "${SCREENS[@]}"; do
  # data-screen of the rendered screen (task-stop is the task screen of task 2)
  SCREEN_NAME="$s"
  case "$s" in
    overview) HASH="#/" ;;
    assign) HASH="#/assign" ;;
    task) HASH="#/task/1" ;;
    task-stop) HASH="#/task/2"; SCREEN_NAME="task" ;;
    task-quota) HASH="#/task/3"; SCREEN_NAME="task" ;;
    task-human) HASH="#/task/4"; SCREEN_NAME="task" ;;
    stats) HASH="#/stats" ;;
    projects) HASH="#/projects" ;;
    board) HASH="#/board" ;;
    settings) HASH="#/settings" ;;
    telegram) HASH="#/telegram" ;;
    *)
      echo "Lỗi: Màn hình không xác định: $s" >&2
      exit 1
      ;;
  esac

  TARGET_URL="${URL}${HASH}"
  DOM_OUT="$TMP/dom-$s.html"

  dump_dom "$TARGET_URL" "$DOM_OUT" "$TMP/chrome-$s"

  # Overview race (05.2-UI-REVIEW Pillar 6): virtual time does not wait for the
  # real /api/tasks answer, so a slow first answer leaves the groups empty.
  # Retry while no task item is rendered (the template's `:data-task-id` does
  # not count), with a fresh Chrome profile each time, at most 3 attempts; the
  # assertions below run on the last dump, so a real failure still fails.
  if [[ "$s" == "overview" ]]; then
    ATTEMPT=1
    while ! grep -qE '[[:space:]]data-task-id="[0-9]+"' "$DOM_OUT" && (( ATTEMPT < 3 )); do
      ATTEMPT=$((ATTEMPT + 1))
      echo "Thử lại overview (lần $ATTEMPT)" >&2
      sleep 1
      dump_dom "$TARGET_URL" "$DOM_OUT" "$TMP/chrome-$s-$ATTEMPT"
    done
  fi

  # Stats race, same cause: retry while the engine rows are not drawn yet.
  if [[ "$s" == "stats" ]]; then
    ATTEMPT=1
    while ! grep -q 'chưa có giá · 3 run' "$DOM_OUT" && (( ATTEMPT < 3 )); do
      ATTEMPT=$((ATTEMPT + 1))
      echo "Thử lại stats (lần $ATTEMPT)" >&2
      sleep 1
      dump_dom "$TARGET_URL" "$DOM_OUT" "$TMP/chrome-$s-$ATTEMPT"
    done
  fi

  # Board race, same cause: Alpine shows an x-show block a frame after the state
  # changes, and virtual time can dump the DOM between the two, leaving the board
  # layout hidden or the cards not drawn yet. Retry while no card is drawn or the
  # layout is still hidden (at most 3 attempts, fresh profile each time).
  if [[ "$s" == "board" ]]; then
    ATTEMPT=1
    while { ! grep -qE '[[:space:]]data-item-key="02-01"' "$DOM_OUT" || grep -qE 'data-board-layout[^>]*display: none' "$DOM_OUT"; } && (( ATTEMPT < 3 )); do
      ATTEMPT=$((ATTEMPT + 1))
      echo "Thử lại board (lần $ATTEMPT)" >&2
      sleep 1
      dump_dom "$TARGET_URL" "$DOM_OUT" "$TMP/chrome-$s-$ATTEMPT"
    done
  fi

  if ! grep -q "data-screen=\"$SCREEN_NAME\"" "$DOM_OUT" || ! grep -q 'data-loaded="yes"' "$DOM_OUT"; then
    echo "Lỗi: Màn hình $s không render thành công (thiếu data-screen=\"$SCREEN_NAME\" hoặc data-loaded=\"yes\")" >&2
    echo "--- 40 dòng đầu của DOM ---" >&2
    head -n 40 "$DOM_OUT" >&2 || true
    echo "--- serve stderr ---" >&2
    if [[ -f "$TMP/serve.err" ]]; then
      cat "$TMP/serve.err" >&2
    fi
    exit 1
  fi

  if [[ "$s" == "task" || "$s" == "task-stop" || "$s" == "task-quota" || "$s" == "task-human" ]]; then
    if ! python3 - "$DOM_OUT" "$s" <<'EOF'
import sys
from html.parser import HTMLParser

VOID_TAGS = {'area', 'base', 'br', 'col', 'embed', 'hr', 'img', 'input', 'link', 'meta', 'source', 'track', 'wbr'}

class PipelineParser(HTMLParser):
    def __init__(self):
        super().__init__()
        self.template_depth = 0
        self.data_pipeline = None
        self.cards = {}
        self.card_texts = {}
        self.card_counts = {}
        self.aria_current_cards = set()
        self.button_ids = set()
        self.legend_items = []
        self.chips = []
        self.in_chip = False
        self.ledger_rows = []
        self.in_ledger_total = False
        self.ledger_total_text = ''
        self.full_text = ''
        self.badges = []
        self.groups = {}
        self.group_footers = {}
        self.budget_text = ''
        self.card_buttons = {}
        self.banners = []
        self.banner_buttons = set()
        self.strip_buttons = set()
        self.textareas = {}
        self.ledger_text = ''
        self.token_lines = []
        self.team_line_text = ''
        self.team_buttons = set()
        self.team_history = []
        self.strip_text = ''
        self.human_text = ''
        self.human_buttons = set()
        self.human_callouts = 0
        self.depth = 0
        self.captures = []
        self.current_card = None
        self.current_group = None

    def handle_starttag(self, tag, attrs):
        if tag == 'template':
            self.template_depth += 1
            return
        if self.template_depth > 0:
            return

        attr_dict = dict(attrs)
        if 'data-pipeline' in attr_dict and self.data_pipeline is None:
            self.data_pipeline = attr_dict['data-pipeline']

        if 'data-card' in attr_dict:
            card_key = attr_dict['data-card']
            self.cards[card_key] = attr_dict.get('data-state')
            if attr_dict.get('aria-current') == 'step':
                self.aria_current_cards.add(card_key)

        if tag == 'button' and 'id' in attr_dict:
            bid = attr_dict['id']
            self.button_ids.add(bid)
            # Where the button sits: innermost card, stop banner, action strip.
            for _, kind, ref in self.captures:
                if kind == 'card':
                    self.card_buttons.setdefault(ref, set()).add(bid)
                elif kind == 'banner':
                    self.banner_buttons.add(bid)
                elif kind == 'strip':
                    self.strip_buttons.add(bid)
                elif kind == 'teamline':
                    self.team_buttons.add(bid)
                elif kind == 'human':
                    self.human_buttons.add(bid)

        if tag == 'textarea' and 'id' in attr_dict:
            self.textareas[attr_dict['id']] = attr_dict

        if tag not in VOID_TAGS:
            self.depth += 1
            # Record the text of marked elements (nested captures allowed).
            if 'data-auto-badge' in attr_dict:
                self.badges.append({'role': attr_dict.get('role'), 'text': ''})
                self.captures.append((self.depth, 'badge', len(self.badges) - 1))
            if 'data-group' in attr_dict:
                key = attr_dict['data-group']
                self.groups[key] = attr_dict.get('class', '')
                self.group_footers.setdefault(key, '')
                self.captures.append((self.depth, 'group', key))
            if 'data-group-footer' in attr_dict:
                self.captures.append((self.depth, 'footer', None))
            if 'data-card' in attr_dict:
                key = attr_dict['data-card']
                self.card_texts[key] = ''
                self.captures.append((self.depth, 'card', key))
            if 'data-card-counts' in attr_dict:
                self.captures.append((self.depth, 'counts', None))
            if 'data-budget' in attr_dict:
                self.captures.append((self.depth, 'budget', None))
            if 'data-stop-banner' in attr_dict:
                self.banners.append({'aria_live': attr_dict.get('aria-live'), 'style': attr_dict.get('style') or '', 'text': ''})
                self.captures.append((self.depth, 'banner', len(self.banners) - 1))
            if 'data-action-strip' in attr_dict:
                self.captures.append((self.depth, 'strip', None))
            if 'data-team-line' in attr_dict:
                self.captures.append((self.depth, 'teamline', None))
            if 'data-human-callout' in attr_dict:
                self.human_callouts += 1
                self.captures.append((self.depth, 'human', None))
            if 'data-team-history' in attr_dict:
                self.captures.append((self.depth, 'teamhist', None))
            if tag == 'li' and any(k == 'teamhist' for _, k, _ in self.captures):
                self.team_history.append('')
                self.captures.append((self.depth, 'teamitem', len(self.team_history) - 1))
            if 'data-ledger' in attr_dict:
                self.captures.append((self.depth, 'ledger', None))
            if 'data-token-line' in attr_dict:
                self.token_lines.append({'style': attr_dict.get('style') or '', 'text': ''})
                self.captures.append((self.depth, 'token', len(self.token_lines) - 1))

        if 'data-legend-item' in attr_dict:
            self.legend_items.append(tag)

        if 'data-placeholder-chip' in attr_dict:
            self.chips.append({
                'tag': tag,
                'has_tabindex': 'tabindex' in attr_dict,
                'text': '',
            })
            self.in_chip = True

        if 'data-ledger-row' in attr_dict:
            self.ledger_rows.append(attr_dict)

        if 'data-ledger-total' in attr_dict:
            self.in_ledger_total = True

    def handle_data(self, data):
        if self.template_depth == 0:
            self.full_text += data
            group = None
            card = None
            for _, kind, ref in self.captures:
                if kind == 'group':
                    group = ref
                elif kind == 'card':
                    card = ref
            for _, kind, ref in self.captures:
                if kind == 'badge':
                    self.badges[ref]['text'] += data
                elif kind == 'card':
                    self.card_texts[ref] += data
                elif kind == 'footer' and group is not None:
                    self.group_footers[group] += data
                elif kind == 'counts' and card is not None:
                    self.card_counts[card] = self.card_counts.get(card, '') + data
                elif kind == 'budget':
                    self.budget_text += data
                elif kind == 'banner':
                    self.banners[ref]['text'] += data
                elif kind == 'ledger':
                    self.ledger_text += data
                elif kind == 'token':
                    self.token_lines[ref]['text'] += data
                elif kind == 'strip':
                    self.strip_text += data
                elif kind == 'teamline':
                    self.team_line_text += data
                elif kind == 'human':
                    self.human_text += data
                elif kind == 'teamitem':
                    self.team_history[ref] += data
            if self.in_chip:
                self.chips[-1]['text'] += data
            if self.in_ledger_total:
                self.ledger_total_text += data

    def handle_endtag(self, tag):
        if tag == 'template':
            if self.template_depth > 0:
                self.template_depth -= 1
            return
        if self.template_depth > 0:
            return
        if tag not in VOID_TAGS:
            while self.captures and self.captures[-1][0] >= self.depth:
                self.captures.pop()
            self.depth -= 1
        if self.in_chip and tag == 'li':
            self.in_chip = False
        if self.in_ledger_total and tag == 'p':
            self.in_ledger_total = False

parser = PipelineParser()
with open(sys.argv[1], encoding='utf-8') as f:
    parser.feed(f.read())
screen = sys.argv[2]

if parser.data_pipeline != "ready":
    print(f"Lỗi: data-pipeline không phải là 'ready' (nhận được: '{parser.data_pipeline}')", file=sys.stderr)
    sys.exit(1)

def fail(msg):
    print(f"Lỗi ({screen}): {msg}", file=sys.stderr)
    sys.exit(1)

if screen == 'task-human':
    # Task 4 (06-18): Hà holds the Code step for 3 days; the callout comes from the task-detail payload.
    if parser.human_callouts != 1:
        fail(f"Số phần tử data-human-callout là {parser.human_callouts}, kỳ vọng 1")
    htext = ' '.join(parser.human_text.split())
    for needle in ('Chờ Hà làm bước Code', 'task/nguoi-lam', 'Sao chép tên nhánh', 'Đã chờ 3 ngày',
                   'Hà làm trên máy của họ rồi đẩy nhánh này lên remote',
                   'Lấy nhánh task/nguoi-lam từ remote vào thư mục làm việc rồi chạy tiếp quy trình.'):
        if needle not in htext:
            fail(f"Callout người làm không chứa '{needle}' (nội dung: '{htext}')")
    if 'ab-act-human_done' not in parser.human_buttons:
        fail(f"Thiếu nút Xong (ab-act-human_done) trong callout (nút trong callout: {parser.human_buttons})")
    if 'ab-act-human_return' in parser.human_buttons:
        fail("Chế độ chờ không được có nút Trả lại")
    if 'ab-act-human_done' in parser.strip_buttons or 'ab-act-human_return' in parser.strip_buttons:
        fail(f"Xong / Trả lại không được nằm trong dải hành động (nút: {parser.strip_buttons})")
    # The strip's own buttons stay but say why they are off (D-29).
    if 'Đang chờ Hà làm — bấm Xong khi họ báo xong.' not in ' '.join(parser.strip_text.split()):
        fail(f"Dải hành động thiếu lý do 'Đang chờ Hà làm — bấm Xong khi họ báo xong.' (nội dung: '{parser.strip_text}')")
    # The smoke repo has task/sua-app checked out, so the server refuses Xong off the task branch and says why.
    if 'Không ở nhánh task' not in htext:
        fail(f"Lý do của server không hiện dưới nút Xong (nội dung: '{htext}')")
    if len(parser.badges) != 1 or parser.badges[0]['role'] != 'status':
        fail(f"Huy hiệu autopilot sai (nhận được: {parser.badges})")
    team_text = ' '.join(parser.team_line_text.split())
    if 'Đội hình: doi-nguoi' not in team_text:
        fail(f"data-team-line không chứa 'Đội hình: doi-nguoi' (nội dung: '{team_text}')")
    print("OK task-human")
    sys.exit(0)

if screen == 'task-quota':
    # Task 3 (06-17): the Dev run on deepseek stopped on quota; the stop slot carries the quota banner.
    if len(parser.banners) != 1:
        fail(f"Số phần tử data-stop-banner là {len(parser.banners)}, kỳ vọng 1")
    banner = parser.banners[0]
    btext = ' '.join(banner['text'].split())
    if banner['aria_live'] != 'polite':
        fail(f"data-stop-banner có aria-live='{banner['aria_live']}', kỳ vọng 'polite'")
    if 'display: none' in banner['style']:
        fail("data-stop-banner đang bị ẩn")
    for needle in ('Hết quota engine deepseek', 'Chạy lại bước này bằng đội hình…', 'Thử lại sau',
                   '(dev) trên engine deepseek dừng vì hết quota hoặc bị giới hạn tốc độ. Agent-brain không tự đổi engine.',
                   'Chạy lại bằng đội hình khác sẽ tiếp tục trên thư mục làm việc hiện tại'):
        if needle not in btext:
            fail(f"Banner quota không chứa '{needle}' (nội dung: '{btext}')")
    if 'Dừng hẳn' in btext:
        fail("Banner quota không được có 'Dừng hẳn' khi autopilot tắt")
    for bid in ('ab-act-quota_rerun', 'ab-act-quota_later'):
        if bid not in parser.banner_buttons:
            fail(f"Thiếu nút {bid} trong banner quota (nút trong banner: {parser.banner_buttons})")
    if 'ab-act-quota_rerun' in parser.strip_buttons or 'ab-act-change_team' in parser.strip_buttons:
        fail("quota_rerun / change_team không được nằm trong dải hành động")
    code_text = ' '.join(parser.card_texts.get('code', '').split())
    if 'deepseek' not in code_text or 'hết quota' not in code_text:
        fail(f"Card code không có engine 'deepseek' và 'hết quota' (nội dung: '{code_text}')")
    if parser.cards.get('code') != 'het_quota':
        fail(f"Card code có state '{parser.cards.get('code')}', kỳ vọng 'het_quota'")
    ledger = ' '.join(parser.ledger_text.split())
    if '· deepseek' not in ledger or 'hết quota' not in ledger:
        fail(f"Sổ lượt chạy không có '· deepseek' và 'hết quota' (nội dung: '{ledger}')")
    total = ' '.join(parser.ledger_total_text.split())
    if 'lượt chạy ·' not in total or 'claude' in total:
        fail(f"Dòng tổng phải là '{{N}} lượt chạy · …' không có 'claude' (nội dung: '{total}')")
    if len(parser.legend_items) != 7:
        fail(f"Số lượng data-legend-item là {len(parser.legend_items)}, kỳ vọng 7")
    team_text = ' '.join(parser.team_line_text.split())
    if 'Đội hình: tiet-kiem' not in team_text:
        fail(f"data-team-line không chứa 'Đội hình: tiet-kiem' (nội dung: '{team_text}')")
    if parser.team_history:
        fail(f"Task 3 không có lịch sử đổi đội hình (nhận được: {parser.team_history})")
    print("OK task-quota")
    sys.exit(0)

if screen == 'task-stop':
    # Task 2: autopilot stopped in the build loop after 3 / 3 rounds (hết vòng).
    expected_cards = {'start', 'plan', 'plan_review', 'ban', 'code', 'code_review', 'qa', 'fix'}
    if set(parser.cards.keys()) != expected_cards:
        fail(f"Danh sách card không khớp. Kỳ vọng {expected_cards}, nhận được {set(parser.cards.keys())}")
    if set(parser.groups.keys()) != {'plan', 'build', 'qa'}:
        fail(f"Danh sách data-group là {set(parser.groups.keys())}, kỳ vọng {{'plan', 'build', 'qa'}}")
    build_cls = parser.groups['build']
    if 'ring-2' not in build_cls or 'ring-amber-500' not in build_cls:
        fail(f"Khung nhóm build không có ring-2 ring-amber-500 (class: '{build_cls}')")
    for key in ('plan', 'qa'):
        if 'ring-amber-500' in parser.groups[key] or 'ring-1' not in parser.groups[key]:
            fail(f"Khung nhóm {key} phải là ring-1 thường (class: '{parser.groups[key]}')")
    build_footer = parser.group_footers.get('build', '')
    if '⟲' not in build_footer or 'vòng 3 / 3' not in build_footer:
        fail(f"Chân nhóm build không chứa '⟲ vòng 3 / 3' (nội dung: '{build_footer}')")
    if 'hết vòng' not in build_footer:
        fail(f"Chân nhóm build không chứa 'hết vòng' (nội dung: '{build_footer}')")
    if 'chưa chạy' not in parser.group_footers.get('qa', ''):
        fail(f"Chân nhóm qa không chứa 'chưa chạy' (nội dung: '{parser.group_footers.get('qa', '')}')")
    if 'hết vòng' in parser.group_footers.get('plan', ''):
        fail("Chân nhóm plan không được có lý do dừng")
    expected_states = {
        'plan_review': 'duyet',
        'ban': 'xong',
        'code': 'xong',
        'code_review': 'sua_lai',
        'qa': 'cho',
        'fix': 'cho',
    }
    for k, st in expected_states.items():
        if parser.cards.get(k) != st:
            fail(f"Card '{k}' có state '{parser.cards.get(k)}', kỳ vọng '{st}'")
    if '2 chặn' not in parser.card_counts.get('code_review', ''):
        fail(f"Card code_review không có '2 chặn' (counts: '{parser.card_counts.get('code_review', '')}')")
    if '1 góp ý' not in parser.card_counts.get('code_review', ''):
        fail(f"Card code_review không có '1 góp ý' (counts: '{parser.card_counts.get('code_review', '')}')")
    if '0 chặn' not in parser.card_counts.get('plan_review', ''):
        fail(f"Card plan_review không có '0 chặn' (counts: '{parser.card_counts.get('plan_review', '')}')")
    if 'code' in parser.card_counts and parser.card_counts['code'].strip():
        fail(f"Card code không được có dòng counts (nội dung: '{parser.card_counts['code']}')")
    if parser.aria_current_cards != {'code_review'}:
        fail(f"Các card có aria-current='step' là {parser.aria_current_cards}, kỳ vọng {{'code_review'}}")
    if len(parser.badges) != 1 or 'Autopilot: chờ bạn · hết vòng' not in parser.badges[0]['text']:
        fail(f"Huy hiệu autopilot không phải 'Autopilot: chờ bạn · hết vòng' ({parser.badges})")
    if 'Ngân sách:' not in parser.budget_text or '≈ $' not in parser.budget_text:
        fail(f"Dòng ngân sách thiếu 'Ngân sách:' hoặc '≈ $' (nội dung: '{parser.budget_text}')")
    if 'sắp hết' not in parser.budget_text and 'đã hết' not in parser.budget_text:
        fail(f"Dòng ngân sách thiếu 'sắp hết' / 'đã hết' (nội dung: '{parser.budget_text}')")
    if 'Gồm chi phí ước tính từ token cho 1 run' not in parser.full_text:
        fail("DOM không chứa ghi chú ước tính 'Gồm chi phí ước tính từ token cho 1 run'")
    if 'Quy trình: mac-dinh ·' not in parser.full_text:
        fail("Dòng quy trình không phải 'Quy trình: mac-dinh · …'")
    # Stop banner: amber, polite live region, server detail, three actions.
    if len(parser.banners) != 1:
        fail(f"Số phần tử data-stop-banner là {len(parser.banners)}, kỳ vọng 1")
    banner = parser.banners[0]
    if banner['aria_live'] != 'polite':
        fail(f"data-stop-banner có aria-live='{banner['aria_live']}', kỳ vọng 'polite'")
    if 'display: none' in banner['style']:
        fail("data-stop-banner đang bị ẩn")
    if 'Autopilot dừng · hết vòng' not in banner['text']:
        fail(f"Banner dừng không chứa 'Autopilot dừng · hết vòng' (nội dung: '{banner['text']}')")
    if 'Review code vẫn yêu cầu sửa lại sau 3 / 3 vòng.' not in banner['text']:
        fail(f"Banner dừng không chứa chi tiết của server (nội dung: '{banner['text']}')")
    for bid in ('ab-act-auto_more_round', 'ab-act-relead', 'ab-act-auto_end'):
        if bid not in parser.banner_buttons:
            fail(f"Thiếu nút {bid} trong banner dừng (nút trong banner: {parser.banner_buttons})")
    if 'ab-act-relead' in parser.strip_buttons:
        fail("ab-act-relead không được nằm cả trong dải hành động khi banner dừng hiện")
    # Ledger: server labels, counts first, one visible token line (selected row).
    if 'Vòng 3 · review code' not in parser.ledger_text:
        fail(f"Sổ lượt chạy không chứa 'Vòng 3 · review code'")
    if '2 chặn · 1 góp ý' not in parser.ledger_text:
        fail(f"Sổ lượt chạy không chứa '2 chặn · 1 góp ý'")
    visible_tokens = [t for t in parser.token_lines if 'display: none' not in t['style']]
    if len(visible_tokens) != 1:
        fail(f"Số data-token-line hiển thị là {len(visible_tokens)}, kỳ vọng 1 (tổng {len(parser.token_lines)})")
    if 'Token: vào' not in visible_tokens[0]['text']:
        fail(f"data-token-line không chứa 'Token: vào' (nội dung: '{visible_tokens[0]['text']}')")
    print("Stop DOM OK")
    sys.exit(0)

expected_cards = {'start', 'plan', 'plan_review', 'ban', 'code', 'code_review', 'qa', 'fix'}
if set(parser.cards.keys()) != expected_cards:
    print(f"Lỗi: Danh sách card không khớp. Kỳ vọng {expected_cards}, nhận được {set(parser.cards.keys())}", file=sys.stderr)
    sys.exit(1)

expected_states = {
    'start': 'xong',
    'plan': 'xong',
    'plan_review': 'duyet',
    'ban': 'cho_ban',
    'code': 'cho',
    'code_review': 'cho',
    'qa': 'cho',
    'fix': 'cho',
}
for k, expected_st in expected_states.items():
    actual_st = parser.cards.get(k)
    if actual_st != expected_st:
        print(f"Lỗi: Card '{k}' có state '{actual_st}', kỳ vọng '{expected_st}'", file=sys.stderr)
        sys.exit(1)

if parser.aria_current_cards != {'ban'}:
    print(f"Lỗi: Các card có aria-current='step' là {parser.aria_current_cards}, kỳ vọng {{'ban'}}", file=sys.stderr)
    sys.exit(1)

if 'Autopilot dừng ở đây để bạn duyệt plan.' not in parser.full_text:
    print("Lỗi: DOM không chứa ghi chú cổng 'Autopilot dừng ở đây để bạn duyệt plan.'", file=sys.stderr)
    sys.exit(1)

# Autopilot waits at the gate (task 1): the manual Chạy Dev / Chạy lại Lead do
# not compete with Duyệt plan / Trả về Lead (they return after Dừng autopilot).
for bid in ('ab-act-run_dev', 'ab-act-relead'):
    if bid in parser.button_ids:
        print(f"Lỗi: Nút {bid} không được hiện khi autopilot chờ ở cổng duyệt", file=sys.stderr)
        sys.exit(1)

if len(parser.legend_items) != 7:
    print(f"Lỗi: Số lượng data-legend-item là {len(parser.legend_items)}, kỳ vọng 7", file=sys.stderr)
    sys.exit(1)

if 'vướng mắc / cần sửa / sửa lại / hết quota' not in parser.full_text:
    print("Lỗi: chú thích thứ 7 không chứa 'vướng mắc / cần sửa / sửa lại / hết quota'", file=sys.stderr)
    sys.exit(1)

if 'đang chạy / chờ người' not in parser.full_text:
    print("Lỗi: chú thích thứ 2 không chứa 'đang chạy / chờ người'", file=sys.stderr)
    sys.exit(1)

# 06-17 engine lines: a card with a run names its engine, a card without one `dự kiến {engine}`
# from the task's team (doi-thu: Dev on deepseek).
plan_engine = ' '.join(parser.card_texts.get('plan', '').split())
code_engine = ' '.join(parser.card_texts.get('code', '').split())
if 'Engine: claude' not in plan_engine:
    print(f"Lỗi: card plan không có dòng engine 'Engine: claude' (nội dung: '{plan_engine}')", file=sys.stderr)
    sys.exit(1)
if 'Engine: dự kiến deepseek' not in code_engine:
    print(f"Lỗi: card code không có dòng engine 'Engine: dự kiến deepseek' (nội dung: '{code_engine}')", file=sys.stderr)
    sys.exit(1)

if len(parser.chips) != 1:
    print(f"Lỗi: Số lượng data-placeholder-chip là {len(parser.chips)}, kỳ vọng 1", file=sys.stderr)
    sys.exit(1)

if 'PR nháp · chưa tự động' not in parser.chips[0]['text']:
    print(f"Lỗi: Chip không chứa 'PR nháp · chưa tự động' (nội dung: '{parser.chips[0]['text']}')", file=sys.stderr)
    sys.exit(1)

if 'ab-act-auto_stop' not in parser.button_ids:
    print(f"Lỗi: Thiếu nút ab-act-auto_stop trong danh sách buttons: {parser.button_ids}", file=sys.stderr)
    sys.exit(1)

# Header helper while autopilot waits at the gate (UI fix 1): says what Dừng autopilot does.
if 'Autopilot dừng ở bước duyệt plan — bấm Duyệt plan để chạy tiếp.' not in parser.full_text:
    print("Lỗi: Thiếu gợi ý 'Autopilot dừng ở bước duyệt plan — bấm Duyệt plan để chạy tiếp.' ở đầu card Pipeline", file=sys.stderr)
    sys.exit(1)

if len(parser.badges) != 1:
    print(f"Lỗi: Số phần tử data-auto-badge là {len(parser.badges)}, kỳ vọng 1", file=sys.stderr)
    sys.exit(1)

if parser.badges[0]['role'] != 'status':
    print(f"Lỗi: data-auto-badge có role='{parser.badges[0]['role']}', kỳ vọng 'status'", file=sys.stderr)
    sys.exit(1)

if 'Autopilot: chờ bạn duyệt plan' not in parser.badges[0]['text']:
    print(f"Lỗi: data-auto-badge không chứa 'Autopilot: chờ bạn duyệt plan' (nội dung: '{parser.badges[0]['text']}')", file=sys.stderr)
    sys.exit(1)

if 'Quy trình: mac-dinh ·' not in parser.full_text:
    print("Lỗi: DOM không chứa 'Quy trình: mac-dinh ·'", file=sys.stderr)
    sys.exit(1)

if 'Ngân sách:' not in parser.full_text:
    print("Lỗi: DOM không chứa 'Ngân sách:'", file=sys.stderr)
    sys.exit(1)

for i, chip in enumerate(parser.chips):
    if chip['tag'] in ('a', 'button'):
        print(f"Lỗi: Chip thứ {i+1} có thẻ tương tác <{chip['tag']}>", file=sys.stderr)
        sys.exit(1)
    if chip['has_tabindex']:
        print(f"Lỗi: Chip thứ {i+1} có thuộc tính tabindex", file=sys.stderr)
        sys.exit(1)
    if 'chưa tự động' not in chip['text']:
        print(f"Lỗi: Chip thứ {i+1} không chứa 'chưa tự động' (nội dung: '{chip['text']}')", file=sys.stderr)
        sys.exit(1)

# Bạn gate (task 1 waits at the gate): Duyệt plan and Trả về Lead inside the Bạn card.
ban_buttons = parser.card_buttons.get('ban', set())
for bid in ('ab-act-approve', 'ab-act-return_lead'):
    if bid not in ban_buttons:
        print(f"Lỗi: Thiếu nút {bid} trong card Bạn (nút trong card: {ban_buttons})", file=sys.stderr)
        sys.exit(1)
    if bid in parser.strip_buttons:
        print(f"Lỗi: Nút {bid} không được nằm trong dải hành động khi pipeline tải được", file=sys.stderr)
        sys.exit(1)

note = parser.textareas.get('ab-return-note')
if note is None:
    print("Lỗi: Thiếu textarea #ab-return-note trong dialog Trả về Lead", file=sys.stderr)
    sys.exit(1)
if note.get('maxlength') != '500':
    print(f"Lỗi: textarea #ab-return-note có maxlength '{note.get('maxlength')}', kỳ vọng '500'", file=sys.stderr)
    sys.exit(1)

# Task 1 has no autopilot stop: the banner element exists but is hidden.
for b in parser.banners:
    if 'display: none' not in b['style']:
        print("Lỗi: banner dừng hiển thị trên task không bị autopilot dừng", file=sys.stderr)
        sys.exit(1)

print("Pipeline DOM OK")

# 06-17 team line: `Đội hình: {name} · {summary}`, one history line, the change button outside the strip.
team_text = ' '.join(parser.team_line_text.split())
if 'Đội hình: doi-thu' not in team_text:
    print(f"Lỗi: data-team-line không chứa 'Đội hình: doi-thu' (nội dung: '{team_text}')", file=sys.stderr)
    sys.exit(1)
if 'Đổi đội hình' not in team_text or 'ab-act-change_team' not in parser.team_buttons:
    print(f"Lỗi: thiếu nút 'Đổi đội hình' trong data-team-line (nút: {parser.team_buttons}, nội dung: '{team_text}')", file=sys.stderr)
    sys.exit(1)
history = [' '.join(t.split()) for t in parser.team_history]
if len(history) != 1 or 'bạn qua dashboard: mac-dinh → doi-thu' not in history[0]:
    print(f"Lỗi: lịch sử đổi đội hình không phải một dòng 'bạn qua dashboard: mac-dinh → doi-thu' (nhận được: {history})", file=sys.stderr)
    sys.exit(1)
if 'ab-act-change_team' in parser.strip_buttons or 'Đổi đội hình' in parser.strip_text:
    print("Lỗi: 'Đổi đội hình' không được nằm trong dải hành động", file=sys.stderr)
    sys.exit(1)
print("OK task-team")

if len(parser.ledger_rows) != 2:
    print(f"Lỗi: Số lượng data-ledger-row là {len(parser.ledger_rows)}, kỳ vọng 2", file=sys.stderr)
    sys.exit(1)

selected_rows = [r for r in parser.ledger_rows if r.get('aria-current') == 'true']
if len(selected_rows) != 1:
    print(f"Lỗi: Số data-ledger-row có aria-current='true' là {len(selected_rows)}, kỳ vọng 1", file=sys.stderr)
    sys.exit(1)

if '2 lượt chạy ·' not in parser.ledger_total_text:
    print(f"Lỗi: data-ledger-total không chứa '2 lượt chạy ·' (nội dung: '{parser.ledger_total_text}')", file=sys.stderr)
    sys.exit(1)

if 'Vòng 1 · plan · claude' not in ' '.join(parser.ledger_text.split()):
    print(f"Lỗi: sổ lượt chạy không có nhãn 'Vòng 1 · plan · claude' (nội dung: '{' '.join(parser.ledger_text.split())}')", file=sys.stderr)
    sys.exit(1)

if 'Sổ lượt chạy' not in parser.full_text:
    print("Lỗi: DOM không chứa 'Sổ lượt chạy'", file=sys.stderr)
    sys.exit(1)

if 'Chu kỳ 1 · plan đầu tiên' not in parser.full_text:
    print("Lỗi: DOM không chứa 'Chu kỳ 1 · plan đầu tiên'", file=sys.stderr)
    sys.exit(1)

print("Ledger DOM OK")
EOF
    then
      echo "--- 40 dòng đầu của DOM ---" >&2
      head -n 40 "$DOM_OUT" >&2 || true
      echo "--- serve stderr ---" >&2
      if [[ -f "$TMP/serve.err" ]]; then
        cat "$TMP/serve.err" >&2
      fi
      exit 1
    fi
  fi

  # UI fix 3: the File column of the plan's Thay đổi table keeps a path on one
  # line (whitespace-nowrap, scrolls inside overflow-x-auto), never per letter.
  if [[ "$s" == "task" ]]; then
    FILE_TDS="$(grep -o '<td[^>]*x-text="c.file"[^>]*>[^<]*</td>' "$DOM_OUT" || true)"
    if [[ -z "$FILE_TDS" ]] || ! grep -qF '>src/app.py</td>' <<< "$FILE_TDS" \
      || grep -qF 'break-all' <<< "$FILE_TDS" || grep -vqF 'whitespace-nowrap' <<< "$FILE_TDS"; then
      echo "Lỗi: Cột File trong bảng Thay đổi vẫn ngắt theo từng chữ (cần whitespace-nowrap, không break-all): ${FILE_TDS:-không có ô File}" >&2
      exit 1
    fi
    echo "Changes table DOM OK"
  fi

  # DOM assertions for the Giao việc, Dự án and Tổng quan screens (Phase 5.2).
  if [[ "$s" == "assign" || "$s" == "projects" || "$s" == "overview" || "$s" == "board" || "$s" == "settings" || "$s" == "stats" || "$s" == "telegram" ]]; then
    if ! python3 - "$DOM_OUT" "$s" "$URL" <<'EOF'
import sys
from html.parser import HTMLParser

VOID_TAGS = {'area', 'base', 'br', 'col', 'embed', 'hr', 'img', 'input', 'link', 'meta', 'source', 'track', 'wbr'}


class Node:
    def __init__(self, tag, attrs, parent):
        self.tag = tag
        self.attrs = dict(attrs)
        self.parent = parent
        self.content = []

    def hidden(self):
        n = self
        while n is not None:
            if 'display: none' in (n.attrs.get('style') or ''):
                return True
            n = n.parent
        return False

    def _own_text(self):
        # Visible text below this node (subtrees hidden by x-show are skipped).
        if 'display: none' in (self.attrs.get('style') or ''):
            return ''
        return ''.join(c if isinstance(c, str) else c._own_text() for c in self.content)

    def text(self):
        return '' if self.hidden() else self._own_text()

    def walk(self):
        for c in self.content:
            if isinstance(c, Node):
                yield c
                yield from c.walk()

    def find(self, pred):
        return [n for n in self.walk() if pred(n)]


class TreeParser(HTMLParser):
    def __init__(self):
        super().__init__()
        self.root = Node('#root', [], None)
        self.cur = self.root
        self.template_depth = 0

    def handle_starttag(self, tag, attrs):
        if tag == 'template':
            self.template_depth += 1
            return
        if self.template_depth > 0:
            return
        node = Node(tag, attrs, self.cur)
        self.cur.content.append(node)
        if tag not in VOID_TAGS:
            self.cur = node

    def handle_startendtag(self, tag, attrs):
        if tag == 'template' or self.template_depth > 0:
            return
        self.cur.content.append(Node(tag, attrs, self.cur))

    def handle_endtag(self, tag):
        if tag == 'template':
            self.template_depth = max(0, self.template_depth - 1)
            return
        if self.template_depth > 0 or tag in VOID_TAGS:
            return
        n = self.cur
        while n is not None and n.tag != tag:
            n = n.parent
        if n is not None and n.parent is not None:
            self.cur = n.parent

    def handle_data(self, data):
        if self.template_depth > 0:
            return
        self.cur.content.append(data)


def fail(msg):
    print(f"Lỗi: {msg}", file=sys.stderr)
    sys.exit(1)


dom_path, screen = sys.argv[1], sys.argv[2]
with open(dom_path, 'r', encoding='utf-8') as f:
    html = f.read()
parser = TreeParser()
parser.feed(html)
root = parser.root
full_text = root.text()

if screen == 'assign':
    fieldsets = []
    for fs in root.find(lambda n: n.tag == 'fieldset'):
        legends = [c for c in fs.content if isinstance(c, Node) and c.tag == 'legend']
        if legends and legends[0].text().strip() == 'Quy trình':
            fieldsets.append(fs)
    if len(fieldsets) != 1:
        fail(f"Số fieldset có legend 'Quy trình' là {len(fieldsets)}, kỳ vọng 1")
    fs = fieldsets[0]
    radios = fs.find(lambda n: n.tag == 'input' and n.attrs.get('type') == 'radio' and n.attrs.get('name') == 'workflow')
    if len(radios) < 2:
        fail(f"Số input[type=radio][name=workflow] là {len(radios)}, kỳ vọng ít nhất 2")
    checked = [r for r in radios if 'checked' in r.attrs]
    if len(checked) != 1:
        fail(f"Số radio quy trình đang chọn là {len(checked)}, kỳ vọng 1")
    if checked[0].attrs.get('value') != 'mac-dinh':
        fail(f"Radio đang chọn có value '{checked[0].attrs.get('value')}', kỳ vọng 'mac-dinh'")
    fs_text = fs.text()
    for needle in ('plan ⇄︎ review → Bạn → code ⇄︎ review → QA ⇄︎ sửa', '3 vòng · 60 phút · $20', 'nhanh',
                   'Quy trình này tự chạy autopilot và luôn dừng ở bước bạn duyệt plan.'):
        if needle not in fs_text:
            fail(f"Fieldset Quy trình không chứa '{needle}' (nội dung: '{' '.join(fs_text.split())[:400]}')")
    if 'Đang tải quy trình...' in fs_text:
        fail("Fieldset Quy trình vẫn hiện 'Đang tải quy trình...'")
    if 'Không tải được danh sách quy trình' in fs_text:
        fail("Fieldset Quy trình hiện callout lỗi tải danh sách")
    print("Assign DOM OK")

    # 06-15: the Đội hình fieldset, a separate kind from Quy trình (UI-SPEC C1).
    def collapse_ws(t):
        return ' '.join(t.split())
    team_sets = []
    for cand in root.find(lambda n: n.tag == 'fieldset'):
        legends = [c for c in cand.content if isinstance(c, Node) and c.tag == 'legend']
        if legends and legends[0].text().strip() == 'Đội hình':
            team_sets.append(cand)
    if len(team_sets) != 1:
        fail(f"Số fieldset có legend 'Đội hình' là {len(team_sets)}, kỳ vọng 1")
    tfs = team_sets[0]
    if tfs is fs:
        fail("Fieldset Đội hình trùng với fieldset Quy trình")
    if 'Đội hình chọn engine chạy từng vai trò.' not in collapse_ws(tfs.text()):
        fail("Fieldset Đội hình thiếu 'Đội hình chọn engine chạy từng vai trò.'")
    options = tfs.find(lambda n: 'data-team-option' in n.attrs)
    option_names = sorted(o.attrs.get('data-team-option') for o in options)
    for name in ('mac-dinh', 'tiet-kiem', 'doi-thu'):
        if name not in option_names:
            fail(f"Thiếu phần tử data-team-option=\"{name}\" (có: {option_names})")
    team_radios = tfs.find(lambda n: n.tag == 'input' and n.attrs.get('type') == 'radio' and n.attrs.get('name') == 'team')
    if len(team_radios) < 3:
        fail(f"Số input[type=radio][name=team] là {len(team_radios)}, kỳ vọng ít nhất 3")
    team_checked = [r for r in team_radios if 'checked' in r.attrs]
    # No project is chosen in a plain DOM dump, so the default is mac-dinh; the project
    # default (doi-thu) is asserted below from the same payload the picker reads.
    if len(team_checked) != 1 or team_checked[0].attrs.get('value') != 'mac-dinh':
        fail(f"Radio đội hình đang chọn là {[r.attrs.get('value') for r in team_checked]}, kỳ vọng ['mac-dinh']")
    tfs_text = tfs.text()
    if 'Đang tải đội hình...' in tfs_text:
        fail("Fieldset Đội hình vẫn hiện 'Đang tải đội hình...'")
    if 'Không tải được danh sách đội hình' in tfs_text:
        fail("Fieldset Đội hình hiện callout lỗi tải danh sách")
    if 'teams.json có lỗi' in tfs_text:
        fail("Fieldset Đội hình hiện callout 'teams.json có lỗi' dù file hợp lệ")
    import json, urllib.parse, urllib.request
    base = urllib.parse.urlsplit(sys.argv[3])
    token = urllib.parse.parse_qs(base.query).get('token', [''])[0]
    req = urllib.request.Request(
        f"{base.scheme}://{base.netloc}/api/teams",
        headers={'Cookie': f"agent_brain_token={token}"},
    )
    with urllib.request.urlopen(req, timeout=10) as resp:
        teams_payload = json.load(resp)
    pd = teams_payload.get('project_defaults', {}).get('web-ban-hang')
    if not pd or pd.get('effective') != 'doi-thu' or pd.get('team') != 'doi-thu':
        fail(f"project_defaults['web-ban-hang'] là {pd}, kỳ vọng team và effective 'doi-thu'")
    print("OK assign-team")

    # 06-24: option readiness lines come from the engine list; chưa kiểm is a line, not a block.
    import os
    opt_text = {o.attrs.get('data-team-option'): collapse_ws(o.text()) for o in options}
    if '! deepseek: thiếu key' not in opt_text.get('tiet-kiem', ''):
        fail(f"Đội hình tiet-kiem thiếu dòng '! deepseek: thiếu key' (nội dung: '{opt_text.get('tiet-kiem')}')")
    if '! gw-chua: chưa kiểm' not in opt_text.get('thu-cong', ''):
        fail(f"Đội hình thu-cong thiếu dòng '! gw-chua: chưa kiểm' (nội dung: '{opt_text.get('thu-cong')}')")
    req = urllib.request.Request(
        f"{base.scheme}://{base.netloc}/api/engines",
        headers={'Cookie': f"agent_brain_token={token}"},
    )
    with urllib.request.urlopen(req, timeout=10) as resp:
        raw_engines = resp.read().decode('utf-8')
    engines_payload = json.loads(raw_engines)
    ids = [e.get('name') for e in engines_payload.get('engines', [])]
    want = ['claude', 'deepseek', 'gw-thu', 'gw-chua', 'claude-max', 'agy', 'dsh', 'người']
    if ids != want:
        fail(f"/api/engines trả {ids}, kỳ vọng {want}")
    states = {e['name']: e['readiness'] for e in engines_payload['engines']}
    if states.get('gw-chua') != 'chua_kiem' or states.get('deepseek') != 'thieu_key' or states.get('dsh') != 'thieu_lenh' or states.get('gw-thu') != 'san_sang':
        fail(f"Trạng thái engine không như kỳ vọng: {states}")
    for var in ('SMOKE_GW_KEY', 'SMOKE_GW2_KEY'):
        value = os.environ.get(var, '')
        if not value or value in html or value in raw_engines:
            fail(f"Giá trị của {var} xuất hiện trong DOM Giao việc hoặc trong /api/engines (hoặc không được đặt)")
    print("OK assign-engines")
    sys.exit(0)

if screen == 'projects':
    for needle in ('Quy trình', 'Lấy từ workflows.json và 2 quy trình có sẵn.', 'mac-dinh', 'nhanh',
                   'Quy trình mặc định: mac-dinh', 'Profile và quy trình mặc định chỉ đổi trong projects_map.json'):
        if needle not in full_text:
            fail(f"Màn hình Dự án không chứa '{needle}'")
    if 'Profile chỉ đổi trong projects_map.json' in full_text:
        fail("Màn hình Dự án còn chú thích cũ 'Profile chỉ đổi trong projects_map.json'")
    cards = root.find(lambda n: 'data-workflow-card' in n.attrs)
    if len(cards) != 1:
        fail(f"Số phần tử data-workflow-card là {len(cards)}, kỳ vọng 1")
    card = cards[0]
    if card.hidden():
        fail("Card Quy trình đang bị ẩn")
    buttons = [b for b in card.find(lambda n: n.tag == 'button') if not b.hidden() and b.text().strip() == 'Đọc lại']
    if len(buttons) < 1:
        fail("Card Quy trình không có nút 'Đọc lại' đang hiện")
    items = [i for i in card.find(lambda n: 'data-workflow-item' in n.attrs) if not i.hidden()]
    names = [' '.join(i.text().split()) for i in items]
    if len(items) < 2 or not names[0].startswith('mac-dinh') or not names[1].startswith('nhanh'):
        fail(f"Danh sách quy trình không bắt đầu bằng mac-dinh, nhanh (nhận được: {names})")
    if 'có sẵn' not in names[0] or '3 vòng · 60 phút · $20' not in names[0]:
        fail(f"Mục mac-dinh thiếu chip 'có sẵn' hoặc giới hạn (nội dung: '{names[0]}')")
    if 'Mặc định của: Web bán hàng' not in names[0]:
        fail(f"Mục mac-dinh thiếu 'Mặc định của: Web bán hàng' (nội dung: '{names[0]}')")
    if 'Không tải được quy trình' in card.text():
        fail("Card Quy trình hiện callout lỗi tải")
    print("Projects DOM OK")
    sys.exit(0)

if screen == 'overview':
    groups = [g for g in root.find(lambda n: n.attrs.get('data-overview-group') == 'waiting')]
    if len(groups) != 1:
        fail(f"Số nhóm data-overview-group='waiting' là {len(groups)}, kỳ vọng 1")
    waiting = groups[0]
    if waiting.hidden():
        fail("Nhóm Chờ bạn đang bị ẩn")
    if 'Duyệt plan hoặc xử lý chỗ autopilot dừng' not in waiting.text():
        fail("Nhóm Chờ bạn không có gợi ý 'Duyệt plan hoặc xử lý chỗ autopilot dừng'")
    items = {i.attrs.get('data-task-id'): i for i in waiting.find(lambda n: 'data-task-id' in n.attrs)}
    for tid in ('1', '2'):
        if tid not in items:
            fail(f"Nhóm Chờ bạn không có task {tid} (có: {sorted(items)})")

    def reason_lines(item):
        return [r for r in item.find(lambda n: 'data-reason-line' in n.attrs) if not r.hidden()]

    stopped = items['2']
    stopped_text = ' '.join(stopped.text().split())
    if 'Chờ bạn · hết vòng' not in stopped_text:
        fail(f"Task 2 không có banner 'Chờ bạn · hết vòng' (nội dung: '{stopped_text}')")
    lines = reason_lines(stopped)
    if len(lines) != 1 or lines[0].text().strip() != 'Review code vẫn yêu cầu sửa lại sau 3 / 3 vòng.':
        fail(f"Task 2 không có dòng lý do 'Review code vẫn yêu cầu sửa lại sau 3 / 3 vòng.' (nhận được: {[l.text() for l in lines]})")
    gate = items['1']
    gate_text = ' '.join(gate.text().split())
    if 'Chờ duyệt plan' not in gate_text:
        fail(f"Task 1 không có banner 'Chờ duyệt plan' (nội dung: '{gate_text}')")
    if reason_lines(gate):
        fail("Task 1 (chờ duyệt plan) không được có dòng lý do")
    # No autopilot driver runs in the fixture: every Autopilot badge stays hidden.
    badges = root.find(lambda n: n.attrs.get('aria-label') == 'Autopilot: đang chạy')
    if not badges:
        fail("Thiếu badge aria-label='Autopilot: đang chạy' trong task item")
    if [b for b in badges if not b.hidden()]:
        fail("Badge Autopilot hiện dù không có autopilot nào đang chạy")
    print("Overview DOM OK")

    # 06-18: the Đang chạy hint, a person's step in the blue cho_nguoi tint, a quota stop under Chờ bạn.
    if 'Agent hoặc nhân sự đang làm việc' not in ' '.join(full_text.split()):
        fail("Nhóm Đang chạy thiếu gợi ý 'Agent hoặc nhân sự đang làm việc'")
    running_groups = [g for g in root.find(lambda n: n.attrs.get('data-overview-group') == 'running')]
    if len(running_groups) != 1:
        fail(f"Số nhóm data-overview-group='running' là {len(running_groups)}, kỳ vọng 1")
    running_items = {i.attrs.get('data-task-id'): i for i in running_groups[0].find(lambda n: 'data-task-id' in n.attrs)}
    if '4' not in running_items:
        fail(f"Nhóm Đang chạy không có task 4 đang chờ người (có: {sorted(running_items)})")
    human_item = running_items['4']
    human_text = ' '.join(human_item.text().split())
    if 'Chờ Hà làm' not in human_text:
        fail(f"Task 4 không có banner 'Chờ Hà làm' (nội dung: '{human_text}')")
    banners = [b for b in human_item.find(lambda n: 'data-task-banner' in n.attrs) if not b.hidden()]
    if len(banners) != 1 or 'text-blue-700' not in banners[0].attrs.get('class', ''):
        fail(f"Banner của task 4 thiếu tông xanh cho_nguoi (class: {[b.attrs.get('class') for b in banners]})")
    glyphs = [g for g in banners[0].find(lambda n: n.attrs.get('aria-hidden') == 'true') if not g.hidden()]
    if len(glyphs) != 1 or glyphs[0].text().strip() != '◐︎':
        fail(f"Banner của task 4 thiếu biểu tượng ◐ (nhận được: {[g.text() for g in glyphs]})")
    if 'animate-pulse' in banners[0].attrs.get('class', '') or 'animate-pulse' in glyphs[0].attrs.get('class', ''):
        fail("Biểu tượng ◐ của người làm không được nhấp nháy")
    quota_item = items.get('3')
    if quota_item is None:
        fail(f"Nhóm Chờ bạn không có task 3 dừng vì hết quota (có: {sorted(items)})")
    if 'Chờ bạn · hết quota' not in ' '.join(quota_item.text().split()):
        fail(f"Task 3 không có banner 'Chờ bạn · hết quota' (nội dung: '{' '.join(quota_item.text().split())}')")
    quota_lines = reason_lines(quota_item)
    if len(quota_lines) != 1 or 'trên engine deepseek dừng vì hết quota' not in quota_lines[0].text():
        fail(f"Task 3 không có dòng lý do quota (nhận được: {[l.text() for l in quota_lines]})")
    print("OK overview-human")
    sys.exit(0)

if screen == 'board':
    def collapse(txt):
        return ' '.join(txt.split())

    cards = {n.attrs['data-item-key']: n for n in root.find(lambda n: 'data-item-key' in n.attrs) if not n.hidden()}
    if sorted(cards) != ['02-01', '02-02', '02-03']:
        fail(f"Các thẻ hiện ra là {sorted(cards)}, kỳ vọng 02-01, 02-02, 02-03")
    if 'sẵn sàng' not in collapse(cards['02-01'].text()):
        fail(f"Thẻ 02-01 thiếu 'sẵn sàng' (nội dung: '{collapse(cards['02-01'].text())}')")
    for key in ('02-02', '02-03'):
        if 'bị chặn' not in collapse(cards[key].text()):
            fail(f"Thẻ {key} thiếu 'bị chặn' (nội dung: '{collapse(cards[key].text())}')")
    edges = {n.attrs['data-edge'] for n in root.find(lambda n: 'data-edge' in n.attrs)}
    for e in ('02-01__02-02', '02-01__02-03'):
        if e not in edges:
            fail(f"Thiếu cạnh {e} (có: {sorted(edges)})")
    waves = sorted(n.attrs['data-wave-head'] for n in root.find(lambda n: 'data-wave-head' in n.attrs))
    if waves != ['0', '1']:
        fail(f"Các wave là {waves}, kỳ vọng ['0', '1']")
    legends = root.find(lambda n: 'data-board-legend' in n.attrs)
    if len(legends) != 1:
        fail(f"Số phần tử data-board-legend là {len(legends)}, kỳ vọng 1")
    legend_text = collapse(legends[0].text())
    for needle in ('xong 0', 'sẵn sàng 1', 'bị chặn 2'):
        if needle not in legend_text:
            fail(f"Chú giải thiếu '{needle}' (nội dung: '{legend_text}')")
    for attr in ('data-board-empty', 'data-board-error'):
        els = root.find(lambda n, a=attr: a in n.attrs)
        if len(els) != 1:
            fail(f"Số phần tử {attr} là {len(els)}, kỳ vọng 1")
        if not els[0].hidden():
            fail(f"{attr} đang hiện")

    # Toolbar, switch, nav link and legend extras (plan 05.5-02 Task 3).
    nav = root.find(lambda n: n.tag == 'a' and n.attrs.get('href') == '#/board')
    if len(nav) != 1 or nav[0].attrs.get('aria-current') != 'page':
        fail(f"Liên kết menu #/board thiếu aria-current=page (nhận được: {[n.attrs.get('aria-current') for n in nav]})")
    switches = root.find(lambda n: n.attrs.get('role') == 'switch')
    if len(switches) != 1 or switches[0].attrs.get('aria-checked') != 'true':
        fail(f"Công tắc role=switch thiếu aria-checked=true (nhận được: {[n.attrs.get('aria-checked') for n in switches]})")
    if 'Thu gọn mục đã xong' not in collapse(switches[0].text()):
        fail(f"Công tắc thiếu nhãn 'Thu gọn mục đã xong' (nội dung: '{collapse(switches[0].text())}')")
    for bid in ('board-reload', 'board-collapse'):
        if not root.find(lambda n, b=bid: n.tag == 'button' and n.attrs.get('id') == b):
            fail(f"Thiếu nút id={bid}")
    for sid in ('board-project', 'board-epic'):
        if not root.find(lambda n, b=sid: n.tag == 'select' and n.attrs.get('id') == b):
            fail(f"Thiếu select id={sid}")
    epic_select = root.find(lambda n: n.tag == 'select' and n.attrs.get('id') == 'board-epic')[0]
    options = {o.attrs.get('value'): collapse(o.text()) for o in epic_select.find(lambda n: n.tag == 'option')}
    if options.get('all') != 'Mọi epic':
        fail(f"Select board-epic thiếu option all = 'Mọi epic' (có: {options})")
    if '02' not in options:
        fail(f"Select board-epic thiếu option 02 (có: {options})")
    ms = root.find(lambda n: n.tag == 'select' and n.attrs.get('id') == 'board-milestone')
    if len(ms) != 1 or not ms[0].hidden():
        fail("Select board-milestone phải bị ẩn khi dự án chỉ có một milestone")
    warns = root.find(lambda n: 'data-board-warnings' in n.attrs)
    if len(warns) != 1 or not warns[0].hidden():
        fail("Hộp data-board-warnings phải bị ẩn khi không có cảnh báo")
    for needle in ('Tổng 3 · 2 wave', 'đường găng'):
        if needle not in legend_text:
            fail(f"Chú giải thiếu '{needle}' (nội dung: '{legend_text}')")

    # Side panel and critical path on screen (plan 05.5-05): the panel starts hidden,
    # the critical edge is a yellow line, the other edge stays slate, no yellow ring on cards.
    panels = root.find(lambda n: 'data-board-panel' in n.attrs)
    if len(panels) != 1 or not panels[0].hidden():
        fail(f"Bảng bên data-board-panel phải có đúng 1 phần tử và bị ẩn khi chưa chọn thẻ (số phần tử: {len(panels)})")

    def edge_path_class(edge_id):
        svgs = root.find(lambda n, e=edge_id: n.tag == 'svg' and n.attrs.get('data-edge') == e)
        if len(svgs) != 1:
            fail(f"Số svg data-edge={edge_id} là {len(svgs)}, kỳ vọng 1")
        paths = svgs[0].find(lambda n: n.tag == 'path')
        if len(paths) != 1:
            fail(f"svg data-edge={edge_id} có {len(paths)} path, kỳ vọng 1")
        return paths[0].attrs.get('class') or ''

    crit_class = edge_path_class('02-01__02-02')
    if 'stroke-yellow-600' not in crit_class:
        fail(f"Cạnh đường găng 02-01__02-02 thiếu stroke-yellow-600 (class: '{crit_class}')")
    plain_class = edge_path_class('02-01__02-03')
    if 'stroke-slate-400' not in plain_class or 'stroke-yellow' in plain_class:
        fail(f"Cạnh thường 02-01__02-03 phải là stroke-slate-400, không vàng (class: '{plain_class}')")
    for key, node in cards.items():
        if 'ring-yellow' in (node.attrs.get('class') or ''):
            fail(f"Thẻ {key} có vòng vàng ring-yellow; đường găng chỉ vẽ bằng đường kẻ")
    print("Board DOM OK")
    sys.exit(0)

if screen == 'settings':
    def collapse(t):
        return ' '.join(t.split())

    screens = root.find(lambda n: n.attrs.get('data-screen') == 'settings')
    if len(screens) != 1:
        fail(f"Số phần tử data-screen=\"settings\" là {len(screens)}, kỳ vọng 1")
    if screens[0].attrs.get('data-loaded') != 'yes':
        fail(f"data-loaded của màn hình Cài đặt là '{screens[0].attrs.get('data-loaded')}', kỳ vọng 'yes'")
    rows = {n.attrs.get('data-team-row') for n in root.find(lambda n: 'data-team-row' in n.attrs)}
    for name in ('mac-dinh', 'tiet-kiem', 'doi-thu'):
        if name not in rows:
            fail(f"Thiếu hàng đội hình data-team-row=\"{name}\" (có: {sorted(rows)})")
    if 'Đội hình và quy trình là hai danh sách riêng' not in collapse(full_text):
        fail("Màn hình Cài đặt thiếu quy tắc đặt tên 'Đội hình và quy trình là hai danh sách riêng'")
    navs = root.find(lambda n: n.tag == 'nav' and n.attrs.get('aria-label') == 'Mục cài đặt')
    if len(navs) != 1:
        fail(f"Số nav 'Mục cài đặt' là {len(navs)}, kỳ vọng 1")
    sub = [collapse(b.text()) for b in navs[0].find(lambda n: n.tag == 'button')]
    if sub != ['Đội hình', 'Quy trình', 'Nhân sự', 'Engine']:
        fail(f"Các nút của nav 'Mục cài đặt' là {sub}, kỳ vọng ['Đội hình', 'Quy trình', 'Nhân sự', 'Engine']")
    current = [collapse(b.text()) for b in navs[0].find(lambda n: n.tag == 'button' and n.attrs.get('aria-current') == 'true')]
    if current != ['Đội hình']:
        fail(f"Nút nav đang chọn là {current}, kỳ vọng ['Đội hình']")
    main_nav = root.find(lambda n: n.tag == 'nav' and n.attrs.get('aria-label') == 'Điều hướng chính')
    if len(main_nav) != 1:
        fail(f"Số nav 'Điều hướng chính' là {len(main_nav)}, kỳ vọng 1")
    links = main_nav[0].find(lambda n: n.tag == 'a')
    if len(links) != 7:
        fail(f"Menu chính có {len(links)} mục, kỳ vọng 7")
    link = [n for n in links if n.attrs.get('href') == '#/settings']
    if len(link) != 1 or link[0].attrs.get('aria-current') != 'page':
        fail(f"Liên kết menu #/settings thiếu aria-current=page (nhận được: {[n.attrs.get('aria-current') for n in link]})")
    if collapse(link[0].text()) != 'Cài đặt':
        fail(f"Liên kết menu #/settings có nhãn '{collapse(link[0].text())}', kỳ vọng 'Cài đặt'")
    print("OK settings-shell")

    # 06-24: the Engine section lists the configured engines in config order (D-30, D-44).
    import os
    eng_rows = root.find(lambda n: 'data-engine-row' in n.attrs)
    eng_names = [n.attrs.get('data-engine-row') for n in eng_rows]
    want = ['claude', 'deepseek', 'gw-thu', 'gw-chua', 'claude-max', 'agy', 'dsh', 'người']
    if eng_names != want:
        fail(f"Hàng engine là {eng_names}, kỳ vọng {want}")
    # The Engine section sits behind the sub-nav (x-show), so read each row's own visible text
    # (hidden parents are not a reason to skip the row; hidden children still are).
    eng_text = {n.attrs.get('data-engine-row'): collapse(n._own_text()) for n in eng_rows}
    expect = {
        'claude': ['có sẵn', 'sẵn sàng'],
        'deepseek': ['anthropic', 'thiếu key', 'Chưa đặt biến môi trường SMOKE_DEEPSEEK_KEY.', 'Biến key: SMOKE_DEEPSEEK_KEY'],
        'gw-thu': ['Cổng thử nghiệm của smoke', 'sẵn sàng', 'Bằng chứng E-09: 2026-10-28 smoke E-09', 'Biến key: SMOKE_GW_KEY'],
        'gw-chua': ['chưa kiểm', 'Endpoint chưa có bằng chứng E-09', 'Biến key: SMOKE_GW2_KEY'],
        'claude-max': ['Profile cố định: personal', 'claude chạy trên máy bằng tài khoản Claude của profile dự án.'],
        'agy': ['agy', 'sẵn sàng'],
        'dsh': ['dsh', 'thiếu lệnh', 'Không tìm thấy lệnh', 'Container: dsh-smoke', 'Không đếm token (chỉ văn bản).'],
        'người': ['có sẵn', 'nhân sự'],
    }
    for name, needles in expect.items():
        for needle in needles:
            if needle not in eng_text[name]:
                fail(f"Hàng engine '{name}' không chứa '{needle}' (nội dung: '{eng_text[name]}')")
    if 'chưa kiểm' in eng_text['gw-thu'] or 'Endpoint chưa có bằng chứng' in eng_text['gw-thu']:
        fail(f"Hàng engine 'gw-thu' đã có bằng chứng nhưng vẫn hiện chưa kiểm (nội dung: '{eng_text['gw-thu']}')")
    for var in ('SMOKE_GW_KEY', 'SMOKE_GW2_KEY'):
        value = os.environ.get(var, '')
        if not value or value in html:
            fail(f"Giá trị của {var} xuất hiện trong DOM Cài đặt (hoặc không được đặt)")
    print("OK settings-engines")
    sys.exit(0)

if screen == 'telegram':
    import os

    def collapse(t):
        return ' '.join(t.split())

    # 06.2-02: the smoke sandbox has no telegram.json, so the bot is off.
    screens = root.find(lambda n: n.attrs.get('data-screen') == 'telegram')
    if len(screens) != 1:
        fail(f"Số phần tử data-screen=\"telegram\" là {len(screens)}, kỳ vọng 1")
    if screens[0].attrs.get('data-loaded') != 'yes':
        fail(f"data-loaded của màn hình Telegram là '{screens[0].attrs.get('data-loaded')}', kỳ vọng 'yes'")
    badge = root.find(lambda n: 'data-telegram-state' in n.attrs)
    if len(badge) != 1 or not collapse(badge[0].text()).endswith('Chưa cấu hình'):
        fail(f"Huy hiệu trạng thái Telegram là {[collapse(b.text()) for b in badge]}, kỳ vọng kết thúc bằng 'Chưa cấu hình'")
    off_text = root.find(lambda n: 'data-telegram-off-text' in n.attrs)
    want = 'Bot Telegram chưa bật. Chạy "agent-brain telegram setup" trong terminal để ghép cặp bot, rồi khởi động lại agent-brain serve.'
    if len(off_text) != 1 or collapse(off_text[0].text()) != want:
        fail(f"Câu hướng dẫn khi bot tắt là {[collapse(t.text()) for t in off_text]}, kỳ vọng '{want}'")
    cmd = root.find(lambda n: 'data-telegram-command' in n.attrs)
    if len(cmd) != 1 or collapse(cmd[0].text()) != 'agent-brain telegram setup':
        fail(f"Lệnh ghép cặp hiện là {[collapse(c.text()) for c in cmd]}, kỳ vọng 'agent-brain telegram setup'")
    if root.find(lambda n: 'data-telegram-message' in n.attrs and not n.hidden()):
        fail("Màn hình Telegram hiện hộp cảnh báo dù bot chỉ chưa được bật")
    rows = {n.attrs.get('data-telegram-row'): collapse(n.text()) for n in root.find(lambda n: 'data-telegram-row' in n.attrs)}
    if sorted(rows) != ['config']:
        fail(f"Khi bot tắt chỉ được có hàng 'config', nhận được {sorted(rows)}")
    want_path = os.path.join(os.environ['XDG_CONFIG_HOME'], 'agent-brain', 'telegram.json')
    if want_path not in rows['config'] or 'Tệp cấu hình' not in rows['config']:
        fail(f"Hàng Tệp cấu hình là '{rows['config']}', kỳ vọng chứa '{want_path}'")
    if root.find(lambda n: n.tag in ('input', 'textarea', 'form')):
        fail("Màn hình Telegram không được có ô nhập hoặc form")
    main_nav = root.find(lambda n: n.tag == 'nav' and n.attrs.get('aria-label') == 'Điều hướng chính')
    if len(main_nav) != 1:
        fail(f"Số nav 'Điều hướng chính' là {len(main_nav)}, kỳ vọng 1")
    link = [n for n in main_nav[0].find(lambda n: n.tag == 'a') if n.attrs.get('href') == '#/telegram']
    if len(link) != 1 or link[0].attrs.get('aria-current') != 'page':
        fail(f"Liên kết menu #/telegram thiếu aria-current=page (nhận được: {[n.attrs.get('aria-current') for n in link]})")
    if collapse(link[0].text()) != 'Telegram':
        fail(f"Liên kết menu #/telegram có nhãn '{collapse(link[0].text())}', kỳ vọng 'Telegram'")
    print("OK telegram-off")
    sys.exit(0)

if screen == 'stats':
    def collapse(t):
        return ' '.join(t.split())

    def card_of(key):
        found = [c for c in root.find(lambda n: n.attrs.get('data-stats-card') == key)]
        if len(found) != 1:
            fail(f"Số card data-stats-card='{key}' là {len(found)}, kỳ vọng 1")
        return found[0]

    # 06-18: Theo engine (task 5 has Dev runs on claude, deepseek without a price, and a person).
    eng = card_of('engines')
    if 'Theo engine' not in collapse(eng.text()):
        fail("Thiếu tiêu đề card 'Theo engine'")
    rows = {}
    for r in eng.find(lambda n: 'data-engine-row' in n.attrs):
        txt = collapse(r.text())
        rows[txt.split(' ')[0]] = (r, txt)
    for name in ('claude', 'deepseek', 'người'):
        if name not in rows:
            fail(f"Thiếu hàng engine '{name}' (có: {sorted(rows)})")
    if 'chưa có giá · 3 run' not in rows['deepseek'][1] or '≈ $0' in rows['deepseek'][1]:
        fail(f"Hàng deepseek phải là 'chưa có giá · 3 run' (nội dung: '{rows['deepseek'][1]}')")
    if 'vào 70.000 · ra 16.000' not in rows['deepseek'][1]:
        fail(f"Hàng deepseek thiếu 'vào 70.000 · ra 16.000' (nội dung: '{rows['deepseek'][1]}')")
    if '≈ $' not in rows['claude'][1] or ' run' not in rows['claude'][1]:
        fail(f"Hàng claude thiếu '≈ $… · n run' (nội dung: '{rows['claude'][1]}')")
    if 'không có token' not in rows['người'][1] or '1 lượt do người làm' not in rows['người'][1]:
        fail(f"Hàng người thiếu 'không có token' / '1 lượt do người làm' (nội dung: '{rows['người'][1]}')")
    if [b for b in rows['người'][0].find(lambda n: n.tag == 'div' and 'bg-indigo-500' in n.attrs.get('class', '')) if not b.hidden()]:
        fail("Hàng người không được có thanh")
    if 'Engine chưa có giá hiện "chưa có giá", không tính vào Chi phí ≈ ở trên.' not in collapse(eng.text()):
        fail("Thiếu chú thích giá dưới card 'Theo engine'")
    print("OK stats-engines")

    # 06-18: QA rate by the Dev engine (claude 0 / 1, deepseek 1 / 2, người 0 / 1).
    rate = card_of('qa-rate')
    rtext = collapse(rate.text())
    if 'Tỉ lệ QA kết luận Cần sửa theo engine của Dev' not in rtext:
        fail("Thiếu tiêu đề card 'Tỉ lệ QA kết luận Cần sửa theo engine của Dev'")
    for needle in ('Chỉ tính các lần QA kết luận Đạt hoặc Cần sửa, gắn với engine của lần Dev được kiểm tra.',
                   'Tỉ lệ chỉ đáng tin khi có nhiều lần QA; xem số lần ở cuối mỗi dòng.'):
        if needle not in rtext:
            fail(f"Card tỉ lệ QA thiếu '{needle}'")
    if 'Chưa có lần QA nào trong khoảng này.' in rtext:
        fail("Card tỉ lệ QA hiện trạng thái rỗng dù có lần QA")
    rrows = {}
    for r in rate.find(lambda n: 'data-rate-row' in n.attrs):
        txt = collapse(r.text())
        rrows[txt.split(' ')[0]] = txt
    for name, want in (('deepseek', '50% 1 / 2 lần QA'), ('claude', '0% 0 / 1 lần QA'), ('người', '0% 0 / 1 lần QA')):
        if want not in rrows.get(name, ''):
            fail(f"Hàng QA của {name} phải là '{want}' (có: {rrows})")
    sys.exit(0)

print(f"Không có kiểm tra DOM cho {screen}")
EOF
    then
      echo "--- 40 dòng đầu của DOM ---" >&2
      head -n 40 "$DOM_OUT" >&2 || true
      echo "--- serve stderr ---" >&2
      if [[ -f "$TMP/serve.err" ]]; then
        cat "$TMP/serve.err" >&2
      fi
      exit 1
    fi
  fi
done

echo "Smoke OK: ${SCREENS[*]}"

if [[ "$ENABLE_KEYS" == "true" ]]; then
  if ! node -e 'process.exit(typeof WebSocket==="function"?0:1)' 2>/dev/null; then
    echo "Cần Node ≥ 22 (có WebSocket) cho --keys" >&2
    exit 1
  fi

  python3 - "$AGENT_BRAIN_DB" <<'EOF'
import sys, sqlite3
db_path = sys.argv[1]
conn = sqlite3.connect(db_path, timeout=5)
cur = conn.cursor()
cur.execute("SELECT project_id FROM tasks WHERE id = 1")
row = cur.fetchone()
if not row:
    print("Lỗi seeding: Không tìm thấy task 1", file=sys.stderr)
    sys.exit(1)
project_id = row[0]

cur.execute("SELECT MIN(started_at) FROM runs WHERE task_id = 1")
row = cur.fetchone()
min_start = row[0] if (row and row[0] is not None) else 1000000000000

roles = ['lead', 'dev', 'qa']
count = 0
# 24 runs + the lead and Review plan runs of the fixture = 26 ledger rows
for i in range(24):
    role = roles[i % 3]
    started_at = min_start - (24 - i) * 60000
    ended_at = started_at + 30000
    outcome = 'qa:pass' if role == 'qa' else None
    cur.execute("""
        INSERT INTO runs (task_id, project_id, role, status, profile, usage_source, cost_usd, num_turns, started_at, ended_at, outcome)
        VALUES (?, ?, ?, 'succeeded', 'work', 'result', 0.10, 3, ?, ?, ?)
    """, (1, project_id, role, started_at, ended_at, outcome))
    count += cur.rowcount

conn.commit()
conn.close()

if count != 24:
    print(f"Lỗi seeding: Số lượt chạy chèn vào là {count}, kỳ vọng 24", file=sys.stderr)
    sys.exit(1)
EOF

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

  if ! AB_SMOKE_REPO="$REPO_DIR" node "$REPO_ROOT/scripts/dashboard-keys.mjs" "$PORT" "$URL" > "$TMP/keys.out" 2>&1; then
    if [[ -f "$TMP/keys.out" ]]; then
      cat "$TMP/keys.out" >&2
    fi
    if [[ -f "$TMP/serve.err" ]]; then
      cat "$TMP/serve.err" >&2
    fi
    exit 1
  fi

  stop_chrome
  cat "$TMP/keys.out"
  echo "Keys OK: combobox, gate-stop, confirm-dialog, raw-dialog, ledger, layout, gate-buttons, return-dialog, change-team, task-quota, task-human, picker, layout-5.2, board-keys, board-layout"
fi
