#!/usr/bin/env bash
# Nghiệm thu trực tiếp bảng công việc (#/board) bằng binary thật trong môi trường cô lập.
#
#   bash scripts/board-live-check.sh          kiểm tra tự động, in "Board live check OK"
#   bash scripts/board-live-check.sh --keep   như trên, rồi giữ serve chạy để mở bảng trong trình duyệt
#
# Cô lập: HOME, XDG_CONFIG_HOME, XDG_DATA_HOME, AGENT_BRAIN_DB, AGENT_BRAIN_CONFIG nằm trong một
# thư mục tạm. Repo này được đăng ký chỉ để ĐỌC (serve chỉ đọc .planning/ qua bộ quét của bảng);
# bảng mẫu SHOP (14 plan của bản thiết kế) được tạo trong thư mục tạm. Không đụng tới
# ~/.config/agent-brain, brain.db thật hay serve cổng 7437 của bạn. Dừng serve bằng đúng PID đã ghi.
set -euo pipefail

SCRIPT_DIR="$(cd -P "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -P "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

KEEP=false
for arg in "$@"; do
  case "$arg" in
    --keep) KEEP=true ;;
    *)
      echo "Tham số không hợp lệ: $arg (chỉ có --keep)" >&2
      exit 2
      ;;
  esac
done

BIN="${AB_BIN:-$REPO_ROOT/agent-brain/target/debug/agent-brain}"
if [[ ! -x "$BIN" ]]; then
  echo "Thiếu $BIN — chạy: cargo build --manifest-path agent-brain/Cargo.toml --offline --locked" >&2
  exit 1
fi
for tool in python3 curl; do
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

export HOME="$TMP/home"
export XDG_CONFIG_HOME="$TMP/xdg_config"
export XDG_DATA_HOME="$TMP/xdg_data"
export AGENT_BRAIN_DB="$TMP/xdg_data/agent-brain/brain.db"
export AGENT_BRAIN_CONFIG="$TMP/brain_config.json"
export GIT_CONFIG_GLOBAL="$TMP/gitconfig"
export GIT_CONFIG_NOSYSTEM=1

mkdir -p "$TMP/home" "$TMP/xdg_config/agent-brain" "$TMP/xdg_data/agent-brain" \
  "$TMP/runs" "$TMP/personal_cfg" "$TMP/out"

cat << 'EOF' > "$TMP/gitconfig"
[user]
  name = Live Check
  email = live-check@example.com
[commit]
  gpgsign = false
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

# Bảng mẫu SHOP: dữ liệu của bản thiết kế (SHOP-01 … SHOP-14) thành plan GSD 01-01 … 01-14.
# Mỗi dòng: số plan | wave trong front matter (wave tính + 1) | phụ thuộc | xong (x hoặc rỗng) | tên.
SHOP_DIR="$TMP/repos/bang-mau"
SHOP_PHASE_DIR="$SHOP_DIR/.planning/phases/01-xuat-bao-cao"
mkdir -p "$SHOP_PHASE_DIR"
SHOP_ROWS=(
  "01|1||x|Chốt định dạng CSV"
  "02|1||x|Quy ước tên file và mã hoá"
  "03|1|||Chọn thư viện ghi CSV"
  "04|2|01||Truy vấn đơn hàng theo khoảng ngày"
  "05|2|01,02||Định dạng tiền tệ và ngày tháng"
  "06|1|||Quyền xuất cho quản trị viên"
  "07|3|03,04||Bộ ghi CSV theo luồng"
  "08|4|07,06||API GET /admin/orders/export"
  "09|5|08||Nút Xuất CSV ở trang đơn hàng"
  "10|4|07||Giới hạn 50.000 dòng và cảnh báo"
  "11|4|05,07||Kiểm thử dữ liệu tiếng Việt có dấu"
  "12|5|08||Ghi nhật ký lượt xuất"
  "13|6|09,11||Kiểm thử đầu-cuối luồng xuất"
  "14|6|09||Hướng dẫn sử dụng cho cửa hàng"
)

