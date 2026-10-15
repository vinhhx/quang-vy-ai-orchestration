#!/usr/bin/env bash
# spikes/lib/env.sh - sourced by every spikes/sN/run.sh.
#   . "$(dirname "$0")/../lib/env.sh"
# Only `set -u` here: this file is sourced, so it must never `set -e` the caller.
#
# Why a clean-env wrapper: the agent that runs the spikes is itself a Claude Code session, so a
# child `claude` would inherit the nested-session variables (CLAUDECODE, CLAUDE_CODE_*, ...).
# Phase 3 will spawn claude from a normal terminal, so the spikes mimic that. CLAUDE_CONFIG_DIR is
# unset too, so S1-S4 and S8 always run on the default work profile ~/.claude (D-05).
# Never use --bare: it drops the subscription OAuth login.
set -u

_SPIKE_ENV_FILE="${BASH_SOURCE[0]:-$0}"
REPO="$(cd "$(dirname "$_SPIKE_ENV_FILE")/../.." && pwd -P)"
export REPO

# Scratch dirs live OUTSIDE the repo: no parent CLAUDE.md is loaded and git status stays stable.
SPIKE_WORK_ROOT="${SPIKE_WORK_ROOT:-${XDG_CACHE_HOME:-$HOME/.cache}/qv-spikes}"
export SPIKE_WORK_ROOT

ts() { date -u +%Y-%m-%dT%H:%M:%SZ; }
epoch() { date +%s; }

CLEAN_ENV=(env -u CLAUDECODE -u CLAUDE_CODE_CHILD_SESSION -u CLAUDE_CODE_SESSION_ID -u CLAUDE_PID -u CLAUDE_EFFORT -u CLAUDE_VC_ENV -u CLAUDE_CODE_MESSAGING_SOCKET -u CLAUDE_CODE_MESSAGING_TOKEN -u CLAUDE_CODE_SESSION_ATTENDED -u CLAUDE_CODE_ENTRYPOINT -u CLAUDE_CODE_EXECPATH -u CLAUDE_CONFIG_DIR)

# run_claude ARGS... : the only way a spike calls claude.
# SPIKE_PROFILE_DIR (optional) selects a NON-default profile; it must never resolve to
# ~/.claude-orca or ~/.claude (E-07, T-1-03).
run_claude() {
  local -a prof=()
  if [ -n "${SPIKE_PROFILE_DIR:-}" ]; then
    local resolved orca work
    resolved="$(realpath -m -- "$SPIKE_PROFILE_DIR" 2>/dev/null || printf '%s' "$SPIKE_PROFILE_DIR")"
    orca="$(realpath -m -- "$HOME/.claude-orca")"
    work="$(realpath -m -- "$HOME/.claude")"
    if [ "$resolved" = "$orca" ] || [ "$resolved" = "$work" ]; then
      echo "run_claude: refusing SPIKE_PROFILE_DIR that resolves to ~/.claude-orca or ~/.claude" >&2
      return 2
    fi
    prof=(CLAUDE_CONFIG_DIR="$SPIKE_PROFILE_DIR")
  fi
  "${CLEAN_ENV[@]}" ${prof[@]+"${prof[@]}"} timeout "${SPIKE_TIMEOUT:-300}" claude "$@" </dev/null
}

# scrub_raw PATH... : redact files/dirs in place (home path, emails, keys, JWT, Bearer).
scrub_raw() {
  python3 -I "$REPO/spikes/lib/scrub.py" "$@"
}
