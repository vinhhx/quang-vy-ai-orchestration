#!/usr/bin/env bash
# scripts/live/claude-allow-sleep.sh
# Wrapper script for Claude Code CLI, used exclusively for the live P2-02 and P2-03 checks
# on the throwaway repo (/mnt/data/IDE/du-an-nhap).
#
# Claude 2.1.301 blocks a foreground 'sleep N' in the Bash tool ("Blocked: standalone sleep"),
# so the kill checks hold the process with python3 -c 'import time; time.sleep(307)'.
# Inserts 'Bash(python3 *)' immediately after the first '--allowedTools' argument.
# Warning: 'Bash(python3 *)' is broad and must never be used for real work.

set -euo pipefail

args=()
inserted=false

for arg in "$@"; do
  args+=("$arg")
  if [[ "$arg" == "--allowedTools" && "$inserted" == false ]]; then
    args+=("Bash(python3 *)")
    inserted=true
  fi
done

exec "${AGENT_BRAIN_REAL_CLAUDE:-claude}" "${args[@]}"
