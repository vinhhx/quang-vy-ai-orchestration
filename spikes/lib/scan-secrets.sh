#!/usr/bin/env bash
# spikes/lib/scan-secrets.sh - hard gate: must exit 0 before anything under spikes/ is staged (D-03).
#
#   bash spikes/lib/scan-secrets.sh [--container NAME]
#
# Host scan:
#   1. value denylist: every host env var whose NAME matches key|token|secret|password and whose
#      value is at least 8 characters is searched for as a fixed string. A hit prints only the NAME.
#   2. patterns: API keys, JWTs, Bearer tokens, emails, and the home directory path.
# --container NAME also runs the same value denylist INSIDE the container (docker exec), so the
# container's secret values (the DeepSeek key) never reach the host. Output: LEAK:<NAME> or CLEAN.
# Exit 1 on any hit. A matching VALUE or line is never echoed - only names and file:line.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO="$(cd "$SCRIPT_DIR/../.." && pwd -P)"
TARGET="$REPO/spikes"
CONTAINER=""
TMP=""
cleanup() { [ -n "${TMP:-}" ] && rm -f "$TMP" 2>/dev/null || true; }
trap cleanup EXIT

while [ $# -gt 0 ]; do
  case "$1" in
    --container) CONTAINER="${2:-}"; shift 2 || { echo "scan: --container needs a name" >&2; exit 2; } ;;
    *) echo "scan: unknown argument: $1" >&2; exit 2 ;;
  esac
done

[ -d "$TARGET" ] || { echo "scan: no spikes/ directory" >&2; exit 2; }
TMP="$(mktemp)"
EXCL=(--exclude-dir=work --exclude-dir=scratch --exclude-dir=.git)
hits=0

# 1. host env value denylist (names only on hit)
while IFS= read -r name; do
  [ -n "$name" ] || continue
  val="${!name-}"
  [ "${#val}" -ge 8 ] || continue
  case "$val" in *$'\n'*) continue ;; esac   # a newline would split the fixed-string patterns
  if grep -rqIF "${EXCL[@]}" -- "$val" "$TARGET" 2>/dev/null; then
    echo "LEAK: value of host env var \$$name is present under spikes/"
    hits=1
  fi
done < <(env | cut -d= -f1 | grep -iE 'key|token|secret|password' || true)

# 2. patterns (print file:line and the pattern label only)
scan_pattern() {  # label, extended regex
  local label="$1" re="$2"
  if grep -rEIn "${EXCL[@]}" -e "$re" "$TARGET" 2>/dev/null | cut -d: -f1,2 >"$TMP" && [ -s "$TMP" ]; then
    echo "HIT[$label]:"
    sed 's/^/  /' "$TMP"
    hits=1
  fi
}
scan_pattern "api-key"  'sk-[A-Za-z0-9_-]{16,}|sk-ant-[A-Za-z0-9_-]{8,}|ghp_[A-Za-z0-9]{20,}'
scan_pattern "bearer"   'Bearer [A-Za-z0-9._-]{16,}'
scan_pattern "jwt"      'eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]+\.'
scan_pattern "home"     "/home/${USER:-$(id -un)}([^A-Za-z0-9_-]|\$)"

# emails: allow-list the documentation domain used by the scratch git identity in S8
if grep -rEIno "${EXCL[@]}" -e '[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}' "$TARGET" 2>/dev/null \
    | grep -viE '@example\.invalid$' | cut -d: -f1,2 >"$TMP" && [ -s "$TMP" ]; then
  echo "HIT[email]:"
  sed 's/^/  /' "$TMP"
  hits=1
fi

# 3. container value denylist
if [ -n "$CONTAINER" ]; then
  command -v docker >/dev/null 2>&1 || { echo "scan: docker not available for --container" >&2; exit 1; }
  # shellcheck disable=SC2016
  out="$(docker exec "$CONTAINER" sh -c '
    found=0
    for n in $(env | cut -d= -f1 | grep -iE "key|token|secret|password"); do
      eval "v=\${$n}"
      [ "${#v}" -ge 8 ] || continue
      case "$v" in *"
"*) continue ;; esac
      if grep -rqIF --exclude-dir=work --exclude-dir=scratch -- "$v" "$1" 2>/dev/null; then
        echo "LEAK:$n"; found=1
      fi
    done
    [ "$found" -eq 1 ] || echo CLEAN
  ' sh "$TARGET" </dev/null 2>/dev/null)" || { echo "scan: docker exec into $CONTAINER failed" >&2; exit 1; }
  echo "$out"
  case "$out" in *LEAK:*) hits=1 ;; esac
fi

if [ "$hits" -ne 0 ]; then
  echo "scan-secrets: FAIL" >&2
  exit 1
fi
echo "scan-secrets: OK"
