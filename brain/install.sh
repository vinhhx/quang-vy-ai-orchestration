#!/usr/bin/env bash
set -euo pipefail

SCRIPT_SOURCE="${BASH_SOURCE[0]}"
while [ -h "$SCRIPT_SOURCE" ]; do
  SCRIPT_DIR="$(cd -P "$(dirname "$SCRIPT_SOURCE")" && pwd)"
  SCRIPT_SOURCE="$(readlink "$SCRIPT_SOURCE")"
  [[ $SCRIPT_SOURCE != /* ]] && SCRIPT_SOURCE="$SCRIPT_DIR/$SCRIPT_SOURCE"
done
SCRIPT_DIR="$(cd -P "$(dirname "$SCRIPT_SOURCE")" && pwd)"
REPO_ROOT="$(cd -P "$SCRIPT_DIR/.." && pwd)"
TARGET_BIN="$SCRIPT_DIR/brain"

BIN_DIR="${BRAIN_BIN_DIR:-$HOME/.local/bin}"

if [[ "${1:-}" == "--uninstall" ]]; then
  LINK_PATH="$BIN_DIR/brain"
  if [[ -e "$LINK_PATH" || -L "$LINK_PATH" ]]; then
    if [[ -L "$LINK_PATH" ]]; then
      CURRENT_TARGET="$(readlink -f "$LINK_PATH" 2>/dev/null || true)"
      EXPECTED_TARGET="$(readlink -f "$TARGET_BIN" 2>/dev/null || true)"
      if [[ "$CURRENT_TARGET" == "$EXPECTED_TARGET" ]]; then
        rm -f "$LINK_PATH"
        echo "Đã gỡ symlink $LINK_PATH"
        exit 0
      else
        echo "Lỗi: $LINK_PATH trỏ tới file khác ($CURRENT_TARGET) — từ chối gỡ" >&2
        exit 1
      fi
    else
      echo "Lỗi: $LINK_PATH không phải là symlink — từ chối gỡ" >&2
      exit 1
    fi
  else
    echo "Không tìm thấy $LINK_PATH để gỡ"
    exit 0
  fi
fi

mkdir -p "$BIN_DIR"
LINK_PATH="$BIN_DIR/brain"

if [[ -e "$LINK_PATH" || -L "$LINK_PATH" ]]; then
  if [[ ! -L "$LINK_PATH" ]]; then
    echo "Lỗi: $LINK_PATH đã tồn tại và không phải là symlink" >&2
    exit 1
  fi
  CURRENT_TARGET="$(readlink -f "$LINK_PATH" 2>/dev/null || true)"
  EXPECTED_TARGET="$(readlink -f "$TARGET_BIN" 2>/dev/null || true)"
  if [[ "$CURRENT_TARGET" != "$EXPECTED_TARGET" ]]; then
    echo "Lỗi: $LINK_PATH là symlink trỏ tới file khác ($CURRENT_TARGET) — từ chối ghi đè" >&2
    exit 1
  fi
fi

ln -sfn "$TARGET_BIN" "$LINK_PATH"
echo "Đã cài đặt brain vào $LINK_PATH -> $TARGET_BIN"

case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *)
    echo "[Gợi ý] Thư mục $BIN_DIR chưa có trong PATH. Hãy thêm: export PATH=\"$BIN_DIR:\$PATH\""
    ;;
esac