{
  echo "# Roadmap: Bảng mẫu SHOP"
  echo
  echo "## Phases"
  echo
  echo "- [ ] **Phase 1: Xuất báo cáo đơn hàng** - xuất CSV cho cửa hàng"
  echo
  echo "## Phase Details"
  echo
  echo "### Phase 1: Xuất báo cáo đơn hàng"
  echo "**Depends on**: Nothing (first phase)"
  echo "Plans:"
  for row in "${SHOP_ROWS[@]}"; do
    IFS='|' read -r num _wave _deps done_flag title <<< "$row"
    box="[ ]"
    if [[ "$done_flag" == "x" ]]; then box="[x]"; fi
    echo "- $box 01-$num-PLAN.md — $title"
  done
} > "$SHOP_DIR/.planning/ROADMAP.md"

cat << 'EOF' > "$SHOP_DIR/.planning/STATE.md"
---
current_phase: "01"
current_phase_name: Xuất báo cáo đơn hàng
---

# Project State
EOF

for row in "${SHOP_ROWS[@]}"; do
  IFS='|' read -r num wave deps done_flag title <<< "$row"
  dep_list=""
  if [[ -n "$deps" ]]; then
    IFS=',' read -r -a dep_nums <<< "$deps"
    for d in "${dep_nums[@]}"; do
      if [[ -n "$dep_list" ]]; then dep_list="$dep_list, "; fi
      dep_list="$dep_list\"01-$d\""
    done
  fi
  cat << PLAN_EOF > "$SHOP_PHASE_DIR/01-$num-PLAN.md"
---
phase: 01-xuat-bao-cao
plan: $num
type: execute
wave: $wave
depends_on: [$dep_list]
files_modified:
  - src/export.py
autonomous: true
requirements: [SHOP-$num]
---

<objective>
$title.
</objective>
PLAN_EOF
  if [[ "$done_flag" == "x" ]]; then
    printf '# Summary\n' > "$SHOP_PHASE_DIR/01-$num-SUMMARY.md"
  fi
done

cat << EOF > "$TMP/xdg_config/agent-brain/projects_map.json"
{
  "scan_roots": [],
  "projects": [
    {
      "id": "quangvy-orchestration",
      "path": "$REPO_ROOT",
      "name_vi": "Điều phối Quang Vy",
      "aliases": [],
      "type": "code",
      "repo": null,
      "profile": "work",
      "missing": false
    },
    {
      "id": "bang-mau",
      "path": "$SHOP_DIR",
      "name_vi": "Bảng mẫu SHOP",
      "aliases": [],
      "type": "code",
      "repo": null,
      "profile": "work",
      "missing": false
    }
  ]
}
EOF

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
    echo "LỖI live: serve dừng sớm (PID $SERVE_PID)" >&2
    cat "$TMP/serve.err" >&2 || true
    exit 1
  fi
  sleep 0.1
done
if [[ -z "$URL" ]]; then
  echo "LỖI live: không thấy URL của serve sau 10 giây" >&2
  cat "$TMP/serve.err" >&2 || true
  exit 1
fi

PORT="$(printf '%s' "$URL" | sed -E 's#^http://127\.0\.0\.1:([0-9]+)/.*#\1#')"
TOKEN="$(printf '%s' "$URL" | sed -E 's#.*[?]token=([^&# ]+).*#\1#')"
if [[ ! "$PORT" =~ ^[0-9]+$ || -z "$TOKEN" || "$TOKEN" == "$URL" ]]; then
  echo "LỖI live: không đọc được cổng hoặc token từ dòng khởi động của serve" >&2
  exit 1
fi

# GET /api/board: cookie của phiên đi vào curl qua stdin (-K -), không bao giờ nằm trên dòng lệnh.
# fetch TÊN TRUY_VẤN → $TMP/out/TÊN.json và $TMP/out/TÊN.code ("<http_code> <giây>").
fetch() {
  local name="$1" query="$2"
  printf 'header = "Cookie: agent_brain_token=%s"\n' "$TOKEN" |
    curl -sS -K - --max-time 10 \
      -o "$TMP/out/$name.json" \
      -w '%{http_code} %{time_total}\n' \
      "http://127.0.0.1:$PORT/api/board?$query" > "$TMP/out/$name.code"
}

