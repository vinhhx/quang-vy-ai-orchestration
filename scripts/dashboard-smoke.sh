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

# Task 1 (default mac-dinh workflow): one Review plan run that approved the
# plan, and autopilot paused at the Bạn gate.
python3 - "$AGENT_BRAIN_DB" <<'PYEOF'
import sys, sqlite3
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

# Task 1 stays the checked-out task (its actions, e.g. Chạy lại Lead for the
# keys confirm-dialog check, need its branch checked out).
git -C "$REPO_DIR" checkout task/sua-app >/dev/null 2>&1

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
  # data-screen of the rendered screen (task-stop is the task screen of task 2)
  SCREEN_NAME="$s"
  case "$s" in
    overview) HASH="#/" ;;
    assign) HASH="#/assign" ;;
    task) HASH="#/task/1" ;;
    task-stop) HASH="#/task/2"; SCREEN_NAME="task" ;;
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

  if [[ "$s" == "task" || "$s" == "task-stop" ]]; then
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

if 'vướng mắc / cần sửa / sửa lại' not in parser.full_text:
    print("Lỗi: chú thích thứ 7 không chứa 'vướng mắc / cần sửa / sửa lại'", file=sys.stderr)
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

if len(parser.ledger_rows) != 2:
    print(f"Lỗi: Số lượng data-ledger-row là {len(parser.ledger_rows)}, kỳ vọng 2", file=sys.stderr)
    sys.exit(1)

selected_rows = [r for r in parser.ledger_rows if r.get('aria-current') == 'true']
if len(selected_rows) != 1:
    print(f"Lỗi: Số data-ledger-row có aria-current='true' là {len(selected_rows)}, kỳ vọng 1", file=sys.stderr)
    sys.exit(1)

if '2 lượt chạy claude' not in parser.ledger_total_text:
    print(f"Lỗi: data-ledger-total không chứa '2 lượt chạy claude' (nội dung: '{parser.ledger_total_text}')", file=sys.stderr)
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

  # DOM assertions for the Giao việc, Dự án and Tổng quan screens (Phase 5.2).
  if [[ "$s" == "assign" || "$s" == "projects" || "$s" == "overview" ]]; then
    if ! python3 - "$DOM_OUT" "$s" <<'EOF'
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
  cat "$TMP/keys.out"
  echo "Keys OK: combobox, gate-stop, confirm-dialog, raw-dialog, ledger, layout, gate-buttons, return-dialog, picker, layout-5.2"
fi
