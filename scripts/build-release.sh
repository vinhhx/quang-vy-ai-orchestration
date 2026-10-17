#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -P "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -P "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

# 1. Target check
TARGET="x86_64-unknown-linux-musl"
if command -v rustup >/dev/null 2>&1; then
  if ! rustup target list --installed | grep -q "^${TARGET}\$"; then
    echo "Thiếu target $TARGET — chạy: rustup target add $TARGET" >&2
    exit 1
  fi
fi

# 2. Compiler selection (Route A vs Route B)
ROUTE=""
if command -v musl-gcc >/dev/null 2>&1; then
  ROUTE="A"
  export CC_x86_64_unknown_linux_musl=musl-gcc
else
  ROUTE="B"
  echo "CẢNH BÁO: không có musl-gcc — dùng gcc + CFLAGS (route B)" >&2
  export CC_x86_64_unknown_linux_musl=gcc
  export CFLAGS_x86_64_unknown_linux_musl="-U_FORTIFY_SOURCE -D_FORTIFY_SOURCE=0 -DSQLITE_DISABLE_LFS"
fi

# 3. Cargo build
cargo build --manifest-path agent-brain/Cargo.toml --release --offline --locked --target "$TARGET"
BIN="agent-brain/target/$TARGET/release/agent-brain"

# 4. Static assertions
FILE_OUT="$(file "$BIN")"
if ! echo "$FILE_OUT" | grep -qE 'statically linked|static-pie linked'; then
  echo "Lỗi: binary không tĩnh ($FILE_OUT)" >&2
  exit 1
fi

if readelf -d "$BIN" 2>&1 | grep -q NEEDED; then
  echo "Lỗi: binary không tĩnh (readelf phát hiện NEEDED)" >&2
  exit 1
fi

# 5. Copy test in isolated environment
TMP="$(mktemp -d)"
SERVE_PID=""
cleanup() {
  if [[ -n "${SERVE_PID:-}" ]]; then
    if [[ -d "/proc/$SERVE_PID" ]] && ! grep -q "State:[[:space:]]*Z" "/proc/$SERVE_PID/status" 2>/dev/null; then
      kill -INT "$SERVE_PID" 2>/dev/null || true
      sleep 0.2
      if [[ -d "/proc/$SERVE_PID" ]] && ! grep -q "State:[[:space:]]*Z" "/proc/$SERVE_PID/status" 2>/dev/null; then
        kill -KILL "$SERVE_PID" 2>/dev/null || true
      fi
    fi
  fi
  rm -rf "$TMP"
}
trap cleanup EXIT

mkdir -p "$TMP/bin" "$TMP/home"
cp "$BIN" "$TMP/bin/agent-brain"
chmod 755 "$TMP/bin/agent-brain"

if ! env -i HOME="$TMP/home" PATH=/nonexistent "$TMP/bin/agent-brain" --version >/dev/null 2>&1; then
  echo "Lỗi: kiểm tra --version trong môi trường rỗng thất bại" >&2
  exit 1
fi

STATS_OUT="$(env -i HOME="$TMP/home" PATH=/nonexistent "$TMP/bin/agent-brain" stats --json 2>/dev/null || true)"
if ! echo "$STATS_OUT" | grep -q '"total"'; then
  echo "Lỗi: kiểm tra stats --json trong môi trường rỗng thất bại" >&2
  exit 1
fi

GUARD_STATUS=0
printf '{bad' | env -i HOME="$TMP/home" PATH=/nonexistent "$TMP/bin/agent-brain" guard --policy /nonexistent >/dev/null 2>&1 || GUARD_STATUS=$?
if [[ "$GUARD_STATUS" -ne 2 ]]; then
  echo "Lỗi: kiểm tra guard trong môi trường rỗng thất bại (mã thoát: $GUARD_STATUS)" >&2
  exit 1
fi

# 6. Dashboard smoke (P4-01)
if ! command -v curl >/dev/null 2>&1; then
  echo "Thiếu curl cho kiểm tra dashboard" >&2
  exit 1
fi

env -i HOME="$TMP/home" PATH=/nonexistent "$TMP/bin/agent-brain" serve --port 0 >"$TMP/serve.out" 2>"$TMP/serve.err" &
SERVE_PID=$!

