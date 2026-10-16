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
trap 'rm -rf "$TMP"' EXIT

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

# 6. Success output
SIZE="$(stat -c %s "$BIN")"
SHA="$(sha256sum "$BIN" | awk '{print $1}')"
echo "P2-09 OK (route $ROUTE): $BIN · $SIZE byte · sha256 $SHA"
echo "agent-brain chạy được khi không có gì cài thêm (env -i, PATH=/nonexistent)"