fetch repo_all "project=quangvy-orchestration&epic=all"
fetch repo_default "project=quangvy-orchestration"
fetch shop_all "project=bang-mau&epic=all"

python3 -I - "$TMP/out" "$REPO_ROOT/.planning/STATE.md" << 'PYEOF'
import json
import os
import re
import sys

sys.stdout.reconfigure(encoding="utf-8")
out_dir, state_path = sys.argv[1], sys.argv[2]


def fail(msg):
    print("LỖI live: " + msg)
    sys.exit(1)


def load(name):
    with open(os.path.join(out_dir, name + ".code"), encoding="utf-8") as f:
        parts = f.read().split()
    if len(parts) != 2:
        fail(f"{name}: curl không ghi được mã HTTP và thời gian")
    status, secs = parts[0], float(parts[1])
    if status != "200":
        fail(f"{name}: HTTP {status}, cần 200")
    if secs >= 2.0:
        fail(f"{name}: trả lời sau {secs:.2f}s, cần dưới 2.0s")
    with open(os.path.join(out_dir, name + ".json"), encoding="utf-8") as f:
        data = json.load(f)
    print(f"LIVE OK: {name}: HTTP 200 trong {secs:.2f}s")
    return data


def board_ok(label, d):
    if d.get("state") != "ok":
        fail(f"{label}: state = {d.get('state')!r} ({d.get('reason')!r}), cần 'ok'")
    if d.get("truncated") is not False:
        fail(f"{label}: bảng bị cắt bớt (truncated = {d.get('truncated')!r})")
    warnings = d.get("warnings")
    if warnings != []:
        for w in warnings or []:
            print(f"  cảnh báo: {w.get('path')}: {w.get('message')}")
        fail(f"{label}: warnings không rỗng ({len(warnings or [])} dòng)")
    print(f"LIVE OK: {label}: state ok, không cắt bớt, không cảnh báo, {len(d['items'])} hạng mục")


def by_key(d):
    return {it["key"]: it for it in d["items"]}


def critical_checks(label, d):
    items = by_key(d)
    crit_items = [it["key"] for it in d["items"] if it["critical"]]
    for key in crit_items:
        if items[key]["status"] == "xong":
            fail(f"{label}: hạng mục găng {key} đã xong")
    crit_edges = [(e["from"], e["to"]) for e in d["edges"] if e["critical"]]
    for a, b in crit_edges:
        for end in (a, b):
            it = items.get(end)
            if it is None:
                fail(f"{label}: cạnh găng {a} → {b} có đầu {end} không có trong hạng mục")
            if it["status"] == "xong":
                fail(f"{label}: cạnh găng {a} → {b} có đầu {end} đã xong")
    return crit_items, crit_edges


# 1. Repo này, mọi epic.
repo = load("repo_all")
board_ok("repo (epic=all)", repo)
items = by_key(repo)
p54 = [it for it in repo["items"] if it["key"].startswith("05.4-")]
if len(p54) != 8:
    fail(f"repo: có {len(p54)} plan 05.4-*, cần 8")
for it in p54:
    if it["status"] != "xong":
        fail(f"repo: {it['key']} có status {it['status']!r}, cần 'xong'")
print("LIVE OK: repo: tám plan 05.4-* đều xong")
for key in ("04", "06"):
    it = items.get(key)
    if it is None:
        fail(f"repo: không có hạng mục thay thế {key} cho phase chưa có plan")
    if it["placeholder"] is not True:
        fail(f"repo: {key} không phải hạng mục thay thế (placeholder = {it['placeholder']!r})")
    if it["status"] == "xong":
        fail(f"repo: hạng mục thay thế {key} có status 'xong'")
print("LIVE OK: repo: phase 04 và 06 là hạng mục thay thế, chưa xong")
if not any(e["from"] == "05.4-08" and e["to"] == "05.5-01" and e["kind"] == "phase" for e in repo["edges"]):
    fail("repo: thiếu cạnh phase 05.4-08 → 05.5-01 (ROADMAP: Depends on Phase 05.4)")
