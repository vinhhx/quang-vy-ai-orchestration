#!/usr/bin/env bash
# Nghiệm thu Phase 7 (nhật ký Obsidian) bằng binary thật, trong môi trường cô lập.
#
#   bash scripts/phase7-live-check.sh                    kiểm tra tự động đường xuất CLI, in "Phase 7 live check OK"
#                                                        (AB_LIVE_DEBUG=1: in thêm đầu ra các bước khi có lỗi)
#   bash scripts/phase7-live-check.sh --schema-dry-run   chỉ chạy thử nâng lược đồ 12 trên BẢN SAO của
#                                                        brain.db thật (không mở file thật bằng SQLite)
#
# Chế độ tự động: HOME, XDG_*, AGENT_BRAIN_DB, AGENT_BRAIN_CONFIG và cả "vault" (một repo bare cục bộ)
# nằm trong một thư mục tạm; dữ liệu là dữ liệu mẫu tự tạo. Không đụng tới ~/.config/agent-brain,
# ~/.local/share/agent-brain, brain.db thật, binary đã cài hay serve đang chạy.
# Chế độ --schema-dry-run: đường dẫn thật chỉ được `ls`, `sha256sum` và `cp -p` đọc.
set -euo pipefail

SCRIPT_DIR="$(cd -P "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -P "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

DRY_RUN=false
for arg in "$@"; do
  case "$arg" in
    --schema-dry-run) DRY_RUN=true ;;
    *)
      echo "Tham số không hợp lệ: $arg (chỉ có --schema-dry-run)" >&2
      exit 2
      ;;
  esac
done

BIN="${AB_BIN:-$REPO_ROOT/agent-brain/target/debug/agent-brain}"
if [[ ! -x "$BIN" ]]; then
  echo "Thiếu $BIN — chạy: cargo build --manifest-path agent-brain/Cargo.toml --offline --locked" >&2
  exit 1
fi
for tool in python3 git; do
  if ! command -v "$tool" > /dev/null 2>&1; then
    echo "Thiếu công cụ $tool trong PATH" >&2
    exit 1
  fi
done

TMP="$(mktemp -d)"

cleanup() {
  local exit_code=$?
  if [[ -d "$TMP" ]]; then
    rm -rf "$TMP"
  fi
  exit "$exit_code"
}
trap cleanup EXIT INT TERM

