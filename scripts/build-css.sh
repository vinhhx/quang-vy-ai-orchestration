#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -P "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -P "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

TW="agent-brain/tools/tailwindcss"
TW_SHA="dc61b3ac6b8c9ca874c0cc4c57b2409791a64c5540404ca5f5367360babc313a"
OUT="agent-brain/assets/dashboard/app.css"

MODE="build"
if [[ $# -eq 0 ]]; then
  MODE="build"
elif [[ $# -eq 1 && "$1" == "--check" ]]; then
  MODE="check"
else
  echo "Cách dùng: bash scripts/build-css.sh [--check]" >&2
  exit 2
fi

if [[ ! -x "$TW" ]]; then
  echo "Thiếu agent-brain/tools/tailwindcss — tải Tailwind v4.3.3 theo agent-brain/assets/dashboard/VENDOR.md (bước của người dùng, cần mạng)" >&2
  exit 1
fi

ACTUAL_SHA="$(sha256sum "$TW" | awk '{print $1}')"
if [[ "$ACTUAL_SHA" != "$TW_SHA" ]]; then
  echo "Lỗi: sha256 của agent-brain/tools/tailwindcss không khớp Tailwind v4.3.3" >&2
  exit 1
fi

if [[ "$MODE" == "build" ]]; then
  mkdir -p "$(dirname "$OUT")"
  (
    cd agent-brain/dashboard-src
    "$REPO_ROOT/$TW" -i input.css -o ../assets/dashboard/app.css --minify
  )
  BYTES="$(stat -c %s "$OUT")"
  echo "Đã tạo agent-brain/assets/dashboard/app.css ($BYTES byte)"
  exit 0
fi

if [[ "$MODE" == "check" ]]; then
  TMP_OUT="$(mktemp)"
  trap 'rm -f "$TMP_OUT"' EXIT
  (
    cd agent-brain/dashboard-src
    "$REPO_ROOT/$TW" -i input.css -o "$TMP_OUT" --minify
  )
  if [[ ! -f "$OUT" ]] || ! cmp -s "$TMP_OUT" "$OUT"; then
    echo "app.css đã cũ so với nguồn — chạy: bash scripts/build-css.sh" >&2
    exit 1
  fi
  echo "app.css khớp nguồn"
  exit 0
fi