print("LIVE OK: repo: cạnh phase 05.4-08 → 05.5-01")
crit_items, crit_edges = critical_checks("repo", repo)
print(f"LIVE OK: repo: đường găng {crit_items or 'rỗng'}, {len(crit_edges)} cạnh găng, mọi đầu chưa xong")

# 2. Repo này, chế độ xem mặc định = current_phase của STATE.md.
default = load("repo_default")
current = None
with open(state_path, encoding="utf-8") as f:
    lines = f.read().split("\n")
if lines and lines[0].strip() == "---":
    for line in lines[1:40]:
        if line.strip() == "---":
            break
        m = re.match(r'^current_phase:\s*"?([0-9][0-9.]*)"?\s*$', line)
        if m:
            current = m.group(1)
            break
if current is None:
    print("LIVE OK: repo (mặc định): STATE.md không có current_phase, bỏ qua so sánh epic mặc định")
else:
    parts = current.split(".")
    padded = f"{int(parts[0]):02d}" + "".join("." + str(int(p)) for p in parts[1:])
    epic_ids = [e["id"] for e in default["epics"]]
    if padded not in epic_ids:
        print(f"LIVE OK: repo (mặc định): current_phase {padded} không nằm trong epics, bỏ qua so sánh")
    else:
        if default["default_epic"] != padded or default["epic"] != padded:
            fail(
                f"repo (mặc định): epic = {default['epic']!r}, default_epic = {default['default_epic']!r}, "
                f"cần {padded!r} (current_phase của STATE.md)"
            )
        print(f"LIVE OK: repo (mặc định): epic mặc định {padded} = current_phase của STATE.md")

# 3. Bảng mẫu SHOP.
shop = load("shop_all")
board_ok("bang-mau (epic=all)", shop)
sitems = by_key(shop)
waves = {k.split("-")[1]: v["wave"] for k, v in sitems.items()}
exp_waves = {
    "01": 0, "02": 0, "03": 0, "04": 1, "05": 1, "06": 0, "07": 2,
    "08": 3, "09": 4, "10": 3, "11": 3, "12": 4, "13": 5, "14": 5,
}
if len(sitems) != 14 or waves != exp_waves:
    fail(f"bang-mau: wave = {waves}, cần {exp_waves}")
print("LIVE OK: bang-mau: wave 01:0 02:0 03:0 04:1 05:1 06:0 07:2 08:3 09:4 10:3 11:3 12:4 13:5 14:5")
done = {"01-01", "01-02"}
ready = {"01-03", "01-04", "01-05", "01-06"}
for key, it in sitems.items():
    want = "xong" if key in done else ("san_sang" if key in ready else "bi_chan")
    if it["status"] != want:
        fail(f"bang-mau: {key} có status {it['status']!r}, cần {want!r}")
print("LIVE OK: bang-mau: hai plan xong, bốn sẵn sàng, tám bị chặn")
s_items, s_edges = critical_checks("bang-mau", shop)
if set(s_items) != {"01-04", "01-07", "01-08", "01-09", "01-13"}:
    fail(f"bang-mau: hạng mục găng = {sorted(s_items)}, cần 01-04, 01-07, 01-08, 01-09, 01-13")
exp_edges = {("01-04", "01-07"), ("01-07", "01-08"), ("01-08", "01-09"), ("01-09", "01-13")}
if set(s_edges) != exp_edges or len(s_edges) != 4:
    fail(f"bang-mau: cạnh găng = {sorted(s_edges)}, cần {sorted(exp_edges)}")
print("LIVE OK: bang-mau: đường găng 01-04 → 01-07 → 01-08 → 01-09 → 01-13 (4 cạnh)")
PYEOF

echo "Board live check OK"

if [[ "$KEEP" == "true" ]]; then
  echo "${URL}#/board"
  echo "Mở URL trên trong trình duyệt; Ctrl+C để dừng serve."
  wait "$SERVE_PID" || true
fi