# ---------------------------------------------------------------------------------------------
# schema_dry_run <đường dẫn brain.db>: chạy thử nâng lược đồ trên bản sao. Đường dẫn gốc chỉ được
# `ls`, `sha256sum` và `cp -p` đọc; không bao giờ đưa cho SQLite hay cho binary.
# ---------------------------------------------------------------------------------------------
schema_dry_run() {
  local real_db="$1" real_dir db_name copy dry
  if [[ ! -f "$real_db" ]]; then
    echo "LỖI dry run: không thấy database: $real_db" >&2
    return 1
  fi
  real_dir="$(dirname "$real_db")"
  db_name="$(basename "$real_db")"
  dry="$TMP/dry"
  rm -rf "$dry"

  # Danh sách thư mục và sha256 của thư mục dữ liệu gốc. Dòng `..` (thư mục cha) không thuộc thư mục
  # dữ liệu nên không dùng để so sánh.
  snapshot_real() {
    {
      ls -la --time-style=full-iso "$real_dir" | awk '$NF != ".."'
      for f in "$real_db" "$real_db-wal" "$real_db-shm"; do
        if [[ -f "$f" ]]; then
          sha256sum "$f"
        fi
      done
    }
  }
  snapshot_real > "$TMP/before.txt"

  mkdir -p "$dry/copy" "$dry/home" "$dry/xdg_config" "$dry/xdg_data"
  copy="$dry/copy/$db_name"
  cp -p "$real_db" "$copy"
  for suffix in -wal -shm; do
    if [[ -f "$real_db$suffix" ]]; then
      cp -p "$real_db$suffix" "$copy$suffix"
    fi
  done

  # Đọc user_version, integrity_check, số dòng, số vi phạm khóa ngoại và số bảng journal của BẢN SAO.
  read_copy() {
    python3 -I - "$copy" << 'PYEOF'
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
print("fk_violations=%d" % c.execute("SELECT COUNT(*) FROM pragma_foreign_key_check").fetchone()[0])
print(
    "journal_tables=%d"
    % c.execute(
        "SELECT COUNT(*) FROM sqlite_master WHERE name IN ('journal_state', 'journal_mail', 'journal_fts')"
    ).fetchone()[0]
)
c.close()
PYEOF
  }
  local before_state after_state
  if ! before_state="$(read_copy)"; then
    echo "$before_state"
    return 1
  fi
  local old_version
  old_version="$(printf '%s\n' "$before_state" | sed -n 's/^user_version=//p')"
  echo "Bản sao: $(printf '%s' "$before_state" | tr '\n' ' ')"

  cp "$REPO_ROOT/agent-brain/assets/config.default.json" "$dry/brain_config.json"
  # Mở bản sao bằng binary mới: lệnh `stats` chạy các bước nâng lược đồ trên bản sao đó.
  if ! HOME="$dry/home" XDG_CONFIG_HOME="$dry/xdg_config" XDG_DATA_HOME="$dry/xdg_data" \
    AGENT_BRAIN_DB="$copy" AGENT_BRAIN_CONFIG="$dry/brain_config.json" \
    "$BIN" stats --json > /dev/null 2> "$dry/stats.err"; then
    echo "LỖI dry run: binary mới không mở được bản sao" >&2
    head -c 2000 "$dry/stats.err" >&2 || true
    return 1
  fi

  if ! after_state="$(read_copy)"; then
    echo "$after_state"
    return 1
  fi
  local new_version
  new_version="$(printf '%s\n' "$after_state" | sed -n 's/^user_version=//p')"
  if [[ "$new_version" != "12" ]]; then
    echo "LỖI dry run: sau khi mở bằng binary mới user_version = $new_version, cần 12" >&2
    return 1
  fi
  if ! printf '%s\n' "$after_state" | grep -qx 'integrity=ok'; then
    echo "LỖI dry run: PRAGMA integrity_check của bản sao đã nâng không phải ok" >&2
    return 1
  fi
  local key a b
  for key in projects tasks runs events; do
    b="$(printf '%s\n' "$before_state" | sed -n "s/^$key=//p")"
    a="$(printf '%s\n' "$after_state" | sed -n "s/^$key=//p")"
    if [[ "$a" != "$b" ]]; then
      echo "LỖI dry run: số dòng bảng $key đổi từ $b thành $a sau khi nâng" >&2
      return 1
    fi
  done
  local fk_before fk_after
  fk_before="$(printf '%s\n' "$before_state" | sed -n 's/^fk_violations=//p')"
  fk_after="$(printf '%s\n' "$after_state" | sed -n 's/^fk_violations=//p')"
  if ((fk_after > fk_before)); then
    echo "LỖI dry run: số vi phạm khóa ngoại tăng từ $fk_before lên $fk_after sau khi nâng" >&2
    return 1
  fi
  if [[ "$(printf '%s\n' "$after_state" | sed -n 's/^journal_tables=//p')" != "3" ]]; then
    echo "LỖI dry run: bản sao đã nâng thiếu bảng journal_state, journal_mail hoặc journal_fts" >&2
    return 1
  fi

  snapshot_real > "$TMP/after.txt"
  if ! diff -u "$TMP/before.txt" "$TMP/after.txt" > "$TMP/dir.diff"; then
    echo "LỖI dry run: thư mục dữ liệu thật đã thay đổi" >&2
    cat "$TMP/dir.diff" >&2
    return 1
  fi

  echo "Schema v12 dry run OK (từ v$old_version)"
  echo "fk_violations_before=$fk_before after=$fk_after"
  echo "real_dir_unchanged=yes"
  rm -rf "$dry"
}

if [[ "$DRY_RUN" == "true" ]]; then
  REAL_DB="${AGENT_BRAIN_DB:-$HOME/.local/share/agent-brain/brain.db}"
  echo "Dừng agent-brain serve và các run trước khi chạy — bản chép của một database đang được ghi có thể không nhất quán."
  schema_dry_run "$REAL_DB"
  exit 0
fi

