#!/usr/bin/env python3
from __future__ import annotations

import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

NESTED_SESSION_VARS = (
    "CLAUDECODE",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_PID",
    "CLAUDE_EFFORT",
    "CLAUDE_VC_ENV",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_EXECPATH",
)


def _update_last_record(record_path: str, blocked: list[dict]) -> None:
    try:
        with open(record_path, "r", encoding="utf-8") as f:
            lines = f.read().splitlines()
        if lines:
            last = json.loads(lines[-1])
            last["blocked"] = blocked
            lines[-1] = json.dumps(last)
            with open(record_path, "w", encoding="utf-8") as f:
                f.write("\n".join(lines) + "\n")
    except Exception:
        pass


def main() -> None:
    settings_val: str | None = None
    if "--settings" in sys.argv:
        idx = sys.argv.index("--settings")
        if idx + 1 < len(sys.argv):
            settings_val = sys.argv[idx + 1]

    blocked: list[dict] = []
    record_path = os.environ.get("FAKE_CLAUDE_RECORD")
    if record_path:
        present_vars = [v for v in NESTED_SESSION_VARS if v in os.environ]
        # The token itself is never recorded: only whether it equals the value the test expects.
        expected_token = os.environ.get("FAKE_CLAUDE_EXPECT_TOKEN")
        actual_token = os.environ.get("ANTHROPIC_AUTH_TOKEN")
        if expected_token is None or actual_token is None:
            token_matches = None
        else:
            token_matches = actual_token == expected_token
        record = {
            "argv": sys.argv[1:],
            "cwd": os.getcwd(),
            "claude_config_dir": os.environ.get("CLAUDE_CONFIG_DIR"),
            "present_vars": present_vars,
            "settings": settings_val,
            "blocked": blocked,
            "env_names": sorted(os.environ.keys()),
            "anthropic_base_url": os.environ.get("ANTHROPIC_BASE_URL"),
            "auth_token_matches_expected": token_matches,
        }
        with open(record_path, "a", encoding="utf-8") as f:
            f.write(json.dumps(record) + "\n")

    scenario: dict = {}
    scenario_path = os.environ.get("FAKE_CLAUDE_SCENARIO")
    if scenario_path and os.path.isfile(scenario_path):
        try:
            with open(scenario_path, "r", encoding="utf-8") as f:
                scenario = json.load(f)
        except Exception as e:
            sys.stderr.write(f"Lỗi đọc FAKE_CLAUDE_SCENARIO: {e}\n")

    if isinstance(scenario, dict) and "by_role" in scenario:
        system_prompt = ""
        if "--append-system-prompt" in sys.argv:
            sp_idx = sys.argv.index("--append-system-prompt")
            if sp_idx + 1 < len(sys.argv):
                system_prompt = sys.argv[sp_idx + 1]
        first_line = system_prompt.splitlines()[0] if system_prompt.splitlines() else ""
        if first_line.startswith("Bạn là Plan Reviewer"):
            role = "plan-review"
        elif first_line.startswith("Bạn là Code Reviewer"):
            role = "code-review"
        elif first_line.startswith("Bạn là Lead"):
            role = "lead"
        elif first_line.startswith("Bạn là QA"):
            role = "qa"
        elif first_line.startswith("Bạn là Dev"):
            role = "dev"
        else:
            role = "unknown"

        state_path = scenario_path + ".state.json"
        state = {}
        if os.path.isfile(state_path):
            try:
                with open(state_path, "r", encoding="utf-8") as sf:
                    state = json.load(sf)
            except Exception:
                state = {}

        role_queue = scenario.get("by_role", {}).get(role, [])
        if role_queue:
            idx = state.get(role, 0)
            entry_idx = min(idx, len(role_queue) - 1)
            chosen_entry = role_queue[entry_idx]
        else:
            chosen_entry = {}

        state[role] = state.get(role, 0) + 1
        try:
            with open(state_path, "w", encoding="utf-8") as sf:
                json.dump(state, sf)
        except Exception:
            pass

        scenario = chosen_entry

    if "stderr" in scenario:
        sys.stderr.write(scenario["stderr"])
        sys.stderr.flush()

    if "sleep" in scenario:
        time.sleep(float(scenario["sleep"]))

    raw_lines = scenario.get("raw_lines")
    events = scenario.get("events")
    ops = scenario.get("ops", [])

    hooks: list[dict] = []
    if settings_val and not scenario.get("ignore_hooks"):
        try:
            with open(settings_val, "r", encoding="utf-8") as sf:
                settings_data = json.load(sf)
                hooks = settings_data.get("hooks", {}).get("PreToolUse", [])
        except Exception as e:
            sys.stderr.write(f"Lỗi đọc settings: {e}\n")

    tool_events: list[dict] = []
    for i, op in enumerate(ops):
        op_type = op.get("op")
        target_path: Path | None = None
        path_for_blocked = ""
        content = ""
        existed = False
        args = []

        if op_type == "write":
            target_path = Path(op["path"])
            if not target_path.is_absolute():
                target_path = Path(os.getcwd()) / target_path
            existed = target_path.exists()
            content = op.get("content", "")
            if existed and op.get("keep_meta"):
                try:
                    with open(target_path, "r", encoding="utf-8") as f:
                        old_lines = f.read().splitlines()
                    if len(old_lines) >= 2:
                        meta_line = old_lines[1]
                        new_lines = content.splitlines()
                        if new_lines:
                            content = (
                                new_lines[0]
                                + "\n"
                                + meta_line
                                + "\n"
                                + "\n".join(new_lines[1:])
                                + ("\n" if content.endswith("\n") else "")
                            )
                        else:
                            content = meta_line + "\n"
                except Exception:
                    pass
            tool_name = "Edit" if existed else "Write"
            tool_input = {"file_path": str(target_path.resolve())}
            path_for_blocked = str(target_path.resolve())
        elif op_type == "read":
            target_path = Path(op["path"])
            if not target_path.is_absolute():
                target_path = Path(os.getcwd()) / target_path
            tool_name = "Read"
            tool_input = {"file_path": str(target_path.resolve())}
            path_for_blocked = str(target_path.resolve())
        elif op_type == "grep":
            target_path = Path(op.get("path", os.getcwd()))
            if not target_path.is_absolute():
                target_path = Path(os.getcwd()) / target_path
            tool_name = "Grep"
            tool_input = {
                "pattern": op.get("pattern", ""),
                "path": str(target_path.resolve()),
            }
            path_for_blocked = str(target_path.resolve())
        elif op_type == "git":
            tool_name = "Bash"
            args = op.get("args", [])
            tool_input = {"command": " ".join(["git"] + [str(a) for a in args])}
            path_for_blocked = ""
        else:
            continue

        # Check hooks
        is_blocked = False
        hook_stderr = ""
        for entry in hooks:
            if not isinstance(entry, dict):
                continue
            nested_hooks = entry.get("hooks")
            if not isinstance(nested_hooks, list):
                continue
            matcher = entry.get("matcher", "")
            if matcher and re.fullmatch(matcher, tool_name):
                for hook_item in nested_hooks:
                    if not isinstance(hook_item, dict):
                        continue
                    if hook_item.get("type") == "command":
                        cmd = hook_item.get("command", "")
                        payload = {
                            "hook_event_name": "PreToolUse",
                            "tool_name": tool_name,
                            "tool_input": tool_input,
                            "cwd": os.getcwd(),
                        }
                        res = subprocess.run(
                            cmd,
                            shell=True,
                            input=json.dumps(payload),
                            text=True,
                            capture_output=True,
                        )
                        if res.returncode == 2:
                            is_blocked = True
                            hook_stderr = res.stderr
                            break
            if is_blocked:
                break

        if is_blocked:
            blocked.append({"tool": tool_name, "path": path_for_blocked})
            asst_event = {
                "type": "assistant",
                "session_id": "fake-session",
                "message": {
                    "id": f"msg_{i}",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "tool_use",
                            "id": f"toolu_{i}",
                            "name": tool_name,
                            "input": tool_input,
                        }
                    ],
                },
            }
            user_event = {
                "type": "user",
                "message": {
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": f"toolu_{i}",
                            "content": hook_stderr,
                            "is_error": True,
                        }
                    ]
                },
            }
            tool_events.append(asst_event)
            tool_events.append(user_event)
            continue

        # Execute op if not blocked
        if op_type == "write" and target_path:
            target_path.parent.mkdir(parents=True, exist_ok=True)
            mode = "a" if op.get("append") else "w"
            with open(target_path, mode, encoding="utf-8") as f:
                f.write(content)
        elif op_type == "git":
            subprocess.run(["git", *args], cwd=os.getcwd(), capture_output=True)

        asst_event = {
            "type": "assistant",
            "session_id": "fake-session",
            "message": {
                "id": f"msg_{i}",
                "role": "assistant",
                "content": [
                    {
                        "type": "tool_use",
                        "id": f"toolu_{i}",
                        "name": tool_name,
                        "input": tool_input,
                    }
                ],
            },
        }
        user_event = {
            "type": "user",
            "message": {
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": f"toolu_{i}",
                        "content": "ok",
                    }
                ]
            },
        }
        tool_events.append(asst_event)
        tool_events.append(user_event)

    if raw_lines is not None:
        for line in raw_lines:
            sys.stdout.write(line if line.endswith("\n") else line + "\n")
            sys.stdout.flush()
    elif events is not None:
        for ev in events:
            line = json.dumps(ev) if isinstance(ev, dict) else str(ev)
            sys.stdout.write(line if line.endswith("\n") else line + "\n")
            sys.stdout.flush()
    else:
        init_event = {
            "type": "system",
            "subtype": "init",
            "session_id": "fake-session",
            "model": "fake",
        }
        sys.stdout.write(json.dumps(init_event) + "\n")
        sys.stdout.flush()

        for ev in tool_events:
            sys.stdout.write(json.dumps(ev) + "\n")
            sys.stdout.flush()

        total_cost = scenario.get("result_cost", 0)
        res_tokens = scenario.get("result_tokens")
        if isinstance(res_tokens, dict):
            usage = {
                "input_tokens": res_tokens.get("input", 10),
                "output_tokens": res_tokens.get("output", 5),
                "cache_read_input_tokens": res_tokens.get("cache_read", 0),
                "cache_creation_input_tokens": res_tokens.get("cache_write", 0),
            }
        else:
            usage = {
                "input_tokens": 10,
                "output_tokens": 5,
                "cache_read_input_tokens": 0,
                "cache_creation_input_tokens": 0,
            }

        result_event = {
            "type": "result",
            "subtype": "success",
            "is_error": False,
            "num_turns": len(ops) + 1,
            "session_id": "fake-session",
            "total_cost_usd": total_cost,
            "usage": usage,
            "permission_denials": [],
        }
        sys.stdout.write(json.dumps(result_event) + "\n")
        sys.stdout.flush()

    if record_path and blocked:
        _update_last_record(record_path, blocked)

    exit_code = scenario.get("exit_code", 0)
    sys.exit(exit_code)


if __name__ == "__main__":
    main()