TOKEN_URL=""
for _ in $(seq 1 50); do
  if [[ -s "$TMP/serve.out" ]]; then
    TOKEN_URL="$(grep -oE 'http://127\.0\.0\.1:[0-9]+/\?token=[0-9a-f]{64}' "$TMP/serve.out" | head -n 1 || true)"
    if [[ -n "$TOKEN_URL" ]]; then
      break
    fi
  fi
  python3 -c 'import time; time.sleep(0.2)'
done

if [[ -z "$TOKEN_URL" ]]; then
  cat "$TMP/serve.err" >&2 || true
  kill -INT "$SERVE_PID" 2>/dev/null || true
  echo "Lỗi: serve không in URL trong 10 giây" >&2
  exit 1
fi

PORT="$(echo "$TOKEN_URL" | sed -E 's|http://127\.0\.0\.1:([0-9]+)/.*|\1|')"

# GET /?token= trả về 303 và ghi cookie
HTTP_303="$(curl -s -o /dev/null -w "%{http_code}" -c "$TMP/jar" "$TOKEN_URL" || true)"
if [[ "$HTTP_303" != "303" ]]; then
  echo "Lỗi: kiểm tra GET /?token= thất bại (mã HTTP: $HTTP_303, mong đợi 303)" >&2
  kill -INT "$SERVE_PID" 2>/dev/null || true
  exit 1
fi

# GET /app.css có cookie trả về 200 và banner tailwindcss v4.3.3
CSS_CODE="$(curl -s -b "$TMP/jar" -o "$TMP/app.css" -w "%{http_code}" "http://127.0.0.1:$PORT/app.css" || true)"
if [[ "$CSS_CODE" != "200" ]]; then
  echo "Lỗi: kiểm tra GET /app.css có cookie thất bại (mã HTTP: $CSS_CODE, mong đợi 200)" >&2
  kill -INT "$SERVE_PID" 2>/dev/null || true
  exit 1
fi
FIRST_LINE="$(head -n 1 "$TMP/app.css" 2>/dev/null || true)"
if ! echo "$FIRST_LINE" | grep -q 'tailwindcss v4\.3\.3'; then
  echo "Lỗi: nội dung /app.css không chứa banner tailwindcss v4.3.3" >&2
  kill -INT "$SERVE_PID" 2>/dev/null || true
  exit 1
fi

# GET / không có cookie trả về 401
UNAUTH_CODE="$(curl -s -o /dev/null -w "%{http_code}" "http://127.0.0.1:$PORT/" || true)"
if [[ "$UNAUTH_CODE" != "401" ]]; then
  echo "Lỗi: kiểm tra GET / không có cookie thất bại (mã HTTP: $UNAUTH_CODE, mong đợi 401)" >&2
  kill -INT "$SERVE_PID" 2>/dev/null || true
  exit 1
fi

# Dừng serve bằng SIGINT và chờ thoát sạch
kill -INT "$SERVE_PID"
STOPPED=false
for _ in $(seq 1 50); do
  if [[ ! -d "/proc/$SERVE_PID" ]] || grep -q "State:[[:space:]]*Z" "/proc/$SERVE_PID/status" 2>/dev/null; then
    STOPPED=true
    break
  fi
  python3 -c 'import time; time.sleep(0.2)'
done

if [[ "$STOPPED" = false ]]; then
  kill -KILL "$SERVE_PID" 2>/dev/null || true
  echo "Lỗi: serve không dừng sạch sau Ctrl+C" >&2
  exit 1
fi

SERVE_STATUS=0
wait "$SERVE_PID" || SERVE_STATUS=$?
SERVE_PID=""
if [[ "$SERVE_STATUS" -ne 0 ]]; then
  echo "Lỗi: serve không dừng sạch sau Ctrl+C (mã thoát: $SERVE_STATUS)" >&2
  exit 1
fi

# 7. Success output
SIZE="$(stat -c %s "$BIN")"
SHA="$(sha256sum "$BIN" | awk '{print $1}')"
echo "P2-09 OK (route $ROUTE): $BIN · $SIZE byte · sha256 $SHA"
echo "P4-01 OK: dashboard chạy trong môi trường rỗng (env -i, PATH=/nonexistent) — / và /app.css qua token, 401 khi thiếu token"
echo "agent-brain chạy được khi không có gì cài thêm (env -i, PATH=/nonexistent)"