# ---------------------------------------------------------------------------------------------
# Chế độ tự động: mọi thứ nằm trong $TMP.
# ---------------------------------------------------------------------------------------------
die() {
  [[ -n "${AB_LIVE_DEBUG:-}" ]] && { for f in "$TMP"/*.out; do echo "--- $f"; cat "$f"; done; } >&2
  echo "LỖI live check: $*" >&2
  exit 1
}

export HOME="$TMP/home"
export XDG_CONFIG_HOME="$TMP/xdg_config"
export XDG_DATA_HOME="$TMP/xdg_data"
export AGENT_BRAIN_DB="$TMP/xdg_data/agent-brain/brain.db"
export AGENT_BRAIN_CONFIG="$TMP/brain_config.json"
export GIT_CONFIG_GLOBAL="$TMP/gitconfig"
export GIT_CONFIG_NOSYSTEM=1

mkdir -p "$HOME" "$XDG_CONFIG_HOME/agent-brain" "$XDG_DATA_HOME/agent-brain" "$TMP/repos"

cat << 'EOF' > "$GIT_CONFIG_GLOBAL"
[user]
  name = Live Check
  email = live-check@example.com
[commit]
  gpgsign = false
[init]
  defaultBranch = main
EOF

# 2026-01-15 20:00 giờ +07:00; ngày tính theo utc_offset_minutes = 420 của cấu hình bên dưới.
NOW_MS="$(python3 -I -c '
import datetime
tz = datetime.timezone(datetime.timedelta(hours=7))
print(int(datetime.datetime(2026, 1, 15, 20, 0, 0, tzinfo=tz).timestamp() * 1000))
')"
export AGENT_BRAIN_JOURNAL_NOW_MS="$NOW_MS"

VAULT="$TMP/vault.git"
git init -q --bare -b main "$VAULT"
VAULT_A="$TMP/vault-a.git"
git init -q --bare -b main "$VAULT_A"
CLONE_A="$TMP/clone-a"

PROJECT_REPO="$TMP/repos/du-an-mau"
mkdir -p "$PROJECT_REPO"
printf 'noi dung mau\n' > "$PROJECT_REPO/readme.txt"
git -C "$PROJECT_REPO" init -q -b main
git -C "$PROJECT_REPO" add readme.txt
git -C "$PROJECT_REPO" commit -q -m init

# write_map [id nữa]: projects_map.json với du-an-mau (nhóm video) và, nếu có, một dự án thứ hai.
write_map() {
  python3 -I - "$XDG_CONFIG_HOME/agent-brain/projects_map.json" "$PROJECT_REPO" "${1:-}" << 'PYEOF'
import json
import sys

path, repo, extra = sys.argv[1], sys.argv[2], sys.argv[3]


def entry(pid, journal_group=None):
    e = {
        "id": pid,
        "path": repo,
        "name_vi": pid,
        "aliases": [],
        "type": "code",
        "repo": None,
        "profile": "work",
        "missing": False,
        "workflow": "nhanh",
    }
    if journal_group:
        e["journal_group"] = journal_group
    return e


projects = [entry("du-an-mau", "video")]
if extra:
    projects.append(entry(extra))
with open(path, "w", encoding="utf-8") as f:
    json.dump({"scan_roots": [], "projects": projects}, f, ensure_ascii=False)
PYEOF
}
write_map

# config.json = cấu hình mặc định + mục journal trỏ vào repo bare cục bộ.
python3 -I - "$REPO_ROOT/agent-brain/assets/config.default.json" "$AGENT_BRAIN_CONFIG" "$TMP/brain_config_no_journal.json" "$VAULT" "$TMP/brain_config_a.json" "$VAULT_A" "$CLONE_A" << 'PYEOF'
import json
import sys

src, dst, dst_no_journal, vault, dst_a, vault_a, clone_a = sys.argv[1:8]
with open(src, encoding="utf-8") as f:
    cfg = json.load(f)
with open(dst_no_journal, "w", encoding="utf-8") as f:
    json.dump(cfg, f, ensure_ascii=False)
web = "https://github.com/example-owner/example-vault"
cfg["journal"] = {"repo_url": vault, "utc_offset_minutes": 420, "web_base_url": web}
with open(dst, "w", encoding="utf-8") as f:
    json.dump(cfg, f, ensure_ascii=False)
# Kiểm tra A dùng vault và bản clone riêng để không ảnh hưởng các kiểm tra sau.
cfg["journal"] = {"repo_url": vault_a, "clone_dir": clone_a, "utc_offset_minutes": 420, "web_base_url": web}
with open(dst_a, "w", encoding="utf-8") as f:
    json.dump(cfg, f, ensure_ascii=False)
PYEOF

# Tạo brain.db (lệnh `stats` mở DB, chạy migration và đồng bộ dự án từ projects_map.json).
"$BIN" stats --json > /dev/null 2> "$TMP/stats.err" || {
  head -c 2000 "$TMP/stats.err" >&2 || true
  die "không tạo được brain.db tạm"
}

# seed_task <dự án> <slug task> <yêu cầu>: một task đã xong ngày 2026-01-15 (lead, dev, qa:pass).
seed_task() {
  python3 -I - "$AGENT_BRAIN_DB" "$1" "$2" "$3" "$NOW_MS" << 'PYEOF'
import datetime
import sqlite3
import sys

db, project, slug, title, now_ms = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], int(sys.argv[5])
tz = datetime.timezone(datetime.timedelta(hours=7))


def ms(h, m):
    return int(datetime.datetime(2026, 1, 15, h, m, 0, tzinfo=tz).timestamp() * 1000)


c = sqlite3.connect(db)
row = c.execute("SELECT id FROM projects WHERE slug = ?", (project,)).fetchone()
if row is None:
    print("LỖI live check: không thấy dự án %s trong DB tạm" % project)
    sys.exit(1)
pid = row[0]
cur = c.execute(
    "INSERT INTO tasks (project_id, slug, title, branch, status, created_at, updated_at) VALUES (?, ?, ?, ?, 'open', ?, ?)",
    (pid, slug, title, "task/" + slug, ms(8, 55), ms(9, 40)),
)
tid = cur.lastrowid
for role, outcome, start, end in (
    ("lead", None, ms(9, 0), ms(9, 5)),
    ("dev", None, ms(9, 10), ms(9, 30)),
    ("qa", "qa:pass", ms(9, 31), ms(9, 40)),
):
    c.execute(
        "INSERT INTO runs (task_id, project_id, role, engine, profile, status, outcome, started_at, ended_at, round)"
        " VALUES (?, ?, ?, 'claude', 'work', 'succeeded', ?, ?, ?, 1)",
        (tid, pid, role, outcome, start, end),
    )
c.commit()
c.close()
PYEOF
}

# dbq SQL: in cột đầu của dòng đầu, chỉ đọc brain.db của môi trường tạm này.
dbq() {
  python3 -I -c '
import sqlite3
import sys

c = sqlite3.connect("file:" + sys.argv[1] + "?mode=ro", uri=True)
row = c.execute(sys.argv[2]).fetchone()
print("" if row is None or row[0] is None else row[0])
' "$AGENT_BRAIN_DB" "$1"
}

remote_commit_count() {
  if git -C "$VAULT" rev-parse --verify --quiet refs/heads/main > /dev/null; then
    git -C "$VAULT" rev-list --count refs/heads/main
  else
    echo 0
  fi
}

fresh_clone() {
  local dir="$TMP/check-$1"
  rm -rf "$dir"
  git clone -q --branch main "$VAULT" "$dir" > /dev/null 2>&1
  echo "$dir"
}

seed_task du-an-mau dung-video-mau "Dựng video mẫu cho sản phẩm thử"
CLONE="$XDG_DATA_HOME/agent-brain/journal"

# Kiểm tra A (vault và bản clone riêng): --no-push ghi tệp nhưng không commit, repo bare chưa có commit.
if ! AGENT_BRAIN_CONFIG="$TMP/brain_config_a.json" "$BIN" journal export --no-push > "$TMP/a.out" 2>&1; then
  cat "$TMP/a.out" >&2
  die "A: journal export --no-push thoát khác 0"
fi
if git -C "$VAULT_A" rev-parse --verify --quiet refs/heads/main > /dev/null; then
  die "A: --no-push đã đẩy commit lên repo bare"
fi
if ! git -C "$CLONE_A" -c core.quotepath=false status --porcelain --untracked-files=all | grep -q 'Today.md'; then
  die "A: --no-push không để lại tệp sinh ra chưa commit trong bản clone"
fi
if [[ -n "$(git -C "$CLONE_A" log --oneline 2> /dev/null || true)" ]]; then
  die "A: --no-push đã tạo commit trong bản clone"
fi
echo "LIVE OK: A --no-push chỉ ghi tệp, không commit, không đẩy"

# Kiểm tra B: export đẩy đúng một commit với năm loại tệp, không có tệp tuần.
if ! "$BIN" journal export > "$TMP/b.out" 2>&1; then
  cat "$TMP/b.out" >&2
  die "B: journal export thoát khác 0"
fi
if [[ "$(remote_commit_count)" != "1" ]]; then
  die "B: repo bare có $(remote_commit_count) commit, cần đúng 1"
fi
B_DIR="$(fresh_clone b)"
if [[ "$(git -C "$B_DIR" log --format=%an)" != "agent-brain journal" ]]; then
  die "B: tác giả commit không phải 'agent-brain journal'"
fi
B_FILES="$(git -C "$B_DIR" -c core.quotepath=false ls-files)"
for f in "Today.md" "Nhật ký/2026/01/2026-01-15.md" "Nhật ký/2026/01/2026-01.md" "Nhật ký/2026/2026.md" "Dự án/du-an-mau.md"; do
  if ! printf '%s\n' "$B_FILES" | grep -qxF "$f"; then
    die "B: thiếu tệp $f trong vault"
  fi
done
if printf '%s\n' "$B_FILES" | grep -q 'Tuần'; then
  die "B: vault có tệp tuần (không được có)"
fi
echo "LIVE OK: B export đẩy một commit của 'agent-brain journal' với đủ năm loại tệp, không có tệp tuần"

# Kiểm tra C: export lần hai không tạo commit mới.
if ! "$BIN" journal export > "$TMP/c.out" 2>&1; then
  cat "$TMP/c.out" >&2
  die "C: journal export lần hai thoát khác 0"
fi
if [[ "$(remote_commit_count)" != "1" ]]; then
  die "C: export lần hai tạo thêm commit"
fi
echo "LIVE OK: C export lần hai không tạo commit"

# Kiểm tra D: yêu cầu có đường dẫn, URL nội bộ, khóa và liên kết nhúng không để lại dấu vết trong vault.
seed_task du-an-mau tac-vu-nhay-cam 'Sửa lỗi cấu hình tại /home/someone/app/config/database.php và gọi https://intranet.example.local/admin?token=abc123 với DEEPSEEK_API_KEY=abcd1234efgh5678 xem ![[ghi chu rieng]]'
if ! "$BIN" journal export > "$TMP/d.out" 2>&1; then
  cat "$TMP/d.out" >&2
  die "D: journal export với yêu cầu nhạy cảm thoát khác 0"
fi
if [[ "$(remote_commit_count)" != "2" ]]; then
  die "D: repo bare có $(remote_commit_count) commit, cần 2 (commit của task thứ hai)"
fi
D_DIR="$(fresh_clone d)"
if grep -rlE --exclude-dir=.git 'intranet|/home/|database\.php|abcd1234efgh5678|DEEPSEEK_API_KEY|!\[\[ghi' "$D_DIR" > "$TMP/d.hits" 2> /dev/null; then
  sed "s#^$D_DIR/##" "$TMP/d.hits" | sed 's/^/  tệp còn dấu vết: /' >&2
  die "D: vault còn đường dẫn, URL, khóa hoặc liên kết nhúng của yêu cầu nhạy cảm"
fi
echo "LIVE OK: D yêu cầu nhạy cảm không để lại đường dẫn, URL, khóa hay liên kết nhúng trong vault"

# Kiểm tra E: một secret đã biết trong tệp sinh ra chặn commit và push, thư báo không chứa giá trị.
LIVE_SECRET="zq7livesecret9x"
write_map "$LIVE_SECRET"
"$BIN" stats --json > /dev/null 2>&1 || die "E: không đồng bộ được dự án thứ hai"
seed_task "$LIVE_SECRET" tac-vu-bi-chan "Việc mẫu của dự án thứ hai"
E_BEFORE="$(remote_commit_count)"
set +e
AB_LIVE_JOURNAL_TOKEN="$LIVE_SECRET" "$BIN" journal export > "$TMP/e.out" 2>&1
E_CODE=$?
set -e
if [[ "$E_CODE" != "1" ]]; then
  cat "$TMP/e.out" >&2
  die "E: export có secret thoát $E_CODE, cần 1"
fi
if [[ "$(remote_commit_count)" != "$E_BEFORE" ]]; then
  die "E: export có secret vẫn đẩy commit lên repo bare"
fi
if grep -qF "$LIVE_SECRET" "$TMP/e.out"; then
  die "E: đầu ra của lệnh chứa giá trị secret"
fi
if [[ "$(dbq "SELECT COUNT(*) FROM journal_mail WHERE kind = 'secret'")" == "0" ]]; then
  die "E: journal_mail không có thư kind 'secret'"
fi
if [[ "$(dbq "SELECT COUNT(*) FROM journal_mail WHERE text LIKE '%$LIVE_SECRET%'")" != "0" ]]; then
  die "E: thư journal_mail chứa giá trị secret"
fi
# Gỡ dự án thứ hai (cả trong map và trong DB tạm) để các bước sau không còn thấy nó.
write_map
python3 -I - "$AGENT_BRAIN_DB" "$LIVE_SECRET" << 'PYEOF'
import sqlite3
import sys

c = sqlite3.connect(sys.argv[1])
pid = c.execute("SELECT id FROM projects WHERE slug = ?", (sys.argv[2],)).fetchone()[0]
c.execute("DELETE FROM runs WHERE project_id = ?", (pid,))
c.execute("DELETE FROM tasks WHERE project_id = ?", (pid,))
c.execute("DELETE FROM projects WHERE id = ?", (pid,))
c.commit()
c.close()
PYEOF
echo "LIVE OK: E secret đã biết chặn commit và push, thư 'secret' không chứa giá trị"

# Kiểm tra F: thiếu mục journal trong cấu hình thì bị từ chối.
set +e
AGENT_BRAIN_CONFIG="$TMP/brain_config_no_journal.json" "$BIN" journal export > "$TMP/f.out" 2>&1
F_CODE=$?
set -e
if [[ "$F_CODE" != "1" ]] || ! grep -q 'journal.repo_url' "$TMP/f.out"; then
  cat "$TMP/f.out" >&2
  die "F: thiếu mục journal phải thoát 1 và nhắc journal.repo_url (thoát $F_CODE)"
fi
echo "LIVE OK: F thiếu mục journal bị từ chối, nhắc journal.repo_url"

# Kiểm tra G: dry run tự thử trên DB tạm, thêm hai dòng events mồ côi (tắt khóa ngoại khi chèn).
python3 -I - "$AGENT_BRAIN_DB" << 'PYEOF'
import sqlite3
import sys

c = sqlite3.connect(sys.argv[1])
c.execute("PRAGMA foreign_keys = OFF")
for seq in (1, 2):
    c.execute(
        "INSERT INTO events (run_id, seq, ts, kind, raw_json) VALUES (999999, ?, 0, 'orphan', '{}')",
        (seq,),
    )
c.commit()
c.close()
PYEOF
if ! G_OUT="$(schema_dry_run "$AGENT_BRAIN_DB" 2>&1)"; then
  printf '%s\n' "$G_OUT" >&2
  die "G: dry run trên DB tạm thất bại"
fi
printf '%s\n' "$G_OUT"
for want in "Schema v12 dry run OK (từ v12)" "fk_violations_before=2 after=2" "real_dir_unchanged=${G_YES:-yes}"; do
  if ! printf '%s\n' "$G_OUT" | grep -qF "$want"; then
    die "G: đầu ra dry run thiếu dòng '$want'"
  fi
done
echo "LIVE OK: G dry run chấp nhận hai dòng events mồ côi (fk_violations_before=2 after=2)"

echo "Phase 7 live check OK"
