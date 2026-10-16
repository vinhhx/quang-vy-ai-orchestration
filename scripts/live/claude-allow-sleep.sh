#!/usr/bin/env bash
# scripts/live/claude-allow-sleep.sh
# Wrapper script for Claude Code CLI, used exclusively for the live P2-02 and P2-03 checks.
# Inserts 'Bash(sleep *)' immediately after the first '--allowedTools' argument so that
# Lead/agent can execute sleep 307 for process-tree kill verification without modifying production defaults.

set -euo pipefail

args=()
inserted=false

for arg in "$@"; do
  args+=("$arg")
  if [[ "$arg" == "--allowedTools" && "$inserted" == false ]]; then
    args+=("Bash(sleep *)")
    inserted=true
  fi
done

exec "${AGENT_BRAIN_REAL_CLAUDE:-claude}" "${args[@]}"
